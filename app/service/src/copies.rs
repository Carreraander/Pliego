//! Nombres de las copias que guardan las herramientas («contrato (comprimido).pdf», «contrato (comprimido 2).pdf»…).
//!
//! Las herramientas de la app no sobrescriben el original: guardan una copia junto a él y ofrecen abrirla.

use std::path::{Path, PathBuf};

/// Ruta de la copia de `input` con la etiqueta `suffix` y la extensión `ext` (sin punto), junto al original.
/// Si ya existe, añade un número: `nombre (etiqueta 2).ext`, `nombre (etiqueta 3).ext`…
pub fn copy_path(input: &Path, suffix: &str, ext: &str) -> PathBuf {
    let stem = input
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "documento".into());
    let suffix: String = suffix
        .chars()
        .filter(|c| !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') && !c.is_control())
        .collect();
    let suffix = suffix.trim();
    let suffix = if suffix.is_empty() { "copia" } else { suffix };
    let ext = ext.trim_start_matches('.');
    let dir = input.parent().map(Path::to_path_buf).unwrap_or_default();
    let first = dir.join(format!("{stem} ({suffix}).{ext}"));
    if !first.exists() {
        return first;
    }
    (2u32..)
        .map(|n| dir.join(format!("{stem} ({suffix} {n}).{ext}")))
        .find(|p| !p.exists())
        .expect("siempre hay un nombre libre")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_next_to_the_original_and_never_collide() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("contrato.pdf");
        std::fs::write(&input, b"x").unwrap();
        let a = copy_path(&input, "comprimido", "pdf");
        assert_eq!(a, dir.path().join("contrato (comprimido).pdf"));
        std::fs::write(&a, b"x").unwrap();
        let b = copy_path(&input, "comprimido", ".pdf");
        assert_eq!(b, dir.path().join("contrato (comprimido 2).pdf"));
        std::fs::write(&b, b"x").unwrap();
        assert_eq!(
            copy_path(&input, "comprimido", "pdf"),
            dir.path().join("contrato (comprimido 3).pdf")
        );
    }

    #[test]
    fn suffix_cannot_escape_the_folder() {
        let p = copy_path(Path::new("/tmp/zz-no-existe/a.pdf"), "../x/y", "pdf");
        assert_eq!(p, Path::new("/tmp/zz-no-existe").join("a (..xy).pdf"));
        let q = copy_path(Path::new("/tmp/zz-no-existe/a.pdf"), "  ", "pdf");
        assert_eq!(q, Path::new("/tmp/zz-no-existe").join("a (copia).pdf"));
    }
}
