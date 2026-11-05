//! Medidas y codificación de texto para las apariencias generadas (fuentes estándar, WinAnsi).
//!
//! Las anchuras de Helvetica y Times-Roman son las publicadas en las tablas AFM de las 14 fuentes estándar
//! (datos de la especificación, no de ninguna implementación). Las variantes en negrita se aproximan; solo se
//! usan para repartir líneas, nunca para posicionar glifos de un documento ajeno.

/// Anchura (milésimas de em) de los caracteres ASCII 32..=126 de Helvetica.
const HELVETICA: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278,
    278, // ␠ ! " # $ % & ' ( ) * + , - . /
    556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556, // 0-9 : ; < = > ?
    1015, 667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833, 722, 778, // @ A-O
    667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 278, 278, 278, 469, 556, // P-Z [ \ ] ^ _
    333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, 556, 556, // ` a-o
    556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584, // p-z { | } ~
];

/// Lo mismo para Times-Roman.
const TIMES: [u16; 95] = [
    250, 333, 408, 500, 500, 833, 778, 180, 333, 333, 500, 564, 250, 333, 250, 278, //
    500, 500, 500, 500, 500, 500, 500, 500, 500, 500, 278, 278, 564, 564, 564, 444, //
    921, 722, 667, 667, 722, 611, 556, 722, 722, 333, 389, 722, 611, 889, 722, 722, //
    556, 722, 667, 556, 611, 722, 722, 944, 722, 722, 611, 333, 278, 333, 469, 500, //
    333, 444, 500, 444, 500, 444, 333, 500, 500, 278, 278, 500, 278, 778, 500, 500, //
    500, 500, 333, 389, 278, 500, 500, 722, 500, 500, 444, 480, 200, 480, 541, //
];

/// Letra ASCII base de un carácter latino acentuado (para medirlo igual que la letra sin acento).
fn base_letter(c: char) -> Option<char> {
    Some(match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
        'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' => 'A',
        'è' | 'é' | 'ê' | 'ë' => 'e',
        'È' | 'É' | 'Ê' | 'Ë' => 'E',
        'ì' | 'í' | 'î' | 'ï' => 'i',
        'Ì' | 'Í' | 'Î' | 'Ï' => 'I',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' => 'o',
        'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' | 'Ø' => 'O',
        'ù' | 'ú' | 'û' | 'ü' => 'u',
        'Ù' | 'Ú' | 'Û' | 'Ü' => 'U',
        'ñ' => 'n',
        'Ñ' => 'N',
        'ç' => 'c',
        'Ç' => 'C',
        'ý' | 'ÿ' => 'y',
        'Ý' => 'Y',
        _ => return None,
    })
}

fn width_1000(base_font: &str, c: char) -> f32 {
    let bold = base_font.contains("Bold");
    let scale = if bold { 1.06 } else { 1.0 };
    if base_font.starts_with("Courier") {
        return 600.0;
    }
    let table = if base_font.starts_with("Times") {
        &TIMES
    } else {
        &HELVETICA
    };
    let c = base_letter(c).unwrap_or(c);
    let w = if (' '..='~').contains(&c) {
        f32::from(table[c as usize - 32])
    } else if c == '\u{a0}' {
        f32::from(table[0])
    } else if table[0] == 250 {
        500.0
    } else {
        556.0
    };
    w * scale
}

/// Anchura del texto en puntos para una de las 14 fuentes estándar.
pub(crate) fn text_width(base_font: &str, text: &str, size: f32) -> f32 {
    text.chars().map(|c| width_1000(base_font, c)).sum::<f32>() * size / 1000.0
}

/// Reparte `text` en líneas de ancho máximo `max_width`: respeta saltos de línea y corta por palabras (y por
/// caracteres si una palabra sola no cabe).
pub(crate) fn wrap(text: &str, base_font: &str, size: f32, max_width: f32) -> Vec<String> {
    let mut lines = Vec::new();
    for para in text.replace("\r\n", "\n").replace('\r', "\n").split('\n') {
        let mut cur = String::new();
        for word in para.split(' ') {
            let candidate = if cur.is_empty() {
                word.to_string()
            } else {
                format!("{cur} {word}")
            };
            if text_width(base_font, &candidate, size) <= max_width || cur.is_empty() && word.is_empty() {
                cur = candidate;
                continue;
            }
            if !cur.is_empty() {
                lines.push(std::mem::take(&mut cur));
            }
            // La palabra sola puede no caber: se parte por caracteres.
            let mut piece = String::new();
            for ch in word.chars() {
                piece.push(ch);
                if text_width(base_font, &piece, size) > max_width && piece.chars().count() > 1 {
                    let last = piece.pop().unwrap_or(' ');
                    lines.push(std::mem::take(&mut piece));
                    piece.push(last);
                }
            }
            cur = piece;
        }
        lines.push(cur);
    }
    lines
}

