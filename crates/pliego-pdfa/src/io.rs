use std::path::Path;

use mupdf::pdf::{PdfDocument, PdfObject};
use pliego_core::{Error, Result};

pub(crate) fn pdf_err(e: mupdf::Error) -> Error {
    Error::Pdf(e.to_string())
}

/// Abre un PDF, autenticando con `password` si lo necesita.
pub(crate) fn open_pdf(path: &Path, password: Option<&str>) -> Result<PdfDocument> {
    if !path.exists() {
        return Err(Error::io_at(path, std::io::Error::from(std::io::ErrorKind::NotFound)));
    }
    let path_str = path
        .to_str()
        .ok_or_else(|| Error::InvalidInput("la ruta del archivo debe ser UTF-8".into()))?;
    let mut doc = PdfDocument::open(path_str).map_err(pdf_err)?;
    if doc.needs_password().map_err(pdf_err)? {
        match password {
            Some(p) if doc.authenticate(p).map_err(pdf_err)? => {}
            _ => return Err(Error::Password),
        }
    }
    Ok(doc)
}

pub(crate) fn name_of(obj: &PdfObject, key: &str) -> Option<String> {
    let v = obj.get_dict(key).ok()??;
    String::from_utf8(v.as_name().ok()?).ok()
}

/// Número de objeto si la referencia es indirecta; 0 si no.
pub(crate) fn xref_of(obj: &PdfObject) -> i32 {
    obj.as_indirect().unwrap_or(0)
}
