//! Formularios, enlaces internos y comentarios al combinar.
mod common;

use std::path::PathBuf;

use pliego_core::{JobContext, Report};
use pliego_organize::*;

fn corpus(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/organize")
        .join(name)
}

fn merged(inputs: &[&str]) -> (Report, PdfDocument, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("m.pdf");
    let files: Vec<_> = inputs.iter().map(|n| corpus(n)).collect();
    let rep = merge(&files, &out, &MergeOptions::default(), &JobContext::silent()).unwrap();
    (rep, open(&out, None).unwrap(), dir)
}

/// Nombre y valor de cada campo con widget, página a página.
fn fields(doc: &PdfDocument) -> Vec<Vec<(String, String)>> {
    (0..page_count(doc).unwrap())
        .map(|i| {
            let page = doc.load_pdf_page(i as i32).unwrap();
            page.widgets()
                .map(|w| {
                    (
                        w.name().unwrap().unwrap_or_default(),
                        w.value().unwrap().unwrap_or_default(),
                    )
                })
                .collect()
        })
        .collect()
}

fn pair(a: &str, b: &str) -> (String, String) {
    (a.to_string(), b.to_string())
}

#[test]
fn form_fields_keep_values_and_get_unique_names() {
    let (rep, doc, _d) = merged(&["form1.pdf", "form2.pdf", "form1.pdf"]);
    let f = fields(&doc);
    assert_eq!(
        f,
        vec![
            vec![pair("nombre", "valor0")],
            vec![pair("email", "valor1")],
            vec![pair("nombre_2", "valor0")],
            vec![pair("nombre_3", "valor0")],
            vec![pair("email_2", "valor1")],
        ]
    );
    assert!(
        rep.warnings.iter().any(|w| w.contains("3 campos")),
        "{:?}",
        rep.warnings
    );
    // Todos los campos raíz figuran en /AcroForm /Fields.
    let acro = doc.catalog().unwrap().get_dict("AcroForm").unwrap().unwrap();
    assert_eq!(acro.get_dict("Fields").unwrap().unwrap().len().unwrap(), 5);
    assert!(acro.get_dict("DR").unwrap().is_some() && acro.get_dict("DA").unwrap().is_some());
}

#[test]
fn internal_links_point_to_their_own_pages() {
    let (_r, doc, _d) = merged(&["a.pdf", "links.pdf", "links.pdf"]);
    // a.pdf ocupa 0..3; links.pdf 3..5 y 5..7.
    for (page, target) in [(3, 4), (5, 6)] {
        let links: Vec<_> = doc.load_page(page).unwrap().links().unwrap().collect();
        assert_eq!(links.len(), 2, "página {page}");
        let internal = links.iter().find(|l| l.dest.is_some()).expect("enlace interno");
        assert_eq!(internal.dest.unwrap().page, target);
        assert!(links.iter().any(|l| l.uri == "https://example.org/"));
    }
    // No se arrastran páginas de origen: el número de páginas es el esperado.
    assert_eq!(page_count(&doc).unwrap(), 7);
}

#[test]
fn extract_and_split_keep_annotations() {
    let doc = open(&corpus("form1.pdf"), None).unwrap();
    // Extraer la página 2 (campo "email") lleva su campo.
    let out = extract(&doc, &[1]).unwrap();
    assert_eq!(fields(&out), vec![vec![pair("email", "valor1")]]);
    // El original no se modifica.
    assert_eq!(
        fields(&doc),
        vec![vec![pair("nombre", "valor0")], vec![pair("email", "valor1")]]
    );

    let links = open(&corpus("links.pdf"), None).unwrap();
    // Con las dos páginas el enlace interno sigue apuntando a la 2; solo con la 1 queda sin destino interno.
    let both = extract(&links, &[0, 1]).unwrap();
    let l: Vec<_> = both.load_page(0).unwrap().links().unwrap().collect();
    assert_eq!(l.iter().find(|l| l.dest.is_some()).unwrap().dest.unwrap().page, 1);
    let only_first = extract(&links, &[0]).unwrap();
    assert_eq!(page_count(&only_first).unwrap(), 1);
    let l: Vec<_> = only_first.load_page(0).unwrap().links().unwrap().collect();
    assert!(l.iter().all(|l| l.dest.is_none()) && l.iter().any(|l| l.uri == "https://example.org/"));

    let dir = tempfile::tempdir().unwrap();
    let rep = split(
        &doc,
        "f",
        &SplitRule::EveryN { n: 1 },
        dir.path(),
        &JobContext::silent(),
    )
    .unwrap();
    assert_eq!(
        fields(&open(&rep.outputs[0], None).unwrap()),
        vec![vec![pair("nombre", "valor0")]]
    );
}
