//! Datos del archivo de un documento para «Propiedades del documento» (flujo W29, `viewer.properties`).

use std::path::Path;
use std::time::UNIX_EPOCH;

use pliego_core::{Error, Result};
use serde::Serialize;

/// Tamaño y fecha de modificación del archivo en disco.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileFacts {
    pub size: u64,
    /// Segundos desde 1970 (UTC); `None` si el sistema de archivos no la da.
    pub modified_secs: Option<u64>,
}

pub fn file_facts(path: &Path) -> Result<FileFacts> {
    let meta = std::fs::metadata(path).map_err(|e| Error::io_at(path, e))?;
    if !meta.is_file() {
        return Err(Error::InvalidInput(format!("{} no es un archivo", path.display())));
    }
    let modified_secs = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs());
    Ok(FileFacts {
        size: meta.len(),
        modified_secs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_size_and_date() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("a.pdf");
        std::fs::write(&f, b"12345").unwrap();
        let facts = file_facts(&f).unwrap();
        assert_eq!(facts.size, 5);
        assert!(facts.modified_secs.is_some_and(|s| s > 1_600_000_000));
    }

    #[test]
    fn missing_file_and_folder_are_errors() {
        let dir = tempfile::tempdir().unwrap();
        assert!(file_facts(&dir.path().join("no.pdf")).is_err());
        assert!(matches!(file_facts(dir.path()), Err(Error::InvalidInput(_))));
    }
}
