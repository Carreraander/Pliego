//! Ayudante común de los trabajos «guarda una copia y ofrece abrirla» (Combinar, Dividir, Comprimir…).
//!
//! Por defecto una herramienta no toca el original: escribe `<nombre> (sufijo).pdf` junto a él, y si ya existe,
//! `<nombre> (sufijo 2).pdf`, etc. Así nunca se pisa un resultado anterior. El informe del trabajo
//! (`Report::outputs`) lleva la ruta y el centro de tareas ofrece «Abrir resultado».

use std::path::{Path, PathBuf};

/// Ruta libre `<carpeta de src>/<nombre> (<suffix>).<ext>`; con colisión añade un contador (`(<suffix> 2)`…).
pub fn unique_sibling(src: &Path, suffix: &str, ext: &str) -> PathBuf {
    let dir = src
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let stem = src
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "documento".into());
    unique_in(dir, &stem, suffix, ext)
}

/// Como [`unique_sibling`] pero con carpeta y nombre base explícitos. `ext` vacío = sin extensión (carpetas).
pub fn unique_in(dir: &Path, stem: &str, suffix: &str, ext: &str) -> PathBuf {
    let name = |label: &str| {
        let base = format!("{stem} ({label})");
        if ext.is_empty() { base } else { format!("{base}.{ext}") }
    };
    let first = dir.join(name(suffix));
    if !first.exists() {
        return first;
    }
    (2u32..)
        .map(|n| dir.join(name(&format!("{suffix} {n}"))))
        .find(|p| !p.exists())
        .expect("siempre hay un nombre libre")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_suffix_next_to_original() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("contrato.pdf");
        assert_eq!(
            unique_sibling(&src, "combinado", "pdf"),
            dir.path().join("contrato (combinado).pdf")
        );
    }

    #[test]
    fn never_overwrites_a_previous_result() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.pdf");
        std::fs::write(dir.path().join("a (copia).pdf"), b"x").unwrap();
        std::fs::write(dir.path().join("a (copia 2).pdf"), b"x").unwrap();
        assert_eq!(unique_sibling(&src, "copia", "pdf"), dir.path().join("a (copia 3).pdf"));
    }

    #[test]
    fn folders_have_no_extension() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("a (partes)")).unwrap();
        assert_eq!(
            unique_in(dir.path(), "a", "partes", ""),
            dir.path().join("a (partes 2)")
        );
    }
}
