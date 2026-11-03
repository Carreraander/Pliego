//! Dónde viven los modelos `*.traineddata`.
//!
//! Orden de búsqueda: `PLIEGO_TESSDATA`, carpeta de usuario de Pliego (`%LOCALAPPDATA%\Pliego\tessdata` en Windows,
//! `$XDG_DATA_HOME/pliego/tessdata` o `~/.local/share/pliego/tessdata` en otros sistemas), `TESSDATA_PREFIX`
//! y las rutas habituales del sistema.

use std::path::{Path, PathBuf};

/// Carpeta de modelos propia de Pliego (donde descarga el gestor de modelos).
pub fn user_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("LOCALAPPDATA").map(|p| PathBuf::from(p).join("Pliego").join("tessdata"))
    }
    #[cfg(not(windows))]
    {
        if let Some(x) = std::env::var_os("XDG_DATA_HOME").filter(|v| !v.is_empty()) {
            return Some(PathBuf::from(x).join("pliego").join("tessdata"));
        }
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share/pliego/tessdata"))
    }
}

/// Carpetas candidatas, en orden de prioridad (solo las que existen).
pub fn search_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(p) = std::env::var_os("PLIEGO_TESSDATA") {
        dirs.push(p.into());
    }
    dirs.extend(user_dir());
    if let Some(p) = std::env::var_os("TESSDATA_PREFIX") {
        let p = PathBuf::from(p);
        dirs.push(p.join("tessdata"));
        dirs.push(p);
    }
    if let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_path_buf))
    {
        dirs.push(exe_dir.join("tessdata"));
    }
    for p in [
        "/usr/share/tesseract-ocr/5/tessdata",
        "/usr/share/tesseract-ocr/4.00/tessdata",
        "/usr/share/tessdata",
        "/usr/local/share/tessdata",
        "/opt/homebrew/share/tessdata",
    ] {
        dirs.push(p.into());
    }
    let mut seen = Vec::new();
    dirs.retain(|d| {
        let keep = d.is_dir() && !seen.contains(d);
        seen.push(d.clone());
        keep
    });
    dirs
}

/// Carpeta donde está el modelo de `lang`, si está instalado.
pub fn find_model_dir(lang: &str) -> Option<PathBuf> {
    search_dirs()
        .into_iter()
        .find(|d| d.join(format!("{lang}.traineddata")).is_file())
}

/// Códigos de idioma instalados (sin `osd`), ordenados y sin repetir.
pub fn available_languages() -> Vec<String> {
    let mut langs: Vec<String> = search_dirs()
        .iter()
        .filter_map(|d| std::fs::read_dir(d).ok())
        .flatten()
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            name.strip_suffix(".traineddata").map(str::to_owned)
        })
        .filter(|l| l != "osd")
        .collect();
    langs.sort();
    langs.dedup();
    langs
}

/// Carpeta única que contenga todos los idiomas de `langs` (p. ej. `"spa+eng"`). Tesseract solo admite una carpeta
/// de datos, así que si los modelos están repartidos se avisa con un error claro.
pub fn resolve_for(langs: &str) -> pliego_core::Result<PathBuf> {
    use pliego_core::Error;
    let codes: Vec<&str> = langs.split('+').map(str::trim).filter(|c| !c.is_empty()).collect();
    if codes.is_empty() {
        return Err(Error::InvalidInput("no se indicó ningún idioma de OCR".into()));
    }
    for c in &codes {
        if !c.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_') {
            return Err(Error::InvalidInput(format!("código de idioma no válido: {c:?}")));
        }
    }
    for d in search_dirs() {
        if codes.iter().all(|c| d.join(format!("{c}.traineddata")).is_file()) {
            return Ok(d);
        }
    }
    let missing: Vec<&str> = codes.iter().copied().filter(|c| find_model_dir(c).is_none()).collect();
    if missing.is_empty() {
        Err(Error::Unsupported(format!(
            "los modelos de {langs} están en carpetas distintas; cópialos a una sola (por ejemplo {})",
            user_dir().map(|p| p.display().to_string()).unwrap_or_default()
        )))
    } else {
        Err(Error::InvalidInput(format!(
            "falta el modelo de idioma: {}. Copia su archivo .traineddata (tessdata_fast) en {}",
            missing.join(", "),
            user_dir().map(|p| p.display().to_string()).unwrap_or_default()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_bad_codes() {
        assert!(resolve_for("").is_err());
        assert!(resolve_for("../x").is_err());
    }
}
