use mupdf::Font;
use pliego_core::{Error, Result};

use crate::pdf_err;

/// Convierte a Latin-1 (lo que no cabe pasa a `?`).
pub fn latin1(s: &str) -> Vec<u8> {
    s.chars().map(|c| u8::try_from(u32::from(c)).unwrap_or(b'?')).collect()
}

/// Cadena literal PDF `( ... )` en bytes, con escapes.
pub fn pdf_string(s: &str) -> Vec<u8> {
    let mut out = vec![b'('];
    for b in latin1(s) {
        match b {
            b'(' | b')' | b'\\' => out.extend([b'\\', b]),
            0x20..=0x7e => out.push(b),
            _ => out.extend(format!("\\{b:03o}").bytes()),
        }
    }
    out.push(b')');
    out
}

/// Medidor de ancho de texto con una fuente estándar.
pub struct Measurer(Font);

impl Measurer {
    pub fn new(font: &str) -> Result<Self> {
        Font::new(font)
            .map(Self)
            .map_err(|_| Error::InvalidInput(format!("fuente no disponible: {font}")))
    }

    /// Ancho en puntos del texto a ese tamaño.
    pub fn width(&self, text: &str, size: f32) -> Result<f32> {
        let mut total = 0.0;
        for b in latin1(text) {
            let g = self.0.encode_character(i32::from(b)).map_err(pdf_err)?;
            total += self.0.advance_glyph(g).map_err(pdf_err)?;
        }
        Ok(total * size)
    }
}

/// Número real para un flujo de contenido.
pub fn num(v: f32) -> String {
    let s = format!("{v:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Fecha `AAAA-MM-DD` a partir de días desde 1970-01-01 (algoritmo civil de Howard Hinnant).
pub fn iso_date(days: u64) -> String {
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        assert_eq!(iso_date(0), "1970-01-01");
        assert_eq!(iso_date(19_723), "2024-01-01");
        assert_eq!(iso_date(20_517), "2026-03-05");
    }

    #[test]
    fn strings_escape_and_latin1() {
        assert_eq!(pdf_string("a(b)\\"), b"(a\\(b\\)\\\\)".to_vec());
        assert_eq!(pdf_string("ñ€"), b"(\\361?)".to_vec());
    }

    #[test]
    fn numbers() {
        assert_eq!(num(12.5), "12.5");
        assert_eq!(num(3.0), "3");
    }
}
