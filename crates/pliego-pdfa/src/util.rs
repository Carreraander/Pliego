//! Utilidades sobre objetos PDF compartidas por la validación y la conversión.

use std::fmt::Write as _;

use pliego_core::Result;
use pliego_pdf::{Object, PdfDocument};

/// Pares (clave, valor) de un diccionario.
pub(crate) fn entries(obj: &Object) -> Vec<(String, Object)> {
    obj.dict_iter().unwrap_or_default()
}

/// Elementos de una matriz (vacío si no lo es).
pub(crate) fn array_items(obj: &Object) -> Vec<Object> {
    let mut v = Vec::new();
    if obj.is_array().unwrap_or(false) {
        for i in 0..obj.len().unwrap_or(0).min(100_000) {
            if let Ok(Some(x)) = obj.get_array(i as i32) {
                v.push(x);
            }
        }
    }
    v
}

pub(crate) fn get(obj: &Object, key: &str) -> Option<Object> {
    obj.get_dict(key).ok().flatten()
}

pub(crate) fn text_of(obj: &Object, key: &str) -> Option<String> {
    get(obj, key)?.as_string().ok().filter(|s| !s.is_empty())
}

/// Cadena de texto PDF: ASCII tal cual, el resto en UTF-16BE con BOM.
pub(crate) fn text_string(doc: &PdfDocument, s: &str) -> Result<Object> {
    let mut hex = String::from("<");
    if s.is_ascii() && !s.contains('\0') {
        for b in s.bytes() {
            let _ = write!(hex, "{b:02X}");
        }
    } else {
        hex.push_str("FEFF");
        for u in s.encode_utf16() {
            let _ = write!(hex, "{u:04X}");
        }
    }
    hex.push('>');
    doc.new_object_from_str(&hex)
}
