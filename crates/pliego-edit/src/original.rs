//! Reutilización de la fuente original de un bloque.
//!
//! Si la fuente que ya usa el bloque (sencilla de un byte o Type0 `Identity-H`) tiene todos los caracteres
//! del texto nuevo, se compone con ella: el aspecto no cambia y el archivo no crece. Los glifos disponibles se
//! deducen del `ToUnicode` (que en un subconjunto lista solo los glifos incrustados) o, si la fuente no va
//! incrustada, de su codificación `WinAnsiEncoding`.

use std::collections::HashMap;

use pliego_core::Result;
use pliego_pdf::{Object, PdfPage};

use crate::fonts::winansi;

/// Fuente original lista para componer con ella.
pub(crate) struct Original {
    /// Nombre del recurso con barra, p. ej. `/F1`.
    pub resource: String,
    /// Dos bytes por código (Type0 `Identity-H`) o uno.
    pub two_byte: bool,
    codes: HashMap<char, u16>,
    /// Anchos por código en milésimas de em.
    widths: HashMap<u16, f32>,
    default_width: Option<f32>,
}

impl Original {
    /// Anchura de `text` (en puntos) al tamaño `size`, o `None` si falta algún carácter.
    pub fn width(&self, text: &str, size: f32) -> Option<f32> {
        let mut w = 0.0;
        for c in text.chars() {
            let code = self.codes.get(&c)?;
            w += self.widths.get(code).copied().or(self.default_width)?;
        }
        Some(w * size / 1000.0)
    }

    /// ¿Se pueden componer todos los caracteres (salvo saltos de línea) con esta fuente?
    pub fn covers(&self, text: &str) -> bool {
        text.chars()
            .filter(|c| *c != '\n')
            .all(|c| self.width(&c.to_string(), 1.0).is_some())
    }

    /// Cadena hexadecimal de `text` con los códigos de la fuente.
    pub fn hex(&self, text: &str) -> String {
        text.chars()
            .filter_map(|c| self.codes.get(&c))
            .map(|code| {
                if self.two_byte {
                    format!("{code:04X}")
                } else {
                    format!("{code:02X}")
                }
            })
            .collect()
    }
}

fn base_name(obj: &Object) -> Option<String> {
    let n = String::from_utf8(obj.get_dict("BaseFont").ok()??.as_name().ok()?).ok()?;
    Some(strip_subset(&n).to_owned())
}

/// Quita el prefijo `ABCDEF+` de las fuentes en subconjunto.
fn strip_subset(name: &str) -> &str {
    match name.split_once('+') {
        Some((p, rest)) if p.len() == 6 && p.chars().all(|c| c.is_ascii_uppercase()) => rest,
        _ => name,
    }
}

fn num(o: &Object) -> Option<f32> {
    if o.is_number().ok()? { o.as_float().ok() } else { None }
}

/// Busca entre las fuentes de la página la que se llama `font_name` y la prepara.
pub(crate) fn find(page: &PdfPage, font_name: &str) -> Result<Option<Original>> {
    let wanted = strip_subset(font_name);
    let res = page.resources()?;
    let Some(fonts) = res.get_dict("Font")? else {
        return Ok(None);
    };
    if !fonts.is_dict()? {
        return Ok(None);
    }
    for i in 0..fonts.dict_len()? as i32 {
        let (Some(k), Some(v)) = (fonts.get_dict_key(i)?, fonts.get_dict_val(i)?) else {
            continue;
        };
        if base_name(&v).as_deref() != Some(wanted) {
            continue;
        }
        let key = k;
        if !key.chars().all(|c| c.is_ascii_alphanumeric() || "_.-".contains(c)) {
            continue;
        }
        if let Some(o) = build(&v, format!("/{key}"))? {
            return Ok(Some(o));
        }
    }
    Ok(None)
}

