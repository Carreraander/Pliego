//! Utilidades para construir y recorrer objetos PDF.

use mupdf::pdf::{PdfDocument, PdfObject};
use pliego_core::{Error, Result};

use crate::pe;

pub const F_READ_ONLY: u32 = 1;
pub const F_REQUIRED: u32 = 1 << 1;
pub const F_MULTILINE: u32 = 1 << 12;
pub const F_PASSWORD: u32 = 1 << 13;
pub const F_NO_TOGGLE_OFF: u32 = 1 << 14;
pub const F_RADIO: u32 = 1 << 15;
pub const F_PUSHBUTTON: u32 = 1 << 16;
pub const F_COMBO: u32 = 1 << 17;

/// Cadena PDF literal: ASCII con escapes, o UTF-16BE con BOM en hexadecimal.
pub fn pdf_str(s: &str) -> String {
    if s.is_ascii() {
        let mut out = String::from("(");
        for c in s.chars() {
            match c {
                '(' | ')' | '\\' => {
                    out.push('\\');
                    out.push(c);
                }
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                c if c.is_ascii_control() => out.push_str(&format!("\\{:03o}", c as u8)),
                c => out.push(c),
            }
        }
        out.push(')');
        out
    } else {
        let mut out = String::from("<FEFF");
        for u in s.encode_utf16() {
            out.push_str(&format!("{u:04X}"));
        }
        out.push('>');
        out
    }
}

/// Nombre PDF (`/Nombre`) con `#XX` para lo que no sea un carácter regular.
pub fn pdf_name(s: &str) -> String {
    let mut out = String::from("/");
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b'+') {
            out.push(b as char);
        } else {
            out.push_str(&format!("#{b:02X}"));
        }
    }
    out
}

pub fn rect_src(r: [f32; 4]) -> String {
    format!("[{} {} {} {}]", r[0], r[1], r[2], r[3])
}

/// Normaliza un rectángulo y comprueba que sea utilizable.
pub fn check_rect(r: [f32; 4]) -> Result<[f32; 4]> {
    if r.iter().any(|v| !v.is_finite()) {
        return Err(Error::InvalidInput("el rectángulo del campo no es válido".into()));
    }
    let n = [r[0].min(r[2]), r[1].min(r[3]), r[0].max(r[2]), r[1].max(r[3])];
    if n[2] - n[0] < 1.0 || n[3] - n[1] < 1.0 {
        return Err(Error::InvalidInput(
            "el campo es demasiado pequeño (mínimo 1×1 puntos)".into(),
        ));
    }
    Ok(n)
}

pub fn check_name(name: &str) -> Result<()> {
    if name.trim().is_empty() {
        return Err(Error::InvalidInput("el campo necesita un nombre".into()));
    }
    if name.contains('.') {
        return Err(Error::InvalidInput(format!(
            "el nombre «{name}» no puede contener puntos"
        )));
    }
    Ok(())
}

pub fn obj(doc: &PdfDocument, src: &str) -> Result<PdfObject> {
    doc.new_object_from_str(src).map_err(pe)
}

pub fn put(o: &mut PdfObject, key: &str, v: PdfObject) -> Result<()> {
    o.dict_put(key, v).map_err(pe)
}

pub fn flags_of(o: &PdfObject) -> u32 {
    o.get_dict_inheritable("Ff")
        .ok()
        .flatten()
        .and_then(|f| f.as_int().ok())
        .unwrap_or(0) as u32
}

pub fn set_flag(field: &mut PdfObject, doc: &PdfDocument, bit: u32, on: bool) -> Result<()> {
    let cur = flags_of(field);
    let new = if on { cur | bit } else { cur & !bit };
    put(field, "Ff", doc.new_int(new as i32).map_err(pe)?)
}

/// Objeto de campo de un widget: el primero, subiendo por `/Parent`, que tenga `/T`.
pub fn field_of(widget: &PdfObject) -> Result<PdfObject> {
    let mut o = widget.try_clone().map_err(pe)?;
    loop {
        if o.get_dict("T").map_err(pe)?.is_some() {
            return Ok(o);
        }
        match o.get_dict("Parent").map_err(pe)? {
            Some(p) => o = p,
            None => return Ok(o),
        }
    }
}

/// `/Rect` crudo de un widget.
pub fn raw_rect(o: &PdfObject) -> [f32; 4] {
    let mut r = [0.0; 4];
    if let Ok(Some(a)) = o.get_dict("Rect") {
        for (i, v) in r.iter_mut().enumerate() {
            if let Ok(Some(n)) = a.get_array(i as i32) {
                *v = n.as_float().unwrap_or(0.0);
            }
        }
    }
    r
}

pub fn xref_of(o: &PdfObject) -> i32 {
    if o.is_indirect().unwrap_or(false) {
        o.as_indirect().unwrap_or(-1)
    } else {
        -1
    }
}

/// `/DA` con la fuente `Helv` (definida en el `/DR` del formulario).
pub fn da(font_size: f32) -> String {
    format!("/Helv {} Tf 0 g", if font_size > 0.0 { font_size } else { 0.0 })
}
