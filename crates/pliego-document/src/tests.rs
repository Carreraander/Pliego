use std::path::PathBuf;

use super::*;

fn corpus(dir: &str, name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus")
        .join(dir)
        .join(name)
}

fn title(doc: &Document) -> Option<String> {
    let info = doc.pdf().trailer().ok()?.get_dict("Info").ok()??;
    let t = info.get_dict("Title").ok()??;
    t.as_string().ok().map(|s| s.to_string())
}

fn set_title(doc: &mut Document, t: &str) {
    doc.with_pdf_mut(|pdf| {
        let mut info = pdf
            .trailer()
            .map_err(pdf_err)?
            .get_dict("Info")
            .map_err(pdf_err)?
            .expect("Info");
        let v = pdf.new_string(t).map_err(pdf_err)?;
        info.dict_put("Title", v).map_err(pdf_err)
    })
    .unwrap();
}

fn copy_to(dir: &Path, dir_name: &str, name: &str) -> PathBuf {
    let dst = dir.join(name);
    fs::copy(corpus(dir_name, name), &dst).unwrap();
    dst
}

#[test]
fn revision_undo_redo() {
    let mut doc = Document::open(corpus("basic", "hola.pdf"), None).unwrap();
    assert_eq!(doc.revision(), 0);
    assert!(!doc.can_undo() && !doc.is_modified());
    assert!(!doc.undo().unwrap());
    set_title(&mut doc, "Uno");
    let r1 = doc.revision();
    set_title(&mut doc, "Dos");
    assert!(doc.revision() > r1 && r1 > 0);
    assert_eq!(title(&doc).as_deref(), Some("Dos"));
    assert!(doc.is_modified());

    let r2 = doc.revision();
    assert!(doc.undo().unwrap());
    assert_eq!(title(&doc).as_deref(), Some("Uno"));
    assert_ne!(
        doc.revision(),
        r2,
        "deshacer cambia la revisión para invalidar la caché"
    );
    assert!(doc.undo().unwrap());
    assert_eq!(title(&doc).as_deref(), Some("Documento de prueba"));
    assert!(!doc.undo().unwrap());

    assert!(doc.redo().unwrap());
    assert!(doc.redo().unwrap());
    assert_eq!(title(&doc).as_deref(), Some("Dos"));
    assert!(!doc.redo().unwrap());

    // Una operación nueva descarta el rehacer.
    doc.undo().unwrap();
    set_title(&mut doc, "Tres");
    assert!(!doc.can_redo());
}

#[test]
fn failed_operation_rolls_back() {
    let mut doc = Document::open(corpus("basic", "hola.pdf"), None).unwrap();
    let before = doc.revision();
    let res: Result<()> = doc.with_pdf_mut(|pdf| {
        let mut info = pdf.trailer().unwrap().get_dict("Info").unwrap().unwrap();
        info.dict_put("Title", pdf.new_string("Roto").unwrap()).unwrap();
        Err(Error::Other("fallo simulado".into()))
    });
    assert!(res.is_err());
    assert_eq!(doc.revision(), before);
    assert_eq!(title(&doc).as_deref(), Some("Documento de prueba"));
    assert!(!doc.can_undo());
}

#[test]
fn undo_limits_drop_oldest() {
    let mut doc = Document::open(corpus("basic", "hola.pdf"), None).unwrap();
    doc.set_undo_limits(UndoLimits {
        max_steps: 2,
        ..Default::default()
    });
    for t in ["a", "b", "c", "d"] {
        set_title(&mut doc, t);
    }
    assert!(doc.undo().unwrap() && doc.undo().unwrap());
    assert!(!doc.undo().unwrap());
    assert_eq!(title(&doc).as_deref(), Some("b"));
}

#[test]
fn atomic_save_and_roundtrip() {
    let tmp = tempfile::tempdir().unwrap();
    let src = copy_to(tmp.path(), "basic", "hola.pdf");
    let mut doc = Document::open(&src, None).unwrap();
    set_title(&mut doc, "Guardado");
    let out = tmp.path().join("salida.pdf");
    doc.save(&out, &SaveOptions::default()).unwrap();
    assert!(!doc.is_modified());
    // El original no se toca y no quedan temporales.
    assert_eq!(fs::read(&src).unwrap(), fs::read(corpus("basic", "hola.pdf")).unwrap());
    let names: Vec<_> = fs::read_dir(tmp.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names.len(), 2, "{names:?}");
    let reopened = Document::open(&out, None).unwrap();
    assert_eq!(title(&reopened).as_deref(), Some("Guardado"));
    // Windows no deja reemplazar un archivo abierto: se suelta antes de volver a guardar.
    drop(reopened);
    // Se puede seguir editando y guardando tras guardar.
    set_title(&mut doc, "Otra vez");
    doc.save(&out, &SaveOptions::default()).unwrap();
    assert_eq!(title(&Document::open(&out, None).unwrap()).as_deref(), Some("Otra vez"));
}

