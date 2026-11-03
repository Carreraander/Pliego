use mupdf::Buffer;
use mupdf::pdf::{PdfDocument, PdfObject};
use pliego_core::{Error, JobContext, Result};
use serde::{Deserialize, Serialize};

use crate::pdf_err;

const MM: f32 = 72.0 / 25.4;

/// Opciones de [`add_printer_marks`]. Todas las medidas en puntos (1 mm = 2,835 pt).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarkOptions {
    /// Sangrado alrededor de la caja de corte (3 mm es lo habitual).
    pub bleed: f32,
    /// Separación entre el borde del sangrado y el inicio de cada marca.
    pub gap: f32,
    /// Longitud de cada marca de corte.
    pub length: f32,
    /// Grosor de las líneas de las marcas.
    pub line_width: f32,
    pub crop_marks: bool,
    /// Cruces de registro en el centro de cada lado.
    pub registration_marks: bool,
}

impl Default for MarkOptions {
    fn default() -> Self {
        Self {
            bleed: 3.0 * MM,
            gap: 3.0 * MM,
            length: 6.0 * MM,
            line_width: 0.25,
            crop_marks: true,
            registration_marks: true,
        }
    }
}

impl MarkOptions {
    fn validate(&self) -> Result<()> {
        for (name, v) in [
            ("sangrado", self.bleed),
            ("separación", self.gap),
            ("longitud", self.length),
        ] {
            if !(v.is_finite() && v >= 0.0) {
                return Err(Error::InvalidInput(format!("el valor de {name} no es válido")));
            }
        }
        if !(self.line_width.is_finite() && self.line_width > 0.0) {
            return Err(Error::InvalidInput("el grosor de las marcas debe ser positivo".into()));
        }
        Ok(())
    }
}

type Rect = [f32; 4];

fn rect_of(obj: &PdfObject) -> Option<Rect> {
    let mut r = [0.0; 4];
    for (i, slot) in r.iter_mut().enumerate() {
        *slot = obj.get_array(i as i32).ok().flatten()?.as_float().ok()?;
    }
    Some([r[0].min(r[2]), r[1].min(r[3]), r[0].max(r[2]), r[1].max(r[3])])
}

fn rect_src(r: Rect) -> String {
    format!("[{} {} {} {}]", r[0], r[1], r[2], r[3])
}

fn expand(r: Rect, by: f32) -> Rect {
    [r[0] - by, r[1] - by, r[2] + by, r[3] + by]
}

fn seg(s: &mut String, a: f32, b: f32, c: f32, d: f32) {
    s.push_str(&format!("{a} {b} m {c} {d} l S\n"));
}

/// Dibuja las marcas con el color de registro («All»), que sale en todas las planchas.
fn marks_stream(trim: Rect, o: &MarkOptions) -> String {
    let [x0, y0, x1, y1] = trim;
    let near = o.bleed + o.gap;
    let far = near + o.length;
    let mut s = format!("q /PliegoReg CS 1 SCN {} w\n", o.line_width);
    if o.crop_marks {
        for (x, sx) in [(x0, -1.0), (x1, 1.0)] {
            for (y, sy) in [(y0, -1.0), (y1, 1.0)] {
                seg(&mut s, x + sx * near, y, x + sx * far, y);
                seg(&mut s, x, y + sy * near, x, y + sy * far);
            }
        }
    }
    if o.registration_marks {
        let r = (o.length / 3.0).max(1.0);
        let mid = far - o.length / 2.0;
        let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        for (x, y) in [(cx, y0 - mid), (cx, y1 + mid), (x0 - mid, cy), (x1 + mid, cy)] {
            seg(&mut s, x - r * 1.5, y, x + r * 1.5, y);
            seg(&mut s, x, y - r * 1.5, x, y + r * 1.5);
            // Círculo con cuatro curvas de Bézier.
            let k = 0.5523 * r;
            s.push_str(&format!(
                "{} {y} m {} {} {} {} {x} {} c {} {} {} {} {} {y} c {} {} {} {} {x} {} c {} {} {} {} {} {y} c S\n",
                x + r,
                x + r,
                y + k,
                x + k,
                y + r,
                y + r,
                x - k,
                y + r,
                x - r,
                y + k,
                x - r,
                x - r,
                y - k,
                x - k,
                y - r,
                y - r,
                x + k,
                y - r,
                x + r,
                y - k,
                x + r,
            ));
        }
    }
    s.push_str("Q\n");
    s
}

fn push_stream(doc: &mut PdfDocument, parts: &mut PdfObject, text: &str) -> Result<()> {
    let s = doc
        .add_stream(&Buffer::from_bytes(text.as_bytes()).map_err(pdf_err)?, None, false)
        .map_err(pdf_err)?;
    parts.array_push(s).map_err(pdf_err)
}

