//! Carteras PDF desde la app (flujo W29, `batch.portfolio`): crear una cartera, listar y extraer adjuntos.
//!
//! La lógica vive aquí, sin Tauri, para probarla en Linux; `app/src-tauri/src/commands/portfolio.rs` solo traduce.
//! Crear y extraer **no tocan los originales**: la cartera se guarda como `<primero> (cartera).pdf` y los adjuntos van a
//! la carpeta `<nombre> (adjuntos)`, ambos junto al primer archivo y con nombre libre (`pliego_appdata::copyjob`).

use std::path::{Path, PathBuf};

use pliego_appdata::copyjob::{unique_in, unique_sibling};
use pliego_core::{Error, JobContext, Report, Result};
pub use pliego_portfolio::{Entry, Listing};

/// Crea `<primer archivo> (<suffix>).pdf` con todos los `files` incrustados, en ese orden.
pub fn create_portfolio(files: &[PathBuf], suffix: &str, ctx: &JobContext) -> Result<Report> {
    let first = files
        .first()
        .ok_or_else(|| Error::InvalidInput("indica al menos un archivo para la cartera".into()))?;
    let out = unique_sibling(first, suffix, "pdf");
    pliego_portfolio::create(files, &out, ctx)
}

/// Archivos incrustados de `path` y si es una cartera.
pub fn list(path: &Path, password: Option<&str>) -> Result<Listing> {
    pliego_portfolio::list(path, password)
}

/// Extrae los adjuntos `names` (todos si está vacío) a una carpeta nueva `<nombre> (<suffix>)` junto a `path`.
pub fn extract_copy(
    path: &Path,
    names: &[String],
    password: Option<&str>,
    suffix: &str,
    ctx: &JobContext,
) -> Result<Report> {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "documento".into());
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let out_dir = unique_in(dir, &stem, suffix, "");
    pliego_portfolio::extract(path, &out_dir, names, password, ctx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn needs_at_least_one_file() {
        let err = create_portfolio(&[], "cartera", &JobContext::silent()).unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)));
    }

    #[test]
    fn create_list_and_extract_roundtrip_without_touching_inputs() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("notas.txt");
        let b = dir.path().join("datos.csv");
        std::fs::write(&a, "hola").unwrap();
        std::fs::write(&b, "x,y\n1,2\n").unwrap();
        let report = create_portfolio(&[a.clone(), b.clone()], "cartera", &JobContext::silent()).unwrap();
        let pdf = report.outputs[0].clone();
        assert_eq!(pdf, dir.path().join("notas (cartera).pdf"));

        let listing = list(&pdf, None).unwrap();
        assert!(listing.is_portfolio);
        assert_eq!(listing.entries.len(), 2);

        let out = extract_copy(&pdf, &[], None, "adjuntos", &JobContext::silent()).unwrap();
        assert_eq!(out.outputs.len(), 2);
        assert!(out.outputs[0].starts_with(dir.path().join("notas (cartera) (adjuntos)")));
        let names: Vec<_> = out.outputs.iter().map(|p| std::fs::read(p).unwrap()).collect();
        assert!(names.contains(&b"hola".to_vec()));
        assert_eq!(std::fs::read(&a).unwrap(), b"hola");

        // Una segunda cartera no pisa la primera.
        let again = create_portfolio(&[a, b], "cartera", &JobContext::silent()).unwrap();
        assert_eq!(again.outputs[0], dir.path().join("notas (cartera 2).pdf"));
    }
}
