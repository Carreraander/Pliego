//! Fuentes estándar y codificación WinAnsi (Windows-1252).

use pliego_core::{Error, Result};
use pliego_pdf::Font;

use crate::types::FontFamily;

/// Nombre PDF de la fuente estándar.
pub fn base14_name(family: FontFamily, bold: bool, italic: bool) -> &'static str {
    match (family, bold, italic) {
        (FontFamily::Sans, false, false) => "Helvetica",
        (FontFamily::Sans, true, false) => "Helvetica-Bold",
        (FontFamily::Sans, false, true) => "Helvetica-Oblique",
        (FontFamily::Sans, true, true) => "Helvetica-BoldOblique",
        (FontFamily::Serif, false, false) => "Times-Roman",
        (FontFamily::Serif, true, false) => "Times-Bold",
        (FontFamily::Serif, false, true) => "Times-Italic",
        (FontFamily::Serif, true, true) => "Times-BoldItalic",
        (FontFamily::Mono, false, false) => "Courier",
        (FontFamily::Mono, true, false) => "Courier-Bold",
        (FontFamily::Mono, false, true) => "Courier-Oblique",
        (FontFamily::Mono, true, true) => "Courier-BoldOblique",
    }
}

/// Familia aproximada a partir del nombre de la fuente original.
pub fn family_from_name(name: &str, serif: bool, mono: bool) -> FontFamily {
    let n = name.to_ascii_lowercase();
    if mono || n.contains("courier") || n.contains("mono") || n.contains("consolas") {
        FontFamily::Mono
    } else if serif || n.contains("times") || n.contains("serif") && !n.contains("sans") || n.contains("roman") {
        FontFamily::Serif
    } else {
        FontFamily::Sans
    }
}

/// Byte WinAnsi de un carácter, si existe.
pub fn winansi(c: char) -> Option<u8> {
    let cp = c as u32;
    match cp {
        0x20..=0x7e | 0xa0..=0xff => Some(cp as u8),
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

/// Codifica en WinAnsi; falla con un mensaje claro si hay caracteres que la fuente estándar no tiene.
pub fn encode(text: &str) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len());
    let mut missing: Vec<char> = Vec::new();
    for c in text.chars() {
        match winansi(c) {
            Some(b) => out.push(b),
            None => {
                if !missing.contains(&c) {
                    missing.push(c);
                }
            }
        }
    }
    if missing.is_empty() {
        return Ok(out);
    }
    let list: String = missing.iter().take(5).collect();
    Err(Error::Unsupported(format!(
        "las fuentes estándar no tienen estos caracteres: «{list}». \
         Escribe solo caracteres latinos o espera a la edición con fuentes incrustadas"
    )))
}

/// Medidor de anchos con la fuente estándar real del motor.
pub struct Metrics {
    font: Font,
}

impl Metrics {
    pub fn new(name: &str) -> Result<Self> {
        Ok(Self { font: Font::new(name)? })
    }

    /// Ancho en puntos de `text` (WinAnsi ya validado) al tamaño `size`.
    pub fn width(&self, text: &str, size: f32) -> f32 {
        text.chars()
            .map(|c| {
                self.font
                    .encode_character(c as i32)
                    .and_then(|g| self.font.advance_glyph(g))
                    .unwrap_or(0.5)
            })
            .sum::<f32>()
            * size
    }
}
