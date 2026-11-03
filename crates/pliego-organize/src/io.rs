use std::path::Path;

use mupdf::Document;
use mupdf::pdf::{PdfDocument, PdfWriteOptions};
use pliego_core::{Error, Result};

use crate::pdf_err;

/// Abre un PDF para modificarlo.
pub fn open(path: &Path, password: Option<&str>) -> Result<PdfDocument> {
    if !path.exists() {
        return Err(Error::io_at(path, std::io::Error::from(std::io::ErrorKind::NotFound)));
    }
    // Se lee a memoria y no se mantiene el archivo abierto: en Windows no se puede sustituir un archivo
    // abierto, y con --in-place el destino es el propio original.
    let bytes = std::fs::read(path).map_err(|e| Error::io_at(path, e))?;
    let mut doc = Document::from_bytes(&bytes, "application/pdf").map_err(pdf_err)?;
    if doc.needs_password().map_err(pdf_err)? && !password.is_some_and(|p| doc.authenticate(p).unwrap_or(false)) {
        return Err(Error::Password);
    }
    PdfDocument::try_from(doc).map_err(|_| Error::InvalidInput(format!("{} no es un PDF", path.display())))
}

/// Guarda a un temporal junto al destino y lo renombra, para no dejar archivos a medias.
/// Elimina objetos sin uso y fusiona los duplicados (fuentes e imágenes idénticas).
pub fn save(doc: &PdfDocument, out: &Path) -> Result<()> {
    let dir = out
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let tmp = tempfile::Builder::new()
        .prefix(".pliego-")
        .suffix(".tmp")
        .tempfile_in(dir)
        .map_err(|e| Error::io_at(dir, e))?;
    let mut opts = PdfWriteOptions::default();
    opts.set_garbage_level(3).set_compress(true);
    let path = tmp
        .path()
        .to_str()
        .ok_or_else(|| Error::InvalidInput("ruta no válida".into()))?;
    doc.save_with_options(path, opts).map_err(pdf_err)?;
    tmp.persist(out).map_err(|e| Error::io_at(out, e.error))?;
    Ok(())
}
