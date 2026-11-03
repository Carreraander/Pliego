//! Reutilización de la fuente incrustada del bloque original.

use pliego_document::{Document, SaveOptions};
use pliego_edit::*;

const JA: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/corpus/edit/corpus/cjk-japones.pdf"
);

#[test]
fn reordenar_palabras_usa_la_fuente_incrustada_sin_crecer() {
    let mut doc = Document::open(JA, None).unwrap();
    let b = text_blocks(&doc, 0).unwrap().remove(0);
    let words: Vec<&str> = b.text.split_whitespace().collect();
    assert!(words.len() >= 3, "{b:?}");
    // Mismos caracteres, otro orden: no hace falta ninguna fuente del sistema.
    let nuevo = format!("{} {}", words[2], words[0]);
    let report = replace_text(&mut doc, 0, b.id, &nuevo, &Style::default()).unwrap();
    assert!(
        report
            .warnings
            .iter()
            .any(|w| w.contains("reutilizó la fuente original")),
        "{report:?}"
    );
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("o.pdf");
    doc.save(&out, &SaveOptions::default()).unwrap();
    let before = std::fs::metadata(JA).unwrap().len();
    let after = std::fs::metadata(&out).unwrap().len();
    assert!(after < before + 2_000, "creció de {before} a {after}");
    let re = Document::open(&out, None).unwrap();
    let t = text_blocks(&re, 0)
        .unwrap()
        .iter()
        .map(|b| b.text.clone())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(t.replace(['\n', ' '], "").contains(&nuevo.replace(' ', "")), "{t}");
}

#[test]
fn un_caracter_que_la_fuente_no_tiene_cae_a_la_sustitucion() {
    let mut doc = Document::open(JA, None).unwrap();
    let b = text_blocks(&doc, 0).unwrap().remove(0);
    // «Z» no está en el subconjunto: se compone con una fuente estándar y se avisa.
    let report = replace_text(&mut doc, 0, b.id, "Zeta", &Style::default()).unwrap();
    assert!(!report.warnings.iter().any(|w| w.contains("reutilizó")), "{report:?}");
}
