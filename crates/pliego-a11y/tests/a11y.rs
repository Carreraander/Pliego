use std::path::PathBuf;

use pliego_a11y::{FixOptions, IssueCode, Severity, check, check_file, fix_file, set_alt_text, set_language};

fn corpus(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/a11y")
        .join(name)
}

fn codes(name: &str) -> Vec<IssueCode> {
    check_file(&corpus(name), None)
        .unwrap()
        .issues
        .iter()
        .map(|i| i.code)
        .collect()
}

#[test]
fn good_document_has_no_errors() {
    let r = check_file(&corpus("bueno.pdf"), None).unwrap();
    assert!(r.passes(), "{:#?}", r.issues);
    assert!(r.tagged);
    assert_eq!(r.language.as_deref(), Some("es-ES"));
    assert_eq!(r.title.as_deref(), Some("Documento de prueba"));
    assert_eq!(r.warnings(), 0, "{:#?}", r.issues);
}

#[test]
fn untagged_document_reports_all_basic_defects() {
    let c = codes("malo.pdf");
    for expected in [
        IssueCode::NoStructureTree,
        IssueCode::NoLanguage,
        IssueCode::NoTitle,
        IssueCode::TitleNotDisplayed,
        IssueCode::FieldWithoutLabel,
        IssueCode::LinkWithoutDescription,
        IssueCode::NoTabOrder,
    ] {
        assert!(c.contains(&expected), "falta {expected:?} en {c:?}");
    }
}

#[test]
fn tagged_document_with_defects() {
    let r = check_file(&corpus("malo_etiquetado.pdf"), None).unwrap();
    let c: Vec<_> = r.issues.iter().map(|i| i.code).collect();
    for expected in [
        IssueCode::InvalidLanguage,
        IssueCode::FigureWithoutAlt,
        IssueCode::HeadingSkipped,
        IssueCode::TableWithoutHeader,
        IssueCode::FieldWithoutLabel,
        IssueCode::NoTitle,
    ] {
        assert!(c.contains(&expected), "falta {expected:?} en {c:?}");
    }
    assert!(!r.has(IssueCode::NoStructureTree));
    assert!(!r.passes());
    // cada hallazgo trae un mensaje y una sugerencia
    assert!(
        r.issues
            .iter()
            .all(|i| !i.message.is_empty() && !i.suggestion.is_empty())
    );
}

#[test]
fn fix_language_title_alt_and_tabs() {
    let input = corpus("malo_etiquetado.pdf");
    let r = check_file(&input, None).unwrap();
    let fig = r.issues.iter().find(|i| i.code == IssueCode::FigureWithoutAlt).unwrap();
    let xref = fig.xref.expect("la figura debe indicar su objeto");

    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("arreglado.pdf");
    let opts = FixOptions {
        language: Some("es-ES".into()),
        title: Some("Informe ñandú".into()),
        alt_texts: vec![(xref, "Un cuadrado negro".into())],
        tab_order: true,
    };
    let report = fix_file(&input, &out, &opts, None).unwrap();
    assert_eq!(report.outputs, vec![out.clone()]);

    let after = check_file(&out, None).unwrap();
    for gone in [
        IssueCode::InvalidLanguage,
        IssueCode::NoLanguage,
        IssueCode::NoTitle,
        IssueCode::TitleNotDisplayed,
        IssueCode::FigureWithoutAlt,
        IssueCode::NoTabOrder,
    ] {
        assert!(!after.has(gone), "{gone:?} sigue presente: {:#?}", after.issues);
    }
    assert_eq!(after.title.as_deref(), Some("Informe ñandú"));
    // lo que no se tocó sigue ahí
    assert!(after.has(IssueCode::TableWithoutHeader));
}

#[test]
fn fix_never_overwrites_input() {
    let input = corpus("malo.pdf");
    let opts = FixOptions {
        language: Some("es".into()),
        ..Default::default()
    };
    assert!(fix_file(&input, &input, &opts, None).is_err());
}

#[test]
fn rejects_bad_input() {
    let mut doc = pliego_pdf::PdfDocument::open(corpus("bueno.pdf").to_str().unwrap()).unwrap();
    assert!(set_language(&mut doc, "español").is_err());
    assert!(set_alt_text(&mut doc, 0, "x").is_err());
    assert!(set_alt_text(&mut doc, 9999, "x").is_err());
    // el objeto 1 es una fuente, no un elemento de estructura
    assert!(set_alt_text(&mut doc, 1, "x").is_err());
    let r = check(&doc).unwrap();
    assert!(
        r.issues
            .iter()
            .all(|i| i.severity == Severity::Warning || i.severity == Severity::Error)
    );
}

#[test]
fn report_serializes_to_json() {
    let r = check_file(&corpus("malo.pdf"), None).unwrap();
    let v = serde_json::to_value(&r).unwrap();
    assert_eq!(v["tagged"], false);
    assert!(v["issues"].as_array().unwrap().len() >= 5);
}

#[test]
fn missing_file_is_an_error() {
    assert!(check_file(&corpus("no-existe.pdf"), None).is_err());
}

#[test]
fn low_contrast_text_is_reported_once_per_page() {
    let r = check_file(&corpus("malo_contraste.pdf"), None).unwrap();
    let low: Vec<_> = r.issues.iter().filter(|i| i.code == IssueCode::LowContrast).collect();
    assert_eq!(low.len(), 1, "{:?}", r.issues);
    assert_eq!(low[0].page, Some(0));
    assert!(low[0].message.starts_with("2 fragmento"), "{}", low[0].message);
    let good = check_file(&corpus("bueno.pdf"), None).unwrap();
    assert!(good.issues.iter().all(|i| i.code != IssueCode::LowContrast));
}

#[test]
fn reading_order_follows_the_structure_tree() {
    let order = pliego_a11y::reading_order_file(&corpus("bueno.pdf"), None)
        .unwrap()
        .expect("el documento bueno está etiquetado");
    assert!(!order.is_empty());
    assert!(order.iter().all(|i| !i.role.is_empty()));
}

#[test]
fn reading_order_of_untagged_document_is_none() {
    assert!(
        pliego_a11y::reading_order_file(&corpus("malo.pdf"), None)
            .unwrap()
            .is_none()
    );
}
