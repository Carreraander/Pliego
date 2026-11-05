//! Edición de texto sobre el lienzo (W29b), sin Tauri para probarla en Linux;
//! `app/src-tauri/src/commands/pageedit.rs` solo traduce argumentos.
//!
//! Las coordenadas son las del visor (puntos de página, origen arriba a la izquierda, con el giro aplicado), que son
//! también las de `pliego-edit`. Los cambios van al propio PDF con copia de seguridad y comparten la pila de deshacer
//! de [`Editor`]. Cada edición devuelve el [`Report`] de `pliego-edit`, cuyos avisos dicen si hubo que cambiar de fuente.

use std::path::Path;

use pliego_core::{Error, Report, Result};
use pliego_document::Document;

pub use pliego_edit::{
    Align, FlowOptions, FontFamily, ImageObject, LinkItem, LinkTarget, RectPts, Style, TextBlock, Transform,
    VectorObject,
};

use crate::annot::Editor;

/// Bloques de texto editables de una página (desde 0).
pub fn blocks(path: &Path, page: usize, password: Option<&str>) -> Result<Vec<TextBlock>> {
    let doc = Document::open(path, password)?;
    pliego_edit::text_blocks(&doc, page)
}

/// Imágenes colocadas en una página (desde 0).
pub fn images(path: &Path, page: usize, password: Option<&str>) -> Result<Vec<ImageObject>> {
    let doc = Document::open(path, password)?;
    pliego_edit::images(&doc, page)
}

/// Objetos vectoriales de una página (desde 0).
pub fn vectors(path: &Path, page: usize, password: Option<&str>) -> Result<Vec<VectorObject>> {
    let doc = Document::open(path, password)?;
    pliego_edit::vectors(&doc, page)
}

