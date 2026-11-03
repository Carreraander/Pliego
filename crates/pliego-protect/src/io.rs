use std::path::Path;

use mupdf::pdf::{PdfDocument, PdfWriteOptions};
use pliego_core::{Error, Result};

pub(crate) fn pdf_err(e: mupdf::Error) -> Error {
    Error::Pdf(e.to_string())
}

/// Abre un PDF, autenticando con `password` si lo necesita.
pub(crate) fn open_pdf(path: &Path, password: Option<&str>) -> Result<PdfDocument> {
    if !path.exists() {
        return Err(Error::io_at(path, std::io::Error::from(std::io::ErrorKind::NotFound)));
    }
    // En Windows `mupdf` solo acepta rutas UTF-8 (`&str`).
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

/// Guarda en un temporal junto al destino y lo renombra. Nunca pisa la entrada.
pub(crate) fn save_atomic(doc: &PdfDocument, input: &Path, output: &Path, options: PdfWriteOptions) -> Result<()> {
    let same = match (std::fs::canonicalize(input), std::fs::canonicalize(output)) {
        (Ok(a), Ok(b)) => a == b,
        _ => input == output,
    };
    if same {
        return Err(Error::InvalidInput(
            "el archivo de salida es el mismo que el de entrada; elige otro nombre para no perder el original".into(),
        ));
    }
    let dir = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let tmp = tempfile::Builder::new()
        .prefix(".pliego-")
        .suffix(".tmp")
        .tempfile_in(dir)
        .map_err(|e| Error::io_at(dir, e))?;
    let tmp_str = tmp
        .path()
        .to_str()
        .ok_or_else(|| Error::InvalidInput("la ruta de salida debe ser UTF-8".into()))?;
    doc.save_with_options(tmp_str, options).map_err(pdf_err)?;
    tmp.persist(output).map_err(|e| Error::io_at(output, e.error))?;
    Ok(())
}
