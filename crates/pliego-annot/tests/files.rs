use mupdf::Size;
use mupdf::pdf::PdfDocument;
use pliego_annot::files::{export_xfdf_file, import_xfdf_file, open, save, summary_of};
use pliego_annot::*;

fn note(page: usize, text: &str) -> AnnotSpec {
    let mut s = AnnotSpec::new(page, Geometry::Note { at: [50.0, 50.0] });
    s.author = Some("Ana".into());
    s.contents = Some(text.into());
    s
}

fn pdf_with_notes(path: &std::path::Path, n: usize) {
    let mut doc = PdfDocument::new();
    doc.new_page(Size::A4).unwrap();
    for i in 0..n {
        add(&mut doc, &note(0, &format!("nota {i}"))).unwrap();
    }
    save(&doc, path).unwrap();
}

#[test]
fn summary_counts_comments() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("a.pdf");
    pdf_with_notes(&p, 2);
    let s = summary_of(&p, None).unwrap();
    assert_eq!(s.total, 2);
    assert_eq!(s.by_author.get("Ana"), Some(&2));
}

#[test]
fn xfdf_roundtrip_goes_to_a_copy_and_leaves_the_original() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("a.pdf");
    pdf_with_notes(&src, 2);
    let xfdf = dir.path().join("a.xfdf");
    assert_eq!(export_xfdf_file(&src, None, &xfdf).unwrap(), 2);
    assert!(std::fs::read_to_string(&xfdf).unwrap().contains("nota 0"));

    // Un PDF limpio de la misma forma recibe los comentarios.
    let mut blank = PdfDocument::new();
    blank.new_page(Size::A4).unwrap();
    let clean = dir.path().join("b.pdf");
    save(&blank, &clean).unwrap();
    let before = std::fs::read(&clean).unwrap();
    let out = dir.path().join("b (con comentarios).pdf");
    assert_eq!(import_xfdf_file(&clean, None, &xfdf, &out).unwrap(), 2);
    assert_eq!(std::fs::read(&clean).unwrap(), before, "el original no cambia");
    assert_eq!(summary_of(&out, None).unwrap().total, 2);
}

#[test]
fn missing_file_is_an_error() {
    assert!(open(std::path::Path::new("/no/existe.pdf"), None).is_err());
}
