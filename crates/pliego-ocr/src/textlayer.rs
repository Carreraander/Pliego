//! Capa de texto invisible: genera el contenido PDF (modo de render 3) que se superpone a la imagen escaneada.
//!
//! Se usa Courier (fuente estándar, sin incrustar): todos sus glifos miden 0,6 em, así que el ancho de cada palabra
//! se ajusta exactamente a su caja con el operador `Tz`, y la selección en el visor coincide con lo escaneado.
//! Codificación WinAnsi: cubre español, francés, portugués, italiano, alemán e inglés; los caracteres fuera de ella
//! se sustituyen por `?` (la selección sigue funcionando, el texto copiado pierde ese carácter).

use std::fmt::Write as _;

/// Nombre del recurso de fuente que se añade a la página.
pub const FONT_RESOURCE: &str = "PlgOcr";

/// Palabra ya situada en el espacio de usuario de la página.
#[derive(Debug, Clone)]
pub struct PlacedWord {
    pub text: String,
    /// Origen de la línea base (esquina inferior izquierda de la palabra).
    pub origin: (f32, f32),
    /// Dirección del texto (vector unitario) y «arriba» (vector unitario), en espacio de usuario.
    pub dir: (f32, f32),
    pub up: (f32, f32),
    /// Tamaño de fuente en puntos y ancho que debe ocupar la palabra, en puntos.
    pub font_size: f32,
    pub width: f32,
}

/// Byte WinAnsi (Windows-1252) de un carácter, si existe.
pub fn win_ansi(c: char) -> Option<u8> {
    match c as u32 {
        0x20..=0x7e | 0xa0..=0xff => Some(c as u32 as u8),
        0x20ac => Some(0x80),
        0x201a => Some(0x82),
        0x0192 => Some(0x83),
        0x201e => Some(0x84),
        0x2026 => Some(0x85),
        0x2020 => Some(0x86),
        0x2021 => Some(0x87),
        0x02c6 => Some(0x88),
        0x2030 => Some(0x89),
        0x0160 => Some(0x8a),
        0x2039 => Some(0x8b),
        0x0152 => Some(0x8c),
        0x017d => Some(0x8e),
        0x2018 => Some(0x91),
        0x2019 => Some(0x92),
        0x201c => Some(0x93),
        0x201d => Some(0x94),
        0x2022 => Some(0x95),
        0x2013 => Some(0x96),
        0x2014 => Some(0x97),
        0x02dc => Some(0x98),
        0x2122 => Some(0x99),
        0x0161 => Some(0x9a),
        0x203a => Some(0x9b),
        0x0153 => Some(0x9c),
        0x017e => Some(0x9e),
        0x0178 => Some(0x9f),
        _ => None,
    }
}

/// Cadena literal PDF `(...)` con escapes, en WinAnsi.
fn pdf_string(text: &str, out: &mut Vec<u8>) {
    out.push(b'(');
    for c in text.chars() {
        let b = win_ansi(c).unwrap_or(b'?');
        match b {
            b'(' | b')' | b'\\' => {
                out.push(b'\\');
                out.push(b);
            }
            b'\r' | b'\n' => out.push(b' '),
            _ => out.push(b),
        }
    }
    out.push(b')');
}

/// Número de caracteres que ocupa `text` en Courier (cada carácter, sea cual sea, una celda).
pub fn cell_count(text: &str) -> usize {
    text.chars().count()
}

/// Flujo de contenido con todas las palabras, listo para añadir al final de la página.
pub fn build_content(words: &[PlacedWord]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    out.extend_from_slice(format!("q BT /{FONT_RESOURCE} 1 Tf 3 Tr\n").as_bytes());
    for w in words {
        let n = cell_count(&w.text);
        if n == 0 || w.font_size <= 0.0 || w.width <= 0.0 {
            continue;
        }
        // Ancho natural de la palabra con Tf=1: 0,6·n·fs; se escala horizontalmente hasta `width`.
        let tz = 100.0 * w.width / (0.6 * w.font_size * n as f32);
        let mut head = String::new();
        let _ = write!(
            head,
            "{:.2} Tz {:.4} {:.4} {:.4} {:.4} {:.3} {:.3} Tm ",
            tz.clamp(1.0, 2000.0),
            w.dir.0 * w.font_size,
            w.dir.1 * w.font_size,
            w.up.0 * w.font_size,
            w.up.1 * w.font_size,
            w.origin.0,
            w.origin.1
        );
        out.extend_from_slice(head.as_bytes());
        // Un espacio final para que la extracción de texto separe las palabras.
        pdf_string(&format!("{} ", w.text), &mut out);
        out.extend_from_slice(b" Tj\n");
    }
    out.extend_from_slice(b"ET Q\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_spanish_and_escapes() {
        let mut v = Vec::new();
        pdf_string("camión (año) \\ €", &mut v);
        assert_eq!(v, b"(cami\xf3n \\(a\xf1o\\) \\\\ \x80)");
        let mut v = Vec::new();
        pdf_string("日本", &mut v);
        assert_eq!(v, b"(??)");
    }

    #[test]
    fn content_scales_width() {
        let w = PlacedWord {
            text: "Hola".into(),
            origin: (10.0, 20.0),
            dir: (1.0, 0.0),
            up: (0.0, 1.0),
            font_size: 10.0,
            width: 24.0,
        };
        let s = String::from_utf8(build_content(&[w])).unwrap();
        // 4 caracteres · 0,6 · 10 = 24 pt → 100 %.
        assert!(
            s.contains("100.00 Tz 10.0000 0.0000 0.0000 10.0000 10.000 20.000 Tm (Hola ) Tj"),
            "{s}"
        );
        assert!(s.starts_with("q BT /PlgOcr 1 Tf 3 Tr"));
        assert!(s.ends_with("ET Q\n"));
    }
}
