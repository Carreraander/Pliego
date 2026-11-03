use std::path::{Path, PathBuf};

use pliego_core::{Error, Result};

/// Un PDF de entrada y su ruta relativa en la salida (se conserva la estructura de subcarpetas).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Input {
    pub path: PathBuf,
    pub rel: PathBuf,
}

fn is_pdf(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
}

fn walk(dir: &Path, base: &Path, skip: Option<&Path>, out: &mut Vec<Input>) -> Result<()> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| Error::io_at(dir, e))?
        .collect::<std::io::Result<_>>()
        .map_err(|e| Error::io_at(dir, e))?;
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let p = e.path();
        let ty = e.file_type().map_err(|e| Error::io_at(&p, e))?;
        if ty.is_dir() {
            if skip.is_some_and(|s| p.canonicalize().is_ok_and(|c| c == s)) {
                continue;
            }
            walk(&p, base, skip, out)?;
        } else if ty.is_file() && is_pdf(&p) {
            let rel = p.strip_prefix(base).unwrap_or(&p).to_path_buf();
            out.push(Input { path: p, rel });
        }
    }
    Ok(())
}

/// Expande archivos y carpetas (recursivo) a una lista ordenada de PDFs. `skip_dir` evita volver a procesar
/// la carpeta de salida cuando está dentro de la de entrada.
pub fn collect_inputs(paths: &[PathBuf], skip_dir: Option<&Path>) -> Result<Vec<Input>> {
    let skip = skip_dir.and_then(|d| d.canonicalize().ok());
    let mut out = Vec::new();
    for p in paths {
        if p.is_dir() {
            walk(p, p, skip.as_deref(), &mut out)?;
        } else if p.is_file() {
            let name = p.file_name().map(PathBuf::from).unwrap_or_else(|| p.clone());
            out.push(Input {
                path: p.clone(),
                rel: name,
            });
        } else {
            return Err(Error::io_at(p, std::io::Error::from(std::io::ErrorKind::NotFound)));
        }
    }
    let mut seen = std::collections::HashSet::new();
    for i in &out {
        if !seen.insert(i.rel.clone()) {
            return Err(Error::InvalidInput(format!(
                "dos entradas darían el mismo archivo de salida ({}); procesa carpetas distintas por separado",
                i.rel.display()
            )));
        }
    }
    Ok(out)
}
