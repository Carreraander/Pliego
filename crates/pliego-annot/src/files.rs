//! Operaciones sobre archivos para la app: abrir con contraseña, resumen, exportar e importar XFDF.
//!
//! Son las funciones que llaman los comandos de la app (que no tienen pruebas propias): reciben rutas y nunca
//! tocan el original; las que escriben un PDF lo hacen a una ruta distinta, de forma atómica.

use std::path::Path;

use mupdf::Document;
use mupdf::pdf::{PdfDocument, PdfWriteOptions};
use pliego_core::{Error, Result};

use crate::{CommentSummary, export_xfdf, import_xfdf, summary};

fn pe(e: mupdf::Error) -> Error {
    Error::Pdf(e.to_string())
}

/// Abre un PDF para anotarlo. Se lee a memoria para no dejar el archivo abierto (en Windows impediría sustituirlo).
pub fn open(path: &Path, password: Option<&str>) -> Result<PdfDocument> {
    let bytes = std::fs::read(path).map_err(|e| Error::io_at(path, e))?;
    let mut doc = Document::from_bytes(&bytes, "application/pdf").map_err(pe)?;
    if doc.needs_password().map_err(pe)? && !password.is_some_and(|p| doc.authenticate(p).unwrap_or(false)) {
        return Err(Error::Password);
    }
    PdfDocument::try_from(doc).map_err(|_| Error::InvalidInput(format!("{} no es un PDF", path.display())))
}

/// Guarda a un temporal junto al destino y lo renombra.
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
    let path = tmp
        .path()
        .to_str()
        .ok_or_else(|| Error::InvalidInput("ruta no válida".into()))?;
    let mut opts = PdfWriteOptions::default();
    opts.set_garbage_level(3).set_compress(true);
    doc.save_with_options(path, opts).map_err(pe)?;
    tmp.persist(out).map_err(|e| Error::io_at(out, e.error))?;
    Ok(())
}

/// Resumen de los comentarios de `src`.
pub fn summary_of(src: &Path, password: Option<&str>) -> Result<CommentSummary> {
    summary(&mut open(src, password)?)
}

/// Escribe en `out` los comentarios de `src` en XFDF. Devuelve cuántos exportó el documento (según su resumen).
pub fn export_xfdf_file(src: &Path, password: Option<&str>, out: &Path) -> Result<usize> {
    let mut doc = open(src, password)?;
    let total = summary(&mut doc)?.total;
    let xml = export_xfdf(&mut doc)?;
    std::fs::write(out, xml).map_err(|e| Error::io_at(out, e))?;
    Ok(total)
}

/// Importa el XFDF `xfdf` en una copia de `src` guardada en `out`. Devuelve cuántos comentarios añadió.
pub fn import_xfdf_file(src: &Path, password: Option<&str>, xfdf: &Path, out: &Path) -> Result<usize> {
    let text = std::fs::read_to_string(xfdf).map_err(|e| Error::io_at(xfdf, e))?;
    let mut doc = open(src, password)?;
    let added = import_xfdf(&mut doc, &text)?;
    save(&doc, out)?;
    Ok(added)
}
