use std::str::FromStr;

use pliego_core::{Error, Result};
use pliego_pdf::{PdfDocument, Quad, TextFlags};
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::validators;

/// Qué buscar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Pattern {
    /// DNI con letra de control válida.
    Dni,
    /// NIE con letra de control válida.
    Nie,
    /// CIF con dígito o letra de control válidos.
    Cif,
    /// IBAN con dígitos de control válidos (cualquier país).
    Iban,
    Email,
    /// Teléfono español (fijo o móvil), con o sin prefijo +34.
    Phone,
    /// Fechas numéricas (12/03/2024, 12-03-24) y escritas (12 de marzo de 2024).
    Date,
    /// Expresión regular propia (sintaxis de la crate `regex`).
    Regex(String),
}

impl Pattern {
    pub fn name(&self) -> String {
        match self {
            Pattern::Dni => "dni".into(),
            Pattern::Nie => "nie".into(),
            Pattern::Cif => "cif".into(),
            Pattern::Iban => "iban".into(),
            Pattern::Email => "email".into(),
            Pattern::Phone => "telefono".into(),
            Pattern::Date => "fecha".into(),
            Pattern::Regex(r) => format!("regex:{r}"),
        }
    }
}

impl FromStr for Pattern {
    type Err = String;

    /// `dni`, `nie`, `cif`, `iban`, `email`, `telefono`, `fecha` o `regex:<expresión>`.
    fn from_str(s: &str) -> std::result::Result<Self, String> {
        if let Some(r) = s.strip_prefix("regex:") {
            return Ok(Pattern::Regex(r.to_owned()));
        }
        match s.to_lowercase().as_str() {
            "dni" => Ok(Pattern::Dni),
            "nie" => Ok(Pattern::Nie),
            "cif" => Ok(Pattern::Cif),
            "iban" => Ok(Pattern::Iban),
            "email" | "correo" => Ok(Pattern::Email),
            "telefono" | "teléfono" | "phone" => Ok(Pattern::Phone),
            "fecha" | "date" => Ok(Pattern::Date),
            other => Err(format!(
                "patrón desconocido «{other}»; usa dni, nie, cif, iban, email, telefono, fecha o regex:<expresión>"
            )),
        }
    }
}

/// Zona de una página, en puntos y en el sistema de coordenadas de la página tal como la ve MuPDF
/// (origen arriba a la izquierda, y hacia abajo).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Area {
    /// Página desde 0.
    pub page: u32,
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

/// Una coincidencia encontrada, con las zonas que ocupa (una por línea).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Match {
    /// Página desde 0.
    pub page: u32,
    pub pattern: String,
    pub text: String,
    /// Zona completa de cada línea (la caja que da el extractor).
    pub areas: Vec<Area>,
    /// Banda central de cada línea, de la misma longitud que `areas`: la caja del extractor es más alta que los
    /// glifos y tocaría las líneas vecinas, así que al redactar se usa esta, recortada por los lados «de arriba» y
    /// «de abajo» del texto. En texto vertical (página girada 90°/270° o texto girado) el recorte va en horizontal,
    /// nunca a lo largo de la línea: eso dejaría sin cubrir los extremos del dato.
    #[serde(default)]
    pub core: Vec<Area>,
}

/// Caja de una línea de una coincidencia: número de línea, límites (x0, y0, x1, y1) y si avanza en vertical.
type LineBox = (usize, (f32, f32, f32, f32), bool);

/// Fracción de la altura de la línea que se recorta por cada lado en [`Match::core`].
const CORE_INSET: f32 = 0.25;

struct Compiled {
    pattern: Pattern,
    regex: Regex,
}

