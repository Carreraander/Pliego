//! Utilidades internas: apertura de PDF y escritura atómica.

use std::io::Write;
use std::path::{Path, PathBuf};

use mupdf::Document;
use pliego_core::{Error, Result};

pub(crate) fn pdf_err(e: mupdf::Error) -> Error {
    Error::Pdf(e.to_string())
}

/// Abre un PDF (o cualquier formato que MuPDF reconozca) y lo desbloquea si hace falta.
pub(crate) fn open_document(path: &Path, password: Option<&str>) -> Result<Document> {
    if !path.exists() {
        return Err(Error::io_at(path, std::io::Error::from(std::io::ErrorKind::NotFound)));
    }
    let mut doc = Document::open(path).map_err(pdf_err)?;
    if doc.needs_password().map_err(pdf_err)? {
        let ok = match password {
            Some(p) => doc.authenticate(p).map_err(pdf_err)?,
            None => false,
        };
        if !ok {
            return Err(Error::Password);
        }
    }
    Ok(doc)
}

/// Escribe `bytes` en `dest` sin dejar archivos a medias: temporal en la misma carpeta y renombrado.
pub(crate) fn write_atomic(dest: &Path, bytes: &[u8]) -> Result<()> {
    let dir = match dest.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    };
    std::fs::create_dir_all(&dir).map_err(|e| Error::io_at(&dir, e))?;
    let mut tmp = tempfile::NamedTempFile::new_in(&dir).map_err(|e| Error::io_at(&dir, e))?;
    tmp.write_all(bytes).map_err(|e| Error::io_at(dest, e))?;
    tmp.persist(dest).map_err(|e| Error::io_at(dest, e.error))?;
    Ok(())
}

/// Evita sobrescribir el archivo de entrada con la salida.
pub(crate) fn ensure_not_same(input: &Path, output: &Path) -> Result<()> {
    if let (Ok(a), Ok(b)) = (input.canonicalize(), output.canonicalize())
        && a == b
    {
        return Err(Error::InvalidInput(
            "el archivo de salida no puede ser el mismo que el de entrada".into(),
        ));
    }
    Ok(())
}

pub(crate) fn file_stem(path: &Path, fallback: &str) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(fallback)
        .to_string()
}
