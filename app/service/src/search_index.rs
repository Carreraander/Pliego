//! Índice de búsqueda de carpetas desde la app (flujo W29, `batch.index`).
//!
//! La lógica vive aquí, sin Tauri, para probarla en Linux; los comandos de `app/src-tauri` solo traducen.
//! El índice se guarda en `<datos de Pliego>/index`.

use std::path::{Path, PathBuf};

use pliego_core::{Error, JobContext, Report, Result};
pub use pliego_index::Hit;
use pliego_index::Index;

/// Carpeta del índice dentro de `data_dir`.
pub fn index_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("index")
}

/// Indexa (o actualiza) todos los PDF de `folder`.
pub fn index_folder(data_dir: &Path, folder: &Path, ctx: &JobContext) -> Result<Report> {
    if !folder.is_dir() {
        return Err(Error::InvalidInput(format!("{} no es una carpeta", folder.display())));
    }
    Index::open(&index_dir(data_dir))?.add_folder(folder, ctx)
}

/// Busca `query` en el índice. Una consulta vacía no devuelve nada.
pub fn search(data_dir: &Path, query: &str, limit: usize) -> Result<Vec<Hit>> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    Index::open(&index_dir(data_dir))?.search(query, limit.clamp(1, 200))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_a_file_as_folder() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.pdf");
        std::fs::write(&file, b"x").unwrap();
        let err = index_folder(dir.path(), &file, &JobContext::silent()).unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)));
    }

    #[test]
    fn empty_folder_indexes_nothing_and_search_finds_nothing() {
        let data = tempfile::tempdir().unwrap();
        let docs = tempfile::tempdir().unwrap();
        index_folder(data.path(), docs.path(), &JobContext::silent()).unwrap();
        assert!(search(data.path(), "hola", 10).unwrap().is_empty());
        assert!(search(data.path(), "  ", 10).unwrap().is_empty());
    }
}
