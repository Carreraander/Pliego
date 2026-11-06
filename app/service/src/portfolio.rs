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

/// Adjunta `files` al PDF en su sitio, con copia previa `<nombre> (<backup_suffix>).pdf` y deshacer compartido.
/// Devuelve los bytes nuevos para recargar el visor.
pub fn attachments_add(
    editor: &crate::annot::Editor,
    path: &Path,
    password: Option<&str>,
    files: &[PathBuf],
    backup_suffix: &str,
) -> Result<Vec<u8>> {
    if files.iter().any(|f| same_file(f, path)) {
        return Err(Error::InvalidInput("un PDF no puede adjuntarse a sí mismo".into()));
    }
    editor.edit_pdf_preserving(path, password, backup_suffix, |doc| {
        pliego_portfolio::add_files(doc, files).map(|_| ())
    })
}

/// Quita los adjuntos `names` del PDF en su sitio (copia previa y deshacer). Guarda el archivo entero, no con un
/// añadido al final: así los datos del adjunto desaparecen del archivo y no quedan recuperables en una versión anterior.
pub fn attachments_remove(
    editor: &crate::annot::Editor,
    path: &Path,
    password: Option<&str>,
    names: &[String],
    backup_suffix: &str,
) -> Result<Vec<u8>> {
    editor.edit_pdf(path, password, backup_suffix, |doc| {
        pliego_portfolio::remove_files(doc, names).map(|_| ())
    })
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
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

    #[test]
    fn attachments_are_added_and_removed_in_place_with_undo() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/basic/hola.pdf");
        if !src.exists() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("doc.pdf");
        std::fs::copy(&src, &input).unwrap();
        let note = dir.path().join("notas de año.txt");
        std::fs::write(&note, "SECRETO-NO-DEBE-QUEDAR ".repeat(4)).unwrap();
        let before = std::fs::read(&input).unwrap();
        let editor = crate::annot::Editor::default();

        attachments_add(
            &editor,
            &input,
            None,
            &[note.clone(), note.clone()],
            "antes de los adjuntos",
        )
        .unwrap();
        let names: Vec<String> = list(&input, None)
            .unwrap()
            .entries
            .into_iter()
            .map(|e| e.filename)
            .collect();
        assert_eq!(names, vec!["notas de año (2).txt", "notas de año.txt"]);
        assert_eq!(
            std::fs::read(dir.path().join("doc (antes de los adjuntos).pdf")).unwrap(),
            before
        );

        attachments_remove(
            &editor,
            &input,
            None,
            &["notas de año.txt".into()],
            "antes de los adjuntos",
        )
        .unwrap();
        assert_eq!(list(&input, None).unwrap().entries.len(), 1);
        attachments_remove(
            &editor,
            &input,
            None,
            &["notas de año (2).txt".into()],
            "antes de los adjuntos",
        )
        .unwrap();
        let raw = std::fs::read(&input).unwrap();
        assert!(list(&input, None).unwrap().entries.is_empty());
        assert!(
            !raw.windows(7).any(|w| w == b"SECRETO"),
            "el adjunto quitado no debe quedar en el archivo"
        );
        assert!(attachments_remove(&editor, &input, None, &["no está".into()], "x").is_err());
        assert!(attachments_add(&editor, &input, None, std::slice::from_ref(&input), "x").is_err());

        for _ in 0..3 {
            editor.undo(&input).unwrap();
        }
        assert_eq!(std::fs::read(&input).unwrap(), before);
    }
}
