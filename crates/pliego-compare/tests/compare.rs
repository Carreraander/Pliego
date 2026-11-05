use std::path::PathBuf;

use pliego_compare::{ChangeKind, CompareOptions, PageStatus, compare, write_report};
use pliego_core::JobContext;

fn corpus(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/compare")
        .join(name)
}

fn run(a: &str, b: &str) -> pliego_compare::Comparison {
    compare(
        &corpus(a),
        &corpus(b),
        &CompareOptions::default(),
        &JobContext::silent(),
    )
    .unwrap()
}

#[test]
fn identical_documents_have_no_differences() {
    let c = run("base.pdf", "base.pdf");
    assert!(c.is_identical(), "{c:#?}");
    assert_eq!(c.summary.pages_a, 2);
    assert_eq!(c.pages.len(), 2);
}

#[test]
fn changed_word_is_found_on_the_right_page() {
    let c = run("base.pdf", "palabra.pdf");
    assert_eq!(c.summary.changed_pages, 1);
    assert!(!c.pages[1].has_differences());
    let p = &c.pages[0];
    assert_eq!(p.text_changes.len(), 1, "{:#?}", p.text_changes);
    let t = &p.text_changes[0];
    assert_eq!(t.kind, ChangeKind::Changed);
    assert_eq!((t.text_a.as_str(), t.text_b.as_str()), ("treinta", "sesenta"));
    assert_eq!((t.page_a, t.page_b), (0, 0));
    assert!(!t.rects_b.is_empty());
    // Y la diferencia visual cae sobre la misma palabra.
    assert!(!p.visual_regions.is_empty());
}

#[test]
fn inserted_paragraph_is_found_on_the_right_page() {
    let c = run("base.pdf", "parrafo.pdf");
    let t = &c.pages[0].text_changes;
    assert_eq!(t.len(), 1, "{t:#?}");
    assert_eq!(t[0].kind, ChangeKind::Added);
    assert!(t[0].text_b.starts_with("Anexo:"));
    assert!(c.summary.added_words >= 14);
    assert!(!c.pages[1].has_differences());
}

#[test]
fn inserted_page_is_detected() {
    let c = run("base.pdf", "pagina.pdf");
    assert_eq!(c.summary.inserted_pages, 1);
    assert_eq!(c.summary.deleted_pages, 0);
    let statuses: Vec<_> = c.pages.iter().map(|p| p.status).collect();
    assert_eq!(
        statuses,
        [PageStatus::Matched, PageStatus::Inserted, PageStatus::Matched]
    );
    assert_eq!(c.pages[1].page_b, Some(1));
    assert_eq!(c.pages[2].page_a, Some(1));
    assert!(!c.pages[2].has_differences());
    // Al revés, es una página eliminada.
    let r = run("pagina.pdf", "base.pdf");
    assert_eq!(r.summary.deleted_pages, 1);
    assert_eq!(r.pages[1].status, PageStatus::Deleted);
}

#[test]
fn format_change_is_reported() {
    let c = run("base.pdf", "formato.pdf");
    let t = &c.pages[0].text_changes;
    assert_eq!(t.len(), 1, "{t:#?}");
    assert!(t[0].format_only);
    assert!(t[0].text_a == "Contrato de servicios");
    let note = t[0].note.as_deref().unwrap();
    assert!(note.contains("tamaño"), "{note}");
    assert_eq!(c.summary.format_changes, 1);
}

#[test]
fn moved_image_is_found_visually() {
    let c = run("imagen_a.pdf", "imagen_b.pdf");
    let p = &c.pages[0];
    assert!(p.text_changes.is_empty());
    assert_eq!(p.visual_regions.len(), 2, "{:#?}", p.visual_regions);
    // Posición antigua: x 72..192, y (842-500-120)=222..342 desde arriba.
    let old = p.visual_regions.iter().find(|r| r.rect.x0 < 100.0).unwrap();
    assert!(
        (old.rect.x0 - 72.0).abs() < 8.0 && (old.rect.y0 - 222.0).abs() < 8.0,
        "{old:?}"
    );
    let new = p.visual_regions.iter().find(|r| r.rect.x0 > 300.0).unwrap();
    assert!(
        (new.rect.x0 - 360.0).abs() < 8.0 && (new.rect.y0 - 422.0).abs() < 8.0,
        "{new:?}"
    );
}

#[test]
fn json_roundtrip() {
    let c = run("base.pdf", "palabra.pdf");
    let json = serde_json::to_string(&c).unwrap();
    let back: pliego_compare::Comparison = serde_json::from_str(&json).unwrap();
    assert_eq!(back.summary, c.summary);
    assert_eq!(back.pages, c.pages);
}

#[test]
fn report_is_a_readable_pdf_with_summary_and_detail_pages() {
    let c = run("base.pdf", "pagina.pdf");
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("informe.pdf");
    write_report(&c, &out).unwrap();
    let doc = pliego_pdf::Document::open(out.to_str().unwrap()).unwrap();
    // Resumen + una página de detalle (la insertada).
    assert_eq!(doc.page_count().unwrap(), 2);
    let text = doc
        .load_page(0)
        .unwrap()
        .to_text_page(pliego_pdf::TextFlags::empty())
        .unwrap()
        .to_text()
        .unwrap();
    assert!(text.contains("Informe de comparación"), "{text}");
    assert!(text.contains("página insertada"), "{text}");
    // Ni el temporal queda atrás ni se tocan las entradas.
    assert!(!out.with_extension("pdf.tmp").exists());
}

#[test]
fn missing_file_is_a_clear_error() {
    let r = compare(
        &corpus("nada.pdf"),
        &corpus("base.pdf"),
        &CompareOptions::default(),
        &JobContext::silent(),
    );
    assert!(matches!(r, Err(pliego_core::Error::Io { .. })));
}
