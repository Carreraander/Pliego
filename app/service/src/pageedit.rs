//! Edición de texto sobre el lienzo (W29b), sin Tauri para probarla en Linux;
//! `app/src-tauri/src/commands/pageedit.rs` solo traduce argumentos.
//!
//! Las coordenadas son las del visor (puntos de página, origen arriba a la izquierda, con el giro aplicado), que son
//! también las de `pliego-edit`. Los cambios van al propio PDF con copia de seguridad y comparten la pila de deshacer
//! de [`Editor`]. Cada edición devuelve el [`Report`] de `pliego-edit`, cuyos avisos dicen si hubo que cambiar de fuente.

use std::path::Path;

use pliego_core::{Error, Report, Result};
use pliego_document::Document;

pub use pliego_edit::{Align, FontFamily, RectPts, Style, TextBlock};

use crate::annot::Editor;

/// Bloques de texto editables de una página (desde 0).
pub fn blocks(path: &Path, page: usize, password: Option<&str>) -> Result<Vec<TextBlock>> {
    let doc = Document::open(path, password)?;
    pliego_edit::text_blocks(&doc, page)
}

/// Resultado de una edición: contenido nuevo del archivo (para recargar el visor) y avisos.
pub struct Edited {
    pub bytes: Vec<u8>,
    pub report: Report,
}

fn run(
    editor: &Editor,
    path: &Path,
    backup_suffix: &str,
    op: impl FnOnce(&mut Document) -> Result<Report>,
) -> Result<Edited> {
    let mut report = None;
    let bytes = editor.edit_document(path, backup_suffix, |doc| {
        report = Some(op(doc)?);
        Ok(())
    })?;
    Ok(Edited {
        bytes,
        report: report.unwrap_or_default(),
    })
}

/// Sustituye el texto del bloque `block` (id de [`blocks`]) de la página `page`.
pub fn replace(
    editor: &Editor,
    path: &Path,
    page: usize,
    block: usize,
    text: &str,
    style: &Style,
    backup_suffix: &str,
) -> Result<Edited> {
    run(editor, path, backup_suffix, |doc| {
        pliego_edit::replace_text(doc, page, block, text, style)
    })
}

/// Añade texto nuevo dentro de `rect`.
pub fn insert(
    editor: &Editor,
    path: &Path,
    page: usize,
    rect: RectPts,
    text: &str,
    style: &Style,
    backup_suffix: &str,
) -> Result<Edited> {
    if text.trim().is_empty() {
        return Err(Error::InvalidInput("escribe el texto que quieres añadir".into()));
    }
    run(editor, path, backup_suffix, |doc| {
        pliego_edit::insert_text(doc, page, rect, text, style)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const CORPUS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/corpus/edit/parrafos.pdf");

    fn copy(dir: &Path) -> std::path::PathBuf {
        let p = dir.join("p.pdf");
        std::fs::copy(CORPUS, &p).unwrap();
        p
    }

    #[test]
    fn lists_blocks() {
        let dir = tempfile::tempdir().unwrap();
        let p = copy(dir.path());
        let b = blocks(&p, 0, None).unwrap();
        assert_eq!(b.len(), 2);
        assert!(b[0].text.starts_with("Primer parrafo"));
        assert!(blocks(&p, 9, None).is_err());
    }

    #[test]
    fn replace_edits_the_file_keeps_a_backup_and_undoes() {
        let dir = tempfile::tempdir().unwrap();
        let p = copy(dir.path());
        let original = std::fs::read(&p).unwrap();
        let ed = Editor::default();
        let out = replace(
            &ed,
            &p,
            0,
            0,
            "Texto nuevo del bloque",
            &Style::default(),
            "copia de seguridad",
        )
        .unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), out.bytes);
        let texts: Vec<_> = blocks(&p, 0, None).unwrap().into_iter().map(|b| b.text).collect();
        assert!(texts.iter().any(|t| t.contains("Texto nuevo")), "{texts:?}");
        assert_eq!(
            std::fs::read(dir.path().join("p (copia de seguridad).pdf")).unwrap(),
            original
        );
        ed.undo(&p).unwrap().unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), original);
    }

    #[test]
    fn insert_adds_text_and_rejects_blank_or_missing_block() {
        let dir = tempfile::tempdir().unwrap();
        let p = copy(dir.path());
        let ed = Editor::default();
        insert(
            &ed,
            &p,
            0,
            [60.0, 600.0, 300.0, 640.0],
            "Añadido aquí",
            &Style::default(),
            "c",
        )
        .unwrap();
        let texts: Vec<_> = blocks(&p, 0, None).unwrap().into_iter().map(|b| b.text).collect();
        assert!(texts.iter().any(|t| t.contains("Añadido")), "{texts:?}");
        let before = std::fs::read(&p).unwrap();
        assert!(insert(&ed, &p, 0, [0.0, 0.0, 10.0, 10.0], "  ", &Style::default(), "c").is_err());
        assert!(replace(&ed, &p, 0, 99, "x", &Style::default(), "c").is_err());
        assert_eq!(std::fs::read(&p).unwrap(), before);
        assert_eq!(ed.undo_depth(&p), 1);
    }
}