/// Texto en WinAnsi (CP1252). Lo que no existe en la codificación se sustituye por `?`.
pub(crate) fn winansi(s: &str) -> Vec<u8> {
    s.chars()
        .map(|c| match c as u32 {
            0x20..=0x7e | 0xa0..=0xff => c as u32 as u8,
            0x20ac => 0x80,
            0x201a => 0x82,
            0x0192 => 0x83,
            0x201e => 0x84,
            0x2026 => 0x85,
            0x2020 => 0x86,
            0x2021 => 0x87,
            0x02c6 => 0x88,
            0x2030 => 0x89,
            0x0160 => 0x8a,
            0x2039 => 0x8b,
            0x0152 => 0x8c,
            0x017d => 0x8e,
            0x2018 => 0x91,
            0x2019 => 0x92,
            0x201c => 0x93,
            0x201d => 0x94,
            0x2022 => 0x95,
            0x2013 => 0x96,
            0x2014 => 0x97,
            0x02dc => 0x98,
            0x2122 => 0x99,
            0x0161 => 0x9a,
            0x203a => 0x9b,
            0x0153 => 0x9c,
            0x017e => 0x9e,
            0x0178 => 0x9f,
            0x09 | 0x0a | 0x0d => b' ',
            _ => b'?',
        })
        .collect()
}

/// Carácter Unicode de un byte WinAnsi (inversa de [`winansi`]); `None` si el byte no está definido.
pub(crate) fn winansi_char(b: u8) -> Option<char> {
    const HIGH: [u32; 32] = [
        0x20ac, 0, 0x201a, 0x0192, 0x201e, 0x2026, 0x2020, 0x2021, 0x02c6, 0x2030, 0x0160, 0x2039, 0x0152, 0, 0x017d,
        0, 0, 0x2018, 0x2019, 0x201c, 0x201d, 0x2022, 0x2013, 0x2014, 0x02dc, 0x2122, 0x0161, 0x203a, 0x0153, 0,
        0x017e, 0x0178,
    ];
    match b {
        0x20..=0x7e | 0xa0..=0xff => Some(char::from(b)),
        0x80..=0x9f => char::from_u32(HIGH[usize::from(b - 0x80)]).filter(|c| *c != '\0'),
        _ => None,
    }
}

/// Contenido de una cadena literal PDF (sin los paréntesis): escapa `\`, `(`, `)` y los bytes no ASCII.
pub(crate) fn escape(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len());
    for &b in bytes {
        match b {
            b'(' | b')' | b'\\' => {
                out.push('\\');
                out.push(b as char);
            }
            0x20..=0x7e => out.push(b as char),
            _ => out.push_str(&format!("\\{b:03o}")),
        }
    }
    out
}

/// `BaseFont` estándar que corresponde a un nombre de recurso de `/DA` (`Helv`, `TiRo`, `Cour`, `HeBo`…).
pub(crate) fn base_font_for(resource_name: &str) -> &'static str {
    match resource_name {
        n if n.starts_with("HeBo") || n.eq_ignore_ascii_case("Helvetica-Bold") => "Helvetica-Bold",
        n if n.starts_with("TiBo") || n.eq_ignore_ascii_case("Times-Bold") => "Times-Bold",
        n if n.starts_with("TiRo") || n.starts_with("Times") => "Times-Roman",
        n if n.starts_with("CoBo") => "Courier-Bold",
        n if n.starts_with("Cour") => "Courier",
        _ => "Helvetica",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helvetica_widths_match_the_afm() {
        // «Hello» en Helvetica a 10 pt: H 722 + e 556 + l 222 + l 222 + o 556 = 2278 → 22,78 pt.
        assert!((text_width("Helvetica", "Hello", 10.0) - 22.78).abs() < 0.01);
        // Courier es monoespaciada.
        assert!((text_width("Courier", "abcd", 10.0) - 24.0).abs() < 1e-3);
    }

    #[test]
    fn wrap_breaks_on_words_and_newlines() {
        let lines = wrap("uno dos tres\ncuatro", "Helvetica", 10.0, 40.0);
        assert!(lines.len() >= 3, "{lines:?}");
        assert_eq!(lines.last().unwrap(), "cuatro");
        // Una palabra que no cabe se parte.
        let long = wrap("abcdefghijklmnopqrstuvwxyz", "Helvetica", 10.0, 50.0);
        assert!(long.len() > 1 && long.iter().all(|l| text_width("Helvetica", l, 10.0) <= 50.0 + 6.0));
    }

    #[test]
    fn winansi_maps_accents_and_specials() {
        assert_eq!(winansi("añ€"), vec![b'a', 0xf1, 0x80]);
        assert_eq!(winansi("日"), vec![b'?']);
        assert_eq!(winansi_char(0x80), Some('€'));
        assert_eq!(winansi_char(0x81), None);
        assert_eq!(winansi_char(0xf1), Some('ñ'));
        for b in 0x20..=0xffu8 {
            if let Some(c) = winansi_char(b) {
                assert_eq!(winansi(&c.to_string()), vec![b]);
            }
        }
        assert_eq!(escape(b"a(b)\\\xf1"), "a\\(b\\)\\\\\\361");
    }
}
