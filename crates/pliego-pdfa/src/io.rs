use std::path::Path;

use pliego_core::{Error, Result};
use pliego_pdf::{Object, PdfDocument, WriteOptions};

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
pub(crate) fn save_atomic(doc: &PdfDocument, input: &Path, output: &Path, options: WriteOptions) -> Result<()> {
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
    tmp.persist(output).map_err(|e| Error::io_at(output, e.error))?;
    Ok(())
}

pub(crate) fn name_of(obj: &Object, key: &str) -> Option<String> {
    let v = obj.get_dict(key).ok()??;
    String::from_utf8(v.as_name().ok()?).ok()
}

/// Número de objeto si la referencia es indirecta; 0 si no.
pub(crate) fn xref_of(obj: &Object) -> i32 {
    obj.as_indirect().unwrap_or(0)
}
