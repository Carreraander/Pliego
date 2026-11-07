//! Datos del archivo de un documento para «Propiedades del documento» (flujo W29, `viewer.properties`).

use std::path::Path;
use std::time::UNIX_EPOCH;

use pliego_core::{Error, Result};
use serde::Serialize;

pub use pliego_pdfa::InfoEdit;

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

/// Cambia título, autor, asunto y palabras clave del PDF en su sitio, con copia previa `<nombre> (<backup_suffix>).pdf`
/// y deshacer compartido (si es PDF/A, también su XMP para que siga siendo válido). Devuelve los bytes nuevos.
pub fn properties_save(
    editor: &crate::annot::Editor,
    path: &Path,
    password: Option<&str>,
    edit: &InfoEdit,
    backup_suffix: &str,
) -> Result<Vec<u8>> {
    properties_save_saved(editor, path, password, edit, backup_suffix)?.into_bytes()
}

/// Como [`properties_save`], con el resultado como [`crate::annot::Saved`] (PDF grandes: se guarda al final del archivo).
pub fn properties_save_saved(
    editor: &crate::annot::Editor,
    path: &Path,
    password: Option<&str>,
    edit: &InfoEdit,
    backup_suffix: &str,
) -> Result<crate::annot::Saved> {
    if *edit == InfoEdit::default() {
        return Err(Error::InvalidInput("no hay ninguna propiedad que cambiar".into()));
    }
    editor.edit_pdf_preserving_saved(path, password, backup_suffix, |doc| {
        pliego_pdfa::update_info(doc, edit).map(|_| ())
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

    #[test]
    fn properties_are_saved_in_place_with_backup_and_undo() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/basic/hola.pdf");
        if !src.exists() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("doc.pdf");
        std::fs::copy(&src, &input).unwrap();
        let before = std::fs::read(&input).unwrap();
        let editor = crate::annot::Editor::default();
        let edit = InfoEdit {
            title: Some("Presupuesto 2027".into()),
            subject: Some("Obra de la cocina".into()),
            ..Default::default()
        };
        properties_save(&editor, &input, None, &edit, "antes de las propiedades").unwrap();
        let doc = pliego_pdf::PdfDocument::open(input.to_str().unwrap()).unwrap();
        assert_eq!(
            doc.metadata(pliego_pdf::MetadataName::Title).unwrap(),
            "Presupuesto 2027"
        );
        assert_eq!(
            doc.metadata(pliego_pdf::MetadataName::Subject).unwrap(),
            "Obra de la cocina"
        );
        assert_eq!(
            std::fs::read(dir.path().join("doc (antes de las propiedades).pdf")).unwrap(),
            before
        );
        editor.undo(&input).unwrap();
        assert_eq!(std::fs::read(&input).unwrap(), before);
        assert!(matches!(
            properties_save(&editor, &input, None, &InfoEdit::default(), "x"),
            Err(Error::InvalidInput(_))
        ));
    }
}
