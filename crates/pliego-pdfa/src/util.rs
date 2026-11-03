//! Utilidades sobre objetos PDF compartidas por la validación y la conversión.

use std::fmt::Write as _;

use mupdf::pdf::{PdfDocument, PdfObject};
use pliego_core::Result;

use crate::io::pdf_err;

/// Pares (clave, valor) de un diccionario.
pub(crate) fn entries(obj: &PdfObject) -> Vec<(String, PdfObject)> {
    let mut v = Vec::new();
    if let Ok(it) = obj.dict_iter() {
        for (k, val) in it.flatten() {
            if let Ok(k) = k.as_name() {
                v.push((String::from_utf8_lossy(&k).into_owned(), val));
            }
        }
    }
    v
}

/// Elementos de una matriz (vacío si no lo es).
pub(crate) fn array_items(obj: &PdfObject) -> Vec<PdfObject> {
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

pub(crate) fn get(obj: &PdfObject, key: &str) -> Option<PdfObject> {
    obj.get_dict(key).ok().flatten()
}

pub(crate) fn text_of(obj: &PdfObject, key: &str) -> Option<String> {
    get(obj, key)?.as_string().ok().filter(|s| !s.is_empty())
}

/// Cadena de texto PDF: ASCII tal cual, el resto en UTF-16BE con BOM.
pub(crate) fn text_string(doc: &PdfDocument, s: &str) -> Result<PdfObject> {
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
    doc.new_object_from_str(&hex).map_err(pdf_err)
}
