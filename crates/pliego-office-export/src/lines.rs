//! Fragmentos (líneas del PDF) y filas: la materia prima del análisis de maquetación.

use pliego_convert::{BBox, Span};

use crate::model::{ListKind, Run};

/// Una línea del texto estructurado de MuPDF.
#[derive(Debug, Clone)]
pub(crate) struct Frag {
    pub bbox: BBox,
    pub spans: Vec<Span>,
}

impl Frag {
    pub fn text(&self) -> String {
        self.spans.iter().map(|s| s.text.as_str()).collect()
    }
    pub fn width(&self) -> f32 {
        self.bbox.x1 - self.bbox.x0
    }
    pub fn height(&self) -> f32 {
        self.bbox.y1 - self.bbox.y0
    }
    pub fn runs(&self) -> Vec<Run> {
        self.spans.iter().map(Run::from_span).collect()
    }
    /// Tamaño de fuente dominante (por número de caracteres).
    pub fn size(&self) -> f32 {
        self.spans
            .iter()
            .max_by_key(|s| s.text.chars().count())
            .map(|s| s.size)
            .unwrap_or(10.0)
    }
    /// `true` si la mayor parte del texto está en negrita.
    pub fn mostly_bold(&self) -> bool {
        let total: usize = self.spans.iter().map(|s| s.text.chars().count()).sum();
        let bold: usize = self
            .spans
            .iter()
            .filter(|s| s.bold)
            .map(|s| s.text.chars().count())
            .sum();
        total > 0 && bold * 2 > total
    }
}

/// Fragmentos que comparten línea visual, de izquierda a derecha.
#[derive(Debug, Clone)]
pub(crate) struct Row {
    pub bbox: BBox,
    pub frags: Vec<Frag>,
}

pub(crate) fn union(a: BBox, b: BBox) -> BBox {
    BBox {
        x0: a.x0.min(b.x0),
        y0: a.y0.min(b.y0),
        x1: a.x1.max(b.x1),
        y1: a.y1.max(b.y1),
    }
}

/// Agrupa fragmentos en filas por solape vertical y ordena cada fila por x.
pub(crate) fn cluster_rows(mut frags: Vec<Frag>) -> Vec<Row> {
    frags.sort_by(|a, b| {
        let ca = (a.bbox.y0 + a.bbox.y1) / 2.0;
        let cb = (b.bbox.y0 + b.bbox.y1) / 2.0;
        ca.total_cmp(&cb).then(a.bbox.x0.total_cmp(&b.bbox.x0))
    });
    let mut rows: Vec<Row> = Vec::new();
    // Centro y alto del primer fragmento de cada fila: referencia fija, para que una fila no «crezca» y
    // absorba la siguiente cuando mezcla cuerpos de distinto tamaño.
    let mut refs: Vec<(f32, f32)> = Vec::new();
    for f in frags {
        let cy = (f.bbox.y0 + f.bbox.y1) / 2.0;
        if let (Some(row), Some(&(rcy, rh))) = (rows.last_mut(), refs.last())
            && (cy - rcy).abs() < 0.5 * f.height().min(rh)
        {
            row.bbox = union(row.bbox, f.bbox);
            row.frags.push(f);
            continue;
        }
        refs.push((cy, f.height()));
        rows.push(Row {
            bbox: f.bbox,
            frags: vec![f],
        });
    }
    for r in &mut rows {
        r.frags.sort_by(|a, b| a.bbox.x0.total_cmp(&b.bbox.x0));
    }
    rows
}

const BULLETS: &[char] = &['•', '▪', '◦', '●', '○', '■', '□', '–', '—', '-', '·', '*', '‣', '▫'];

/// Marcador de lista al inicio de `text`: tipo, número y longitud en bytes (incluye el espacio posterior).
pub(crate) fn parse_marker(text: &str) -> Option<(ListKind, u32, usize)> {
    let lead = text.len() - text.trim_start().len();
    let t = &text[lead..];
    let mut chars = t.char_indices();
    let (_, c) = chars.next()?;
    if BULLETS.contains(&c) {
        let after = lead + c.len_utf8();
        let rest = &text[after..];
        if rest.is_empty() || rest.starts_with(char::is_whitespace) {
            let sp = rest.len() - rest.trim_start().len();
            return Some((ListKind::Bullet, 0, after + sp));
        }
        return None;
    }
    if c.is_ascii_digit() {
        let digits: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
        if digits.len() > 3 {
            return None;
        }
        let rest = &t[digits.len()..];
        let mut rc = rest.chars();
        let sep = rc.next()?;
        if sep != '.' && sep != ')' {
            return None;
        }
        let rest2 = &rest[1..];
        if rest2.is_empty() || rest2.starts_with(char::is_whitespace) {
            let sp = rest2.len() - rest2.trim_start().len();
            return Some((ListKind::Decimal, digits.parse().ok()?, lead + digits.len() + 1 + sp));
        }
    }
    None
}

/// `true` si el texto es solo un marcador de lista (`•`, `1.`...).
pub(crate) fn is_marker_only(text: &str) -> bool {
    match parse_marker(text) {
        Some((_, _, len)) => text[len..].trim().is_empty(),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markers() {
        assert_eq!(parse_marker("• hola"), Some((ListKind::Bullet, 0, "• ".len())));
        assert_eq!(parse_marker("- hola"), Some((ListKind::Bullet, 0, 2)));
        assert_eq!(parse_marker("-hola"), None);
        assert_eq!(
            parse_marker("12. texto").map(|m| (m.0, m.1)),
            Some((ListKind::Decimal, 12))
        );
        assert_eq!(parse_marker("3) texto").map(|m| m.1), Some(3));
        assert_eq!(parse_marker("3.5 texto"), None);
        assert_eq!(parse_marker("2026 texto"), None);
        assert!(is_marker_only("•"));
        assert!(is_marker_only("1."));
        assert!(!is_marker_only("1. a"));
    }
}
