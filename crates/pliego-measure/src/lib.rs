//! Medidas sobre planos: escala calibrada, distancia, perímetro y área, y anotaciones de medida.
//!
//! La escala se guarda en el PDF como viewport con diccionario `Measure` (ISO 32000-1, 12.9), de modo que otros
//! lectores la entienden. Las anotaciones de medida son `Line`, `PolyLine` o `Polygon` con su `Measure`, y el
//! texto de la medida en `Contents`. Las coordenadas son las de página en puntos, las mismas que `pliego-annot`.

mod pdf;

use std::fmt;
use std::str::FromStr;

use pliego_core::{Error, Lang, Result};
use serde::{Deserialize, Serialize};

pub use pdf::{MeasuredAnnot, add_measurement, add_measurement_in, list_measurements, page_scale, set_scale};

pub type Pt = [f32; 2];

/// Unidades de longitud reconocidas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Unit {
    Pt,
    Mm,
    Cm,
    M,
    Km,
    In,
    Ft,
}

impl Unit {
    pub fn label(self) -> &'static str {
        match self {
            Unit::Pt => "pt",
            Unit::Mm => "mm",
            Unit::Cm => "cm",
            Unit::M => "m",
            Unit::Km => "km",
            Unit::In => "in",
            Unit::Ft => "ft",
        }
    }

    /// Cuántos puntos PDF (1/72 in) caben en una unidad.
    pub fn points(self) -> f64 {
        match self {
            Unit::Pt => 1.0,
            Unit::Mm => 72.0 / 25.4,
            Unit::Cm => 72.0 / 2.54,
            Unit::M => 72.0 / 0.0254,
            Unit::Km => 72.0 / 0.0000254,
            Unit::In => 72.0,
            Unit::Ft => 72.0 * 12.0,
        }
    }
}

impl FromStr for Unit {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self> {
        Ok(match s.trim().to_lowercase().as_str() {
            "pt" => Unit::Pt,
            "mm" => Unit::Mm,
            "cm" => Unit::Cm,
            "m" => Unit::M,
            "km" => Unit::Km,
            "in" | "\"" => Unit::In,
            "ft" | "'" => Unit::Ft,
            other => {
                return Err(Error::InvalidInput(format!(
                    "unidad desconocida «{other}» (mm, cm, m, km, in, ft, pt)"
                )));
            }
        })
    }
}

impl fmt::Display for Unit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Escala de una página: cuántas unidades reales hay en un punto del papel.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Scale {
    pub unit: Unit,
    pub units_per_point: f64,
}

impl Scale {
    /// Calibración con una medida conocida: `page_points` puntos del papel equivalen a `real` `unit`.
    pub fn calibrate(page_points: f64, real: f64, unit: Unit) -> Result<Scale> {
        if !(page_points.is_finite() && real.is_finite()) || page_points <= 0.0 || real <= 0.0 {
            return Err(Error::InvalidInput(
                "la distancia en el papel y la real deben ser mayores que cero".into(),
            ));
        }
        Ok(Scale {
            unit,
            units_per_point: real / page_points,
        })
    }

    /// Escala dada como razón: `paper_value` `paper_unit` en el papel = `real_value` `real_unit` en la realidad
    /// (por ejemplo, 1 cm = 5 m). El resultado se expresa en `real_unit`.
    pub fn from_ratio(paper_value: f64, paper_unit: Unit, real_value: f64, real_unit: Unit) -> Result<Scale> {
        Scale::calibrate(paper_value * paper_unit.points(), real_value, real_unit)
    }

    /// Texto de la razón, p. ej. «1 pt = 0.0353 m».
    pub fn ratio_text(&self) -> String {
        format!("1 pt = {} {}", trim_num(self.units_per_point), self.unit)
    }
}

fn trim_num(v: f64) -> String {
    let s = format!("{v:.6}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Shape {
    /// Dos puntos.
    Line,
    /// Tramos abiertos: longitud.
    Polyline,
    /// Figura cerrada: perímetro y área.
    Polygon,
}

/// Resultado de medir una figura con una escala.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Measurement {
    pub shape: Shape,
    /// Longitud (distancia, longitud de la línea quebrada o perímetro), en `unit`.
    pub length: f64,
    /// Área en `unit`², solo para figuras cerradas.
    pub area: Option<f64>,
    pub unit: Unit,
    /// Texto para mostrar (y para `Contents` de la anotación).
    pub text: String,
}

fn seg(a: Pt, b: Pt) -> f64 {
    let (dx, dy) = (f64::from(b[0] - a[0]), f64::from(b[1] - a[1]));
    dx.hypot(dy)
}

/// Número con dos decimales: coma en español, punto en inglés.
fn fmt_value(v: f64, lang: Lang) -> String {
    let s = format!("{v:.2}");
    match lang {
        Lang::Es => s.replace('.', ","),
        Lang::En => s,
    }
}

