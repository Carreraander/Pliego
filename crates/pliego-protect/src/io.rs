use std::path::Path;

use pliego_pdf::{PdfDocument, WriteOptions};
use pliego_core::{Error, Result};

/// Abre un PDF, autenticando con `password` si lo necesita.
pub(crate) fn open_pdf(path: &Path, password: Option<&str>) -> Result<PdfDocument> {
    if !path.exists() {
        return Err(Error::io_at(path, std::io::Error::from(std::io::ErrorKind::NotFound)));
    }
    let mut doc = PdfDocument::open(path)?;
    if doc.needs_password()? {
        match password {
            Some(p) if doc.authenticate(p)? => {}
            _ => return Err(Error::Password),
        }
    }
    Ok(doc)
}

/// Guarda en un temporal junto al destino y lo renombra. Nunca pisa la entrada.
pub(crate) fn save_atomic(doc: PdfDocument, input: &Path, output: &Path, options: WriteOptions) -> Result<()> {
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
    doc.save_with_options(tmp.path(), &options)?;
    // Se suelta el documento antes de renombrar: en Windows el motor puede seguir teniendo abierto el temporal.
    drop(doc);
    // Windows: un antivirus o el indexador pueden retener el archivo recién escrito unos instantes (error 32).
    let mut tmp = tmp;
    let mut attempt = 0;
    loop {
        match tmp.persist(output) {
            Ok(_) => return Ok(()),
            Err(e) if attempt < 10 && e.error.raw_os_error() == Some(32) => {
                attempt += 1;
                tmp = e.file;
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Err(e) => return Err(Error::io_at(output, e.error)),
        }
    }
}
