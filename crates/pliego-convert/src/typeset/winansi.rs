//! Unicode → Windows-1252 (WinAnsiEncoding de PDF).

/// Unicode de los códigos 0x80..=0x9F de cp1252 (0 = sin asignar).
const HIGH: [u32; 32] = [
    0x20AC, 0, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039, 0x0152, 0, 0x017D, 0, 0,
    0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014, 0x02DC, 0x2122, 0x0161, 0x203A, 0x0153, 0, 0x017E, 0x0178,
];

/// Código WinAnsi de `c`, o `None` si no tiene.
pub(super) fn encode(c: char) -> Option<u8> {
    let u = c as u32;
    match u {
        0x20..=0x7E | 0xA0..=0xFF => Some(u as u8),
        _ => HIGH.iter().position(|&h| h == u).map(|i| 0x80 + i as u8),
    }
}

/// Transcripción de signos tipográficos que cp1252 no tiene.
pub(super) fn transliterate(c: char) -> Option<&'static str> {
    Some(match c {
        '\u{2212}' => "-",                           // signo menos
        '\u{2010}' | '\u{2011}' | '\u{2012}' => "-", // guiones
        '\u{2015}' => "\u{2014}",                    // barra horizontal
        '\u{2192}' => "->",
        '\u{2190}' => "<-",
        '\u{2194}' => "<->",
        '\u{21D2}' => "=>",
        '\u{2264}' => "<=",
        '\u{2265}' => ">=",
        '\u{2260}' => "!=",
        '\u{2248}' => "~",
        '\u{00D7}' => "x",
        '\u{2026}' => "...",
        '\u{2713}' | '\u{2714}' => "v",
        '\u{25CF}' | '\u{25E6}' | '\u{25AA}' | '\u{25A0}' => "\u{2022}",
        '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{FEFF}' | '\u{00AD}' => "",
        '\u{2028}' | '\u{2029}' => "\n",
        '\u{2002}' | '\u{2003}' | '\u{2009}' | '\u{202F}' => " ",
        _ => return None,
    })
}