fn compile(patterns: &[Pattern]) -> Result<Vec<Compiled>> {
    patterns
        .iter()
        .map(|p| {
            let src = match p {
                Pattern::Dni => r"\b\d{8}[ .-]?[A-Za-z]\b".to_owned(),
                Pattern::Nie => r"\b[XYZxyz][ .-]?\d{7}[ .-]?[A-Za-z]\b".to_owned(),
                Pattern::Cif => r"\b[A-HJNP-SUVWa-hjnp-suvw][ .-]?\d{7}[ .-]?[0-9A-Ja-j]\b".to_owned(),
                Pattern::Iban => r"\b[A-Za-z]{2}\d{2}(?:[ ]?[A-Za-z0-9]{4}){2,7}(?:[A-Za-z0-9]{1,3})?\b".to_owned(),
                Pattern::Email => r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}\b".to_owned(),
                Pattern::Phone => r"(?:(?:\+|00)34[ .-]?)?\b[6789]\d{2}[ .-]?\d{3}[ .-]?\d{3}\b".to_owned(),
                Pattern::Date => concat!(
                    r"\b(?:0?[1-9]|[12]\d|3[01])[/.-](?:0?[1-9]|1[0-2])[/.-](?:\d{4}|\d{2})\b",
                    r"|\b(?:0?[1-9]|[12]\d|3[01]) de (?:enero|febrero|marzo|abril|mayo|junio|julio|agosto|septiembre|setiembre|octubre|noviembre|diciembre) de \d{4}\b"
                )
                .to_owned(),
                Pattern::Regex(r) => r.clone(),
            };
            let regex = regex::RegexBuilder::new(&src)
                .case_insensitive(matches!(p, Pattern::Date))
                .build()
                .map_err(|e| Error::InvalidInput(format!("expresión regular no válida «{src}»: {e}")))?;
            Ok(Compiled { pattern: p.clone(), regex })
        })
        .collect()
}

fn accepts(p: &Pattern, text: &str) -> bool {
    match p {
        Pattern::Dni => validators::is_valid_dni(text),
        Pattern::Nie => validators::is_valid_nie(text),
        Pattern::Cif => validators::is_valid_cif(text),
        Pattern::Iban => validators::is_valid_iban(text),
        _ => true,
    }
}

struct Glyph {
    ch: char,
    quad: Option<Quad>,
    line: usize,
    /// ¿La línea de este glifo avanza en vertical en el espacio de página?
    vertical: bool,
}

fn quad_bounds(q: &Quad) -> (f32, f32, f32, f32) {
    let xs = [q.ul.x, q.ur.x, q.ll.x, q.lr.x];
    let ys = [q.ul.y, q.ur.y, q.ll.y, q.lr.y];
    (
        xs.iter().copied().fold(f32::MAX, f32::min),
        ys.iter().copied().fold(f32::MAX, f32::min),
        xs.iter().copied().fold(f32::MIN, f32::max),
        ys.iter().copied().fold(f32::MIN, f32::max),
    )
}

/// ¿La línea avanza en vertical? Se mide por el desplazamiento del primer al último glifo: las cajas de los glifos
/// salen siempre alineadas con los ejes, así que su forma no dice hacia dónde va el texto (una `I` girada y una `I`
/// recta se parecen). Una línea de un solo glifo cuenta como horizontal.
fn advances_vertically<'a>(quads: impl Iterator<Item = &'a Quad>) -> bool {
    let centre = |q: &Quad| ((q.ul.x + q.lr.x) / 2.0, (q.ul.y + q.lr.y) / 2.0);
    let mut it = quads.map(centre);
    let Some(first) = it.next() else { return false };
    let last = it.last().unwrap_or(first);
    (last.1 - first.1).abs() > (last.0 - first.0).abs()
}

/// Banda central de una línea: se recorta [`CORE_INSET`] por el lado de arriba y el de abajo del texto, que en una
/// línea vertical son el izquierdo y el derecho.
fn core_band(page: u32, b: (f32, f32, f32, f32), vertical: bool) -> Area {
    let (mut x0, mut y0, mut x1, mut y1) = b;
    if vertical {
        let d = (x1 - x0) * CORE_INSET;
        x0 += d;
        x1 -= d;
    } else {
        let d = (y1 - y0) * CORE_INSET;
        y0 += d;
        y1 -= d;
    }
    Area { page, x0, y0, x1, y1 }
}