#[test]
fn failed_save_leaves_destination_intact() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("no_existe").join("x.pdf");
    let mut doc = Document::open(corpus("basic", "hola.pdf"), None).unwrap();
    assert!(doc.save(&out, &SaveOptions::default()).is_err());
}

#[test]
fn signed_document_saves_incrementally() {
    let tmp = tempfile::tempdir().unwrap();
    let src = copy_to(tmp.path(), "document", "firmado.pdf");
    let original = fs::read(&src).unwrap();
    let mut doc = Document::open(&src, None).unwrap();
    assert!(doc.is_signed());
    set_title(&mut doc, "Con anotación");
    doc.save_incremental().unwrap();
    let saved = fs::read(&src).unwrap();
    assert!(saved.len() > original.len());
    assert_eq!(&saved[..original.len()], &original[..], "los bytes firmados no cambian");
    assert_eq!(
        title(&Document::open(&src, None).unwrap()).as_deref(),
        Some("Con anotación")
    );
}

#[test]
fn signed_document_save_is_incremental_by_default() {
    let tmp = tempfile::tempdir().unwrap();
    let src = copy_to(tmp.path(), "document", "firmado.pdf");
    let original = fs::read(&src).unwrap();
    let mut doc = Document::open(&src, None).unwrap();
    set_title(&mut doc, "X");
    doc.save(&src, &SaveOptions::default()).unwrap();
    assert_eq!(&fs::read(&src).unwrap()[..original.len()], &original[..]);
    // Y se puede volver a guardar incrementalmente después.
    set_title(&mut doc, "Y");
    doc.save_incremental().unwrap();
    assert_eq!(title(&Document::open(&src, None).unwrap()).as_deref(), Some("Y"));
}

#[test]
fn signed_document_refuses_unsafe_save() {
    let tmp = tempfile::tempdir().unwrap();
    let src = copy_to(tmp.path(), "document", "firmado.pdf");
    let mut doc = Document::open(&src, None).unwrap();
    set_title(&mut doc, "A");
    set_title(&mut doc, "B");
    doc.undo().unwrap();
    let before = fs::read(&src).unwrap();
    let err = doc.save(&src, &SaveOptions::default()).unwrap_err();
    assert!(matches!(err, Error::Unsupported(_)), "{err}");
    assert_eq!(fs::read(&src).unwrap(), before);
    // Con permiso explícito se guarda de forma completa.
    let copy = tmp.path().join("copia.pdf");
    doc.save(
        &copy,
        &SaveOptions {
            allow_invalidate_signatures: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(title(&Document::open(&copy, None).unwrap()).as_deref(), Some("A"));
}

#[test]
fn unsigned_document_is_not_signed() {
    let doc = Document::open(corpus("basic", "hola.pdf"), None).unwrap();
    assert!(!doc.is_signed());
    assert!(matches!(
        Document::open_bytes(b"esto no es un pdf", None),
        Err(Error::Pdf(_))
    ));
}

#[test]
fn repair_truncated_pdf() {
    let tmp = tempfile::tempdir().unwrap();
    let mut bytes = fs::read(corpus("basic", "hola.pdf")).unwrap();
    // Corta la tabla xref y el trailer: MuPDF debe reconstruirlos.
    let cut = bytes.windows(4).rposition(|w| w == b"xref").unwrap();
    bytes.truncate(cut);
    let broken = tmp.path().join("roto.pdf");
    fs::write(&broken, &bytes).unwrap();
    let fixed = tmp.path().join("reparado.pdf");
    repair(&broken, &fixed, None).unwrap();
    let doc = Document::open(&fixed, None).unwrap();
    assert_eq!(doc.pdf().page_count().unwrap(), 2);
    assert!(repair(&broken, &broken, None).is_err());
    assert_eq!(fs::read(&broken).unwrap(), bytes, "el original no se modifica");
}