/// Añade marcas de corte y de registro a todas las páginas (o solo a `pages`, desde 0) y amplía las cajas.
///
/// - `TrimBox` pasa a ser el área final (la que ya tenía, o la `CropBox`/`MediaBox` actual si no tenía).
/// - `BleedBox` = `TrimBox` + sangrado. El sangrado **no genera contenido**: el diseño debe llegar hasta ahí.
/// - `MediaBox` y `CropBox` crecen para que quepan las marcas.
pub fn add_printer_marks(
    doc: &mut PdfDocument,
    opts: &MarkOptions,
    pages: Option<&[usize]>,
    ctx: &JobContext<'_>,
) -> Result<usize> {
    opts.validate()?;
    let total = doc.page_count().map_err(pdf_err)?.max(0) as usize;
    let targets: Vec<usize> = match pages {
        Some(list) => {
            if let Some(bad) = list.iter().find(|p| **p >= total) {
                return Err(Error::InvalidInput(format!(
                    "la página {} no existe (el documento tiene {total})",
                    bad + 1
                )));
            }
            list.to_vec()
        }
        None => (0..total).collect(),
    };
    let reg_cs = doc
        .new_object_from_str(
            "[/Separation /All /DeviceCMYK << /FunctionType 2 /Domain [0 1] /C0 [0 0 0 0] /C1 [1 1 1 1] /N 1 >>]",
        )
        .map_err(pdf_err)?;
    let reg_cs = doc.add_object(&reg_cs).map_err(pdf_err)?;

    for (n, &p) in targets.iter().enumerate() {
        ctx.step(n as u64, targets.len() as u64, "Añadiendo marcas")?;
        let mut page = doc.find_page(p as i32).map_err(pdf_err)?;
        let media = page
            .get_dict_inheritable("MediaBox")
            .map_err(pdf_err)?
            .and_then(|m| rect_of(&m))
            .ok_or_else(|| Error::Pdf(format!("la página {} no tiene MediaBox", p + 1)))?;
        let crop = page
            .get_dict_inheritable("CropBox")
            .map_err(pdf_err)?
            .and_then(|m| rect_of(&m))
            .unwrap_or(media);
        let trim = page
            .get_dict("TrimBox")
            .map_err(pdf_err)?
            .and_then(|m| rect_of(&m))
            .unwrap_or(crop);
        let bleed = expand(trim, opts.bleed);
        let slug = expand(trim, opts.bleed + opts.gap + opts.length);
        // La página final cubre la unión de lo que ya tenía y el área de marcas.
        let new_media = [
            media[0].min(slug[0]),
            media[1].min(slug[1]),
            media[2].max(slug[2]),
            media[3].max(slug[3]),
        ];
        for (key, r) in [
            ("TrimBox", trim),
            ("BleedBox", bleed),
            ("MediaBox", new_media),
            ("CropBox", new_media),
        ] {
            let arr = doc.new_object_from_str(&rect_src(r)).map_err(pdf_err)?;
            page.dict_put(key, arr).map_err(pdf_err)?;
        }

        // Recursos propios de la página con el color de registro.
        let mut resources = match page.get_dict_inheritable("Resources").map_err(pdf_err)? {
            Some(r) => r.copy_dict().map_err(pdf_err)?,
            None => doc.new_dict().map_err(pdf_err)?,
        };
        let mut spaces = match resources.get_dict("ColorSpace").map_err(pdf_err)? {
            Some(c) => c.copy_dict().map_err(pdf_err)?,
            None => doc.new_dict().map_err(pdf_err)?,
        };
        spaces
            .dict_put("PliegoReg", reg_cs.try_clone().map_err(pdf_err)?)
            .map_err(pdf_err)?;
        resources.dict_put("ColorSpace", spaces).map_err(pdf_err)?;
        page.dict_put("Resources", resources).map_err(pdf_err)?;

        // Contenido: q + original + Q + marcas, para que el estado gráfico del original no afecte a las marcas.
        let mut parts = doc.new_array().map_err(pdf_err)?;
        push_stream(doc, &mut parts, "q\n")?;
        if let Some(original) = page.get_dict("Contents").map_err(pdf_err)? {
            if original.is_array().map_err(pdf_err)? {
                for i in 0..original.len().map_err(pdf_err)? {
                    if let Some(part) = original.get_array(i as i32).map_err(pdf_err)? {
                        parts.array_push(part).map_err(pdf_err)?;
                    }
                }
            } else {
                parts.array_push(original).map_err(pdf_err)?;
            }
        }
        push_stream(doc, &mut parts, "Q\n")?;
        push_stream(doc, &mut parts, &marks_stream(trim, opts))?;
        page.dict_put("Contents", parts).map_err(pdf_err)?;
    }
    ctx.step(targets.len() as u64, targets.len() as u64, "Marcas añadidas")?;
    Ok(targets.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crop_marks_sit_outside_the_bleed() {
        let o = MarkOptions {
            registration_marks: false,
            ..MarkOptions::default()
        };
        let s = marks_stream([100.0, 100.0, 200.0, 300.0], &o);
        // 4 esquinas × 2 líneas.
        assert_eq!(s.matches(" l S").count(), 8);
        // Las marcas nunca invaden el sangrado: la más cercana empieza a 'bleed + gap' del corte.
        let near = o.bleed + o.gap;
        assert!(s.contains(&format!("{} 100 m", 100.0 - near)), "{s}");
    }
}
