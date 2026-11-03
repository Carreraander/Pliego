//! Producción de impresión: tintas, vista previa de separaciones y preflight.
//!
//! - [`inks`] lista las tintas (de proceso y planchas de color directo) que usa un documento.
//! - [`separations`] genera las planchas CMYK de una página para la vista previa.
//! - [`preflight`] comprueba el documento contra un [`Profile`] y devuelve un [`PreflightReport`].
//!
//! Todo se basa en recorrer los flujos de contenido de cada página (`content` y `scan`), sin estado global.

mod content;
mod inks;
mod preflight;
mod scan;
mod separations;

pub use inks::{Ink, InkKind, inks};
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