/// Enlaces de una página (desde 0).
pub fn links(path: &Path, page: usize, password: Option<&str>) -> Result<Vec<LinkItem>> {
    let doc = Document::open(path, password)?;
    pliego_edit::links(&doc, page)
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

/// Sustituye el texto del bloque `block` (id de [`blocks`]) de la página `page`. Con `flow`, lo que hay debajo
/// sube o baja según cambie el número de líneas y lo que no cabe pasa a la página siguiente (W43).
#[allow(clippy::too_many_arguments)]
pub fn replace(
    editor: &Editor,
    path: &Path,
    page: usize,
    block: usize,
    text: &str,
    style: &Style,
    flow: bool,
    backup_suffix: &str,
) -> Result<Edited> {
    let opts = if flow {
        FlowOptions::default()
    } else {
        FlowOptions::NONE
    };
    run(editor, path, backup_suffix, |doc| {
        pliego_edit::replace_text_flow(doc, page, block, text, style, &opts)
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

/// Tamaño máximo de una imagen que se acepta insertar (el archivo entero se lee en memoria).
const MAX_IMAGE_BYTES: u64 = 64 * 1024 * 1024;

fn read_image(image: &Path) -> Result<Vec<u8>> {
    let len = std::fs::metadata(image)?.len();
    if len > MAX_IMAGE_BYTES {
        return Err(Error::InvalidInput("la imagen pesa más de 64 MB".into()));
    }
    Ok(std::fs::read(image)?)
}

/// Inserta la imagen `image` ocupando `rect` de la página.
pub fn image_insert(
    editor: &Editor,
    path: &Path,
    page: usize,
    rect: RectPts,
    image: &Path,
    backup_suffix: &str,
) -> Result<Edited> {
    let data = read_image(image)?;
    run(editor, path, backup_suffix, |doc| {
        pliego_edit::insert_image(doc, page, rect, &data)
    })
}

/// Sustituye la imagen `id` por `image` conservando su posición y tamaño.
pub fn image_replace(
    editor: &Editor,
    path: &Path,
    page: usize,
    id: usize,
    image: &Path,
    backup_suffix: &str,
) -> Result<Edited> {
    let data = read_image(image)?;
    run(editor, path, backup_suffix, |doc| {
        pliego_edit::replace_image(doc, page, id, &data)
    })
}

/// Recorta la imagen `id` a `rect` (espacio de página).
pub fn image_crop(
    editor: &Editor,
    path: &Path,
    page: usize,
    id: usize,
    rect: RectPts,
    backup_suffix: &str,
) -> Result<Edited> {
    run(editor, path, backup_suffix, |doc| {
        pliego_edit::crop_image(doc, page, id, rect)
    })
}

/// Qué clase de objeto de la página se edita.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectKind {
    Image,
    Vector,
}

/// Mueve, redimensiona y/o gira una imagen o un objeto vectorial.
pub fn object_transform(
    editor: &Editor,
    path: &Path,
    page: usize,
    kind: ObjectKind,
    id: usize,
    t: &Transform,
    backup_suffix: &str,
) -> Result<Edited> {
    run(editor, path, backup_suffix, |doc| match kind {
        ObjectKind::Image => pliego_edit::transform_object(doc, page, id, t),
        ObjectKind::Vector => pliego_edit::transform_vector(doc, page, id, t),
    })
}

/// Borra una imagen o un objeto vectorial.
pub fn object_delete(
    editor: &Editor,
    path: &Path,
    page: usize,
    kind: ObjectKind,
    id: usize,
    backup_suffix: &str,
) -> Result<Edited> {
    run(editor, path, backup_suffix, |doc| match kind {
        ObjectKind::Image => pliego_edit::delete_object(doc, page, id),
        ObjectKind::Vector => pliego_edit::delete_vector(doc, page, id),
    })
}

/// Crea un enlace en `rect`.
pub fn link_add(
    editor: &Editor,
    path: &Path,
    page: usize,
    rect: RectPts,
    target: &LinkTarget,
    backup_suffix: &str,
) -> Result<Edited> {
    run(editor, path, backup_suffix, |doc| {
        pliego_edit::add_link(doc, page, rect, target)
    })
}

/// Cambia el destino del enlace `id`.
pub fn link_update(
    editor: &Editor,
    path: &Path,
    page: usize,
    id: usize,
    target: &LinkTarget,
    backup_suffix: &str,
) -> Result<Edited> {
    run(editor, path, backup_suffix, |doc| {
        pliego_edit::update_link(doc, page, id, target)
    })
}

/// Borra el enlace `id`.
pub fn link_delete(editor: &Editor, path: &Path, page: usize, id: usize, backup_suffix: &str) -> Result<Edited> {
    run(editor, path, backup_suffix, |doc| {
        pliego_edit::delete_link(doc, page, id)
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
            true,
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
        assert!(replace(&ed, &p, 0, 99, "x", &Style::default(), true, "c").is_err());
        assert_eq!(std::fs::read(&p).unwrap(), before);
        assert_eq!(ed.undo_depth(&p), 1);
    }

    #[test]
    fn images_vectors_and_links_roundtrip_with_undo() {
        let dir = tempfile::tempdir().unwrap();
        let p = copy(dir.path());
        let png = dir.path().join("i.png");
        std::fs::copy(
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/corpus/edit/azul.png"),
            &png,
        )
        .unwrap();
        let ed = Editor::default();
        let original = std::fs::read(&p).unwrap();

        image_insert(&ed, &p, 0, [60.0, 400.0, 160.0, 460.0], &png, "c").unwrap();
        let imgs = images(&p, 0, None).unwrap();
        assert_eq!(imgs.len(), 1, "{imgs:?}");
        let t = Transform {
            dx: 20.0,
            dy: 10.0,
            ..Transform::default()
        };
        object_transform(&ed, &p, 0, ObjectKind::Image, imgs[0].id, &t, "c").unwrap();
        let moved = &images(&p, 0, None).unwrap()[0];
        assert!((moved.bbox[0] - imgs[0].bbox[0] - 20.0).abs() < 0.1);
        object_delete(&ed, &p, 0, ObjectKind::Image, moved.id, "c").unwrap();
        assert!(images(&p, 0, None).unwrap().is_empty());

        let target = LinkTarget::Uri {
            uri: "https://pdfpliego.com".into(),
        };
        link_add(&ed, &p, 0, [10.0, 10.0, 100.0, 30.0], &target, "c").unwrap();
        let l = links(&p, 0, None).unwrap();
        assert_eq!(l.len(), 1);
        link_update(&ed, &p, 0, l[0].id, &LinkTarget::Page { page: 0 }, "c").unwrap();
        assert_eq!(
            links(&p, 0, None).unwrap()[0].target,
            Some(LinkTarget::Page { page: 0 })
        );
        link_delete(&ed, &p, 0, links(&p, 0, None).unwrap()[0].id, "c").unwrap();
        assert!(links(&p, 0, None).unwrap().is_empty());

        // Todo se deshace hasta el original.
        while ed.undo(&p).unwrap().is_some() {}
        assert_eq!(std::fs::read(&p).unwrap(), original);
        assert!(image_insert(&ed, &p, 0, [0.0, 0.0, 10.0, 10.0], &dir.path().join("no.png"), "c").is_err());
    }
}