/// Mide una figura con el texto en español. `closed` = polígono (perímetro y área); si no, 2 puntos = distancia y más
/// = línea quebrada.
pub fn measure(scale: &Scale, points: &[Pt], closed: bool) -> Result<Measurement> {
    measure_in(scale, points, closed, Lang::Es)
}

/// Como [`measure`], con el texto de la medida (`Measurement::text`) en `lang`.
pub fn measure_in(scale: &Scale, points: &[Pt], closed: bool, lang: Lang) -> Result<Measurement> {
    let min = if closed { 3 } else { 2 };
    if points.len() < min {
        return Err(Error::InvalidInput(format!(
            "hacen falta al menos {min} puntos para medir esta figura"
        )));
    }
    if points.iter().flatten().any(|v| !v.is_finite()) {
        return Err(Error::InvalidInput("coordenadas no válidas".into()));
    }
    let mut len_pt: f64 = points.windows(2).map(|w| seg(w[0], w[1])).sum();
    let unit = scale.unit;
    if !closed {
        let length = len_pt * scale.units_per_point;
        return Ok(Measurement {
            shape: if points.len() == 2 {
                Shape::Line
            } else {
                Shape::Polyline
            },
            length,
            area: None,
            unit,
            text: format!("{} {unit}", fmt_value(length, lang)),
        });
    }
    len_pt += seg(points[points.len() - 1], points[0]);
    let twice: f64 = (0..points.len())
        .map(|i| {
            let (a, b) = (points[i], points[(i + 1) % points.len()]);
            f64::from(a[0]) * f64::from(b[1]) - f64::from(b[0]) * f64::from(a[1])
        })
        .sum();
    let k = scale.units_per_point;
    let (length, area) = (len_pt * k, twice.abs() / 2.0 * k * k);
    Ok(Measurement {
        shape: Shape::Polygon,
        length,
        area: Some(area),
        unit,
        text: match lang {
            Lang::Es => format!(
                "Área {} {unit}² · Perímetro {} {unit}",
                fmt_value(area, lang),
                fmt_value(length, lang)
            ),
            Lang::En => format!(
                "Area {} {unit}² · Perimeter {} {unit}",
                fmt_value(area, lang),
                fmt_value(length, lang)
            ),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calibrates_and_measures() {
        // 200 pt del plano son 5 m.
        let s = Scale::calibrate(200.0, 5.0, Unit::M).unwrap();
        let d = measure(&s, &[[0.0, 0.0], [120.0, 160.0]], false).unwrap();
        assert_eq!(d.shape, Shape::Line);
        assert!((d.length - 5.0).abs() < 1e-9);
        assert_eq!(d.text, "5,00 m");
    }

    #[test]
    fn area_and_perimeter_of_a_rectangle() {
        // 1 cm de papel = 5 m reales; rectángulo de 2 cm × 1 cm de papel = 10 m × 5 m.
        let s = Scale::from_ratio(1.0, Unit::Cm, 5.0, Unit::M).unwrap();
        let (w, h) = (2.0 * Unit::Cm.points() as f32, Unit::Cm.points() as f32);
        let m = measure(&s, &[[0.0, 0.0], [w, 0.0], [w, h], [0.0, h]], true).unwrap();
        assert!((m.length - 30.0).abs() < 1e-3, "{}", m.length);
        assert!((m.area.unwrap() - 50.0).abs() < 1e-2, "{:?}", m.area);
        assert!(m.text.contains("Área 50,00 m²"));
    }

    #[test]
    fn rejects_bad_input() {
        assert!(Scale::calibrate(0.0, 1.0, Unit::M).is_err());
        let s = Scale::calibrate(1.0, 1.0, Unit::M).unwrap();
        assert!(measure(&s, &[[0.0, 0.0]], false).is_err());
        assert!(measure(&s, &[[0.0, 0.0], [1.0, 1.0]], true).is_err());
        assert!("furlong".parse::<Unit>().is_err());
    }

    #[test]
    fn the_text_follows_the_language() {
        let s = Scale::calibrate(100.0, 5.0, Unit::M).unwrap();
        let sq = [[0.0, 0.0], [100.0, 0.0], [100.0, 100.0], [0.0, 100.0]];
        assert_eq!(
            measure_in(&s, &sq, true, Lang::Es).unwrap().text,
            "Área 25,00 m² · Perímetro 20,00 m"
        );
        assert_eq!(
            measure_in(&s, &sq, true, Lang::En).unwrap().text,
            "Area 25.00 m² · Perimeter 20.00 m"
        );
        assert_eq!(measure_in(&s, &sq[..2], false, Lang::En).unwrap().text, "5.00 m");
    }
}
