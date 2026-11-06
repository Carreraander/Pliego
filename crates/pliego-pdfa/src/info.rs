//! Propiedades del documento (título, autor, asunto y palabras clave): se escriben en `/Info` y, si el documento
//! declara PDF/A, también en su XMP, para que siga siendo válido (PDF/A exige que coincidan).

use pliego_core::Result;
use pliego_pdf::{Object, PdfDocument};
use serde::{Deserialize, Serialize};

use crate::Level;
use crate::convert::{read_info, write_info};
use crate::util::get;
use crate::validate::xmp_value;
use crate::xmp::{build_xmp, now_pdf_date};

/// Lo que se quiere cambiar. `None` deja el valor como está; un texto vacío lo quita.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InfoEdit {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub subject: Option<String>,
    #[serde(default)]
    pub keywords: Option<String>,
}

/// Qué pasó con los metadatos XMP.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum XmpSync {
    /// No tenía XMP.
    None,
    /// Era PDF/A: se rehízo el XMP con los valores nuevos.
    Rebuilt,
    /// Tenía XMP que no es de un nivel que Pliego sepa escribir: se deja (los lectores usan `/Info`, que es más nuevo).
    Kept,
}

/// Nivel PDF/A que declara un XMP, si Pliego sabe escribirlo.
fn declared_level(xmp: &str) -> Option<Level> {
    let part = xmp_value(xmp, "pdfaid:part")?;
    let conf = xmp_value(xmp, "pdfaid:conformance")?.to_ascii_uppercase();
    match (part.as_str(), conf.as_str()) {
        ("1", "B") => Some(Level::A1b),
        ("2", "B") => Some(Level::A2b),
        ("2", "U") => Some(Level::A2u),
        ("3", "B") => Some(Level::A3b),
        _ => None,
    }
}

/// Cambia las propiedades del documento abierto y pone la fecha de modificación.
pub fn update_info(doc: &mut PdfDocument, edit: &InfoEdit) -> Result<XmpSync> {
    let mut info = read_info(doc)?;
    let mut removed = Vec::new();
    for (key, slot, new) in [
        ("Title", &mut info.title, &edit.title),
        ("Author", &mut info.author, &edit.author),
        ("Subject", &mut info.subject, &edit.subject),
        ("Keywords", &mut info.keywords, &edit.keywords),
    ] {
        match new.as_deref().map(str::trim) {
            None => {}
            Some("") => {
                *slot = None;
                removed.push(key);
            }
            Some(v) => *slot = Some(v.to_string()),
        }
    }
    info.modified = Some(now_pdf_date());
    write_info(doc, &info)?;
    if !removed.is_empty() {
        let trailer = doc.trailer()?;
        if let Some(mut d) = get(&trailer, "Info").filter(|i| i.is_dict().unwrap_or(false)) {
            for k in removed {
                if d.get_dict(k)?.is_some() {
                    d.dict_delete(k)?;
                }
            }
        }
    }

    let mut catalog = doc.catalog()?;
    let Some(bytes) = get(&catalog, "Metadata").and_then(|m| m.read_stream().ok()) else {
        return Ok(XmpSync::None);
    };
    let Some(level) = declared_level(&String::from_utf8_lossy(&bytes)) else {
        return Ok(XmpSync::Kept);
    };
    let xmp = build_xmp(level, &info);
    let mut dict = doc.new_dict()?;
    dict.dict_put("Type", Object::new_name("Metadata")?)?;
    dict.dict_put("Subtype", Object::new_name("XML")?)?;
    let meta = doc.add_stream(xmp.as_bytes(), Some(&dict), false)?;
    catalog.dict_put("Metadata", meta)?;
    Ok(XmpSync::Rebuilt)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_pliego_can_write() {
        let x =
            |p: &str, c: &str| format!("<pdfaid:part>{p}</pdfaid:part><pdfaid:conformance>{c}</pdfaid:conformance>");
        assert_eq!(declared_level(&x("2", "u")), Some(Level::A2u));
        assert_eq!(declared_level(&x("1", "B")), Some(Level::A1b));
        assert_eq!(declared_level(&x("2", "A")), None);
        assert_eq!(declared_level("<x/>"), None);
    }
}
