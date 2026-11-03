//! Producción de impresión: tintas, vista previa de separaciones y preflight.
//!
//! - [`inks`] lista las tintas (de proceso y planchas de color directo) que usa un documento.
//! - [`separations`] genera las planchas CMYK de una página para la vista previa.
//! - [`preflight`] comprueba el documento contra un [`Profile`] y devuelve un [`PreflightReport`].
//! - [`fix_hairlines`] sube al mínimo el grosor de las líneas demasiado finas.
//! - [`add_printer_marks`] añade marcas de corte y registro y amplía las cajas de sangrado.
//! - [`to_pdfx`] convierte a PDF/X con Ghostscript (opcional, herramienta externa).
//!
//! Todo se basa en recorrer los flujos de contenido de cada página (`content` y `scan`), sin estado global.

mod content;
mod hairlines;
mod inks;
mod marks;
mod pdfx;
mod preflight;
mod scan;
mod separations;

pub use hairlines::{HairlineFix, fix_hairlines};
pub use inks::{Ink, InkKind, inks};
pub use marks::{MarkOptions, add_printer_marks};
pub use pdfx::{PdfxLevel, PdfxOptions, find_ghostscript, to_pdfx};
pub use preflight::{Finding, FindingCode, PreflightReport, Profile, Severity, preflight};
pub use separations::{Plate, PlateChannel, Separations, separations};

pub use mupdf::pdf::PdfDocument;

pub(crate) fn pdf_err(e: mupdf::Error) -> pliego_core::Error {
    pliego_core::Error::Pdf(e.to_string())
}

/// Abre un PDF de solo lectura.
pub fn open(path: &std::path::Path, password: Option<&str>) -> pliego_core::Result<PdfDocument> {
    use pliego_core::Error;
    if !path.exists() {
        return Err(Error::io_at(path, std::io::Error::from(std::io::ErrorKind::NotFound)));
    }
    let bytes = std::fs::read(path).map_err(|e| Error::io_at(path, e))?;
    let mut doc = mupdf::Document::from_bytes(&bytes, "application/pdf").map_err(pdf_err)?;
    if doc.needs_password().map_err(pdf_err)? && !password.is_some_and(|p| doc.authenticate(p).unwrap_or(false)) {
        return Err(Error::Password);
    }
    PdfDocument::try_from(doc).map_err(|_| Error::InvalidInput(format!("{} no es un PDF", path.display())))
}

/// Guarda a un temporal junto al destino y lo renombra, para no dejar archivos a medias.
pub fn save(doc: &PdfDocument, out: &std::path::Path) -> pliego_core::Result<()> {
    use pliego_core::Error;
    let dir = out
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(std::path::Path::new("."));
    let tmp = tempfile::Builder::new()
        .prefix(".pliego-")
        .suffix(".tmp")
        .tempfile_in(dir)
        .map_err(|e| Error::io_at(dir, e))?;
    let tmp_str = tmp
        .path()
        .to_str()
        .ok_or_else(|| Error::InvalidInput("la ruta de salida debe ser UTF-8".into()))?;
    let mut opts = mupdf::pdf::PdfWriteOptions::default();
    opts.set_garbage_level(2).set_compress(true);
    doc.save_with_options(tmp_str, opts).map_err(pdf_err)?;
    tmp.persist(out).map_err(|e| Error::io_at(out, e.error))?;
    Ok(())
}