fn build(font: &Object, resource: String) -> Result<Option<Original>> {
    let subtype = font
        .get_dict("Subtype")?
        .and_then(|s| s.as_name().ok())
        .unwrap_or_default();
    let to_unicode = match font.get_dict("ToUnicode")? {
        Some(t) if t.is_stream()? => Some(parse_to_unicode(&t.read_stream()?)),
        _ => None,
    };
    match subtype.as_slice() {
        b"Type0" => {
            let enc = font.get_dict("Encoding")?.and_then(|e| e.as_name().ok());
            if enc.as_deref() != Some(b"Identity-H") {
                return Ok(None);
            }
            let (Some(codes), Some(desc)) = (
                to_unicode,
                font.get_dict("DescendantFonts")?
                    .and_then(|d| d.get_array(0).ok().flatten()),
            ) else {
                return Ok(None);
            };
            let default_width = desc.get_dict("DW")?.and_then(|d| num(&d)).unwrap_or(1000.0);
            let mut widths = HashMap::new();
            if let Some(w) = desc.get_dict("W")? {
                parse_w(&w, &mut widths)?;
            }
            Ok(Some(Original {
                resource,
                two_byte: true,
                codes,
                widths,
                default_width: Some(default_width),
            }))
        }
        b"Type1" | b"TrueType" => {
            let (Some(first), Some(w)) = (
                font.get_dict("FirstChar")?.and_then(|f| num(&f)),
                font.get_dict("Widths")?,
            ) else {
                return Ok(None);
            };
            let mut widths = HashMap::new();
            for i in 0..w.len()? as i32 {
                if let Some(v) = w.get_array(i)?.and_then(|v| num(&v)) {
                    let code = first as i32 + i;
                    if (0..256).contains(&code) {
                        widths.insert(code as u16, v);
                    }
                }
            }
            let embedded = font.get_dict("FontDescriptor")?.is_some_and(|d| {
                ["FontFile", "FontFile2", "FontFile3"]
                    .iter()
                    .any(|k| d.get_dict(*k).ok().flatten().is_some())
            });
            let codes = match to_unicode {
                Some(c) => c,
                None if !embedded => {
                    // Sin incrustar: WinAnsi estándar, siempre que no haya `Differences`.
                    let ok = match font.get_dict("Encoding")? {
                        Some(e) if e.is_name()? => e.as_name().ok().as_deref() == Some(b"WinAnsiEncoding"),
                        _ => false,
                    };
                    if !ok {
                        return Ok(None);
                    }
                    (32u32..=0x20AC)
                        .filter_map(char::from_u32)
                        .filter_map(|ch| winansi(ch).filter(|b| *b >= 32).map(|b| (ch, b as u16)))
                        .collect()
                }
                None => return Ok(None),
            };
            let codes = codes.into_iter().filter(|(_, c)| *c < 256).collect();
            Ok(Some(Original {
                resource,
                two_byte: false,
                codes,
                widths,
                default_width: None,
            }))
        }
        _ => Ok(None),
    }
}

fn parse_w(w: &Object, out: &mut HashMap<u16, f32>) -> Result<()> {
    let n = w.len()? as i32;
    let mut i = 0;
    while i < n {
        let Some(c) = w.get_array(i)?.and_then(|v| num(&v)) else {
            break;
        };
        let Some(next) = w.get_array(i + 1)? else { break };
        if next.is_array()? {
            for j in 0..next.len()? as i32 {
                if let Some(v) = next.get_array(j)?.and_then(|v| num(&v)) {
                    out.insert((c as i32 + j) as u16, v);
                }
            }
            i += 2;
        } else {
            let (Some(hi), Some(v)) = (num(&next), w.get_array(i + 2)?.and_then(|v| num(&v))) else {
                break;
            };
            for code in (c as i32)..=(hi as i32).min(c as i32 + 70_000) {
                out.insert(code as u16, v);
            }
            i += 3;
        }
    }
    Ok(())
}

/// Lee `bfchar`/`bfrange` y devuelve carácter → código (solo asignaciones a un único carácter).
fn parse_to_unicode(data: &[u8]) -> HashMap<char, u16> {
    let text = String::from_utf8_lossy(data);
    let mut map = HashMap::new();
    let hexes = |line: &str| -> Vec<String> {
        let mut v = Vec::new();
        let mut rest = line;
        while let Some(a) = rest.find('<') {
            let Some(b) = rest[a..].find('>') else { break };
            v.push(rest[a + 1..a + b].replace(char::is_whitespace, ""));
            rest = &rest[a + b + 1..];
        }
        v
    };
    let to_char = |h: &str| -> Option<char> {
        let units: Vec<u16> = (0..h.len())
            .step_by(4)
            .filter_map(|i| u16::from_str_radix(h.get(i..i + 4)?, 16).ok())
            .collect();
        let s = String::from_utf16(&units).ok()?;
        let mut it = s.chars();
        let c = it.next()?;
        it.next().is_none().then_some(c)
    };
    let mut mode = 0;
    for line in text.lines() {
        let l = line.trim();
        if l.ends_with("beginbfchar") {
            mode = 1;
        } else if l.ends_with("beginbfrange") {
            mode = 2;
        } else if l.starts_with("endbf") {
            mode = 0;
        } else if mode == 1 {
            let h = hexes(l);
            if let [src, dst] = h.as_slice()
                && let (Ok(code), Some(c)) = (u16::from_str_radix(src, 16), to_char(dst))
            {
                map.entry(c).or_insert(code);
            }
        } else if mode == 2 {
            let h = hexes(l);
            if h.len() >= 3
                && !l.contains('[')
                && let (Ok(lo), Ok(hi), Some(c0)) = (
                    u16::from_str_radix(&h[0], 16),
                    u16::from_str_radix(&h[1], 16),
                    to_char(&h[2]),
                )
            {
                for (k, code) in (lo..=hi).enumerate() {
                    if let Some(c) = char::from_u32(c0 as u32 + k as u32) {
                        map.entry(c).or_insert(code);
                    }
                }
            }
        }
    }
    map
}