/// Busca los patrones en todo el documento. Las coincidencias de DNI, NIE, CIF e IBAN solo se
/// devuelven si pasan la validación del dígito o letra de control.
pub fn find_patterns(doc: &PdfDocument, patterns: &[Pattern]) -> Result<Vec<Match>> {
    let compiled = compile(patterns)?;
    let pages = doc.page_count()?.max(0);
    let mut out = Vec::new();
    for p in 0..pages {
        let page = doc.load_page(p)?;
        let tp = page.to_text_page(TextFlags::empty())?;

        // Texto de la página con la posición de cada carácter.
        let mut glyphs: Vec<Glyph> = Vec::new();
        let mut line_no = 0usize;
        for block in tp.blocks() {
            for line in &block.lines {
                let vertical = advances_vertically(line.chars.iter().map(|c| &c.quad));
                for c in &line.chars {
                    if let Some(ch) = c.ch {
                        glyphs.push(Glyph {
                            ch,
                            quad: Some(c.quad),
                            line: line_no,
                            vertical,
                        });
                    }
                }
                glyphs.push(Glyph {
                    ch: '\n',
                    quad: None,
                    line: line_no,
                    vertical,
                });
                line_no += 1;
            }
        }
        let mut text = String::new();
        let mut starts = Vec::with_capacity(glyphs.len());
        for g in &glyphs {
            starts.push(text.len());
            text.push(g.ch);
        }

        for c in &compiled {
            for m in c.regex.find_iter(&text) {
                if !accepts(&c.pattern, m.as_str()) {
                    continue;
                }
                let first = starts.partition_point(|&s| s < m.start());
                let last = starts.partition_point(|&s| s < m.end());
                let mut lines: Vec<LineBox> = Vec::new();
                for g in &glyphs[first..last] {
                    let Some(q) = &g.quad else { continue };
                    if g.ch.is_whitespace() {
                        continue;
                    }
                    let b = quad_bounds(q);
                    match lines.last_mut() {
                        Some((l, r, _)) if *l == g.line => {
                            *r = (r.0.min(b.0), r.1.min(b.1), r.2.max(b.2), r.3.max(b.3));
                        }
                        _ => lines.push((g.line, b, g.vertical)),
                    }
                }
                if lines.is_empty() {
                    continue;
                }
                out.push(Match {
                    page: p as u32,
                    pattern: c.pattern.name(),
                    text: m.as_str().to_owned(),
                    areas: lines
                        .iter()
                        .map(|(_, b, _)| Area {
                            page: p as u32,
                            x0: b.0,
                            y0: b.1,
                            x1: b.2,
                            y1: b.3,
                        })
                        .collect(),
                    core: lines
                        .iter()
                        .map(|(_, b, vertical)| core_band(p as u32, *b, *vertical))
                        .collect(),
                });
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_names() {
        assert_eq!("DNI".parse::<Pattern>().unwrap(), Pattern::Dni);
        assert_eq!("regex:a+b".parse::<Pattern>().unwrap(), Pattern::Regex("a+b".into()));
        assert!("foo".parse::<Pattern>().is_err());
    }

    #[test]
    fn bad_regex_is_input_error() {
        assert!(matches!(
            compile(&[Pattern::Regex("(".into())]),
            Err(Error::InvalidInput(_))
        ));
    }

    fn hits(p: Pattern, text: &str) -> Vec<String> {
        let c = compile(std::slice::from_ref(&p)).unwrap();
        c[0].regex
            .find_iter(text)
            .filter(|m| accepts(&p, m.as_str()))
            .map(|m| m.as_str().to_owned())
            .collect()
    }

    #[test]
    fn regexes_with_validators() {
        assert_eq!(hits(Pattern::Dni, "DNI 12345678Z y 12345678A"), ["12345678Z"]);
        assert_eq!(hits(Pattern::Nie, "NIE X1234567L, X1234567A"), ["X1234567L"]);
        assert_eq!(hits(Pattern::Cif, "CIF A58818501 / A58818502"), ["A58818501"]);
        assert_eq!(
            hits(Pattern::Iban, "IBAN ES91 2100 0418 4502 0005 1332 fin"),
            ["ES91 2100 0418 4502 0005 1332"]
        );
        assert_eq!(
            hits(Pattern::Email, "escribe a ana.lopez@ejemplo.es."),
            ["ana.lopez@ejemplo.es"]
        );
        assert_eq!(
            hits(Pattern::Phone, "tel +34 612 345 678 o 91 234"),
            ["+34 612 345 678"]
        );
        assert_eq!(
            hits(Pattern::Date, "el 12/03/2024 y el 5 de marzo de 2023, no 32/13/2024"),
            ["12/03/2024", "5 de marzo de 2023"]
        );
    }
}
