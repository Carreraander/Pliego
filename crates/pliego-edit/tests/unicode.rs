use pliego_core::Error;
use pliego_document::{Document, SaveOptions};
use pliego_edit::*;

const CORPUS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/corpus/edit/parrafos.pdf");

fn open() -> Document {
    Document::open(CORPUS, None).unwrap()
}

fn texto(doc: &Document) -> String {
    text_blocks(doc, 0)
        .unwrap()
        .iter()
        .map(|b| b.lines.join(" "))
        .collect::<Vec<_>>()
        .join(" | ")
}

/// Si el equipo no tiene ninguna fuente con esos caracteres, la prueba no puede ejecutarse.
fn sin_fuente(e: &Error) -> bool {
    matches!(e, Error::Unsupported(m) if m.contains("ninguna fuente"))
}

fn roundtrip(nuevo: &str) {
    let mut doc = open();
    let b = text_blocks(&doc, 0).unwrap().remove(0);
    let report = match replace_text(&mut doc, 0, b.id, nuevo, &Style::default()) {
        Ok(r) => r,
        Err(e) if sin_fuente(&e) => {
            eprintln!("sin fuente instalada para {nuevo:?}: se omite");
            return;
        }
        Err(e) => panic!("{e}"),
    };
    assert!(
        report.warnings.iter().any(|w| w.contains("compuso con la fuente")),
        "{report:?}"
    );
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("u.pdf");
    doc.save(&out, &SaveOptions::default()).unwrap();
    // Solo se incrustan los glifos usados: el PDF no crece con el tamaño de la fuente.
    let size = std::fs::metadata(&out).unwrap().len();
    assert!(
        size < 100_000,
        "el PDF pesa {size} bytes: ¿se incrustó la fuente entera?"
    );
    let re = Document::open(&out, None).unwrap();
    let t = texto(&re).replace('\n', " ");
    let esperado: String = nuevo.chars().filter(|c| !c.is_whitespace()).collect();
    let obtenido: String = t.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(obtenido.contains(&esperado), "esperado {esperado:?} en {obtenido:?}");
    assert!(t.contains("Este es otro bloque"), "el otro bloque cambió: {t}");
}

#[test]
fn cirilico_y_griego() {
    roundtrip("Привет мир Ελληνικά");
}

#[test]
fn japones_con_reflujo() {
    roundtrip("これは長い日本語の文章で、ブロックの幅に合わせて折り返されるはずです。");
}

#[test]
fn fuente_indicada_que_no_existe_da_error_claro() {
    let mut doc = open();
    let style = Style {
        font_file: Some("no-existe.ttf".into()),
        ..Style::default()
    };
    let err = insert_text(&mut doc, 0, [10.0, 10.0, 200.0, 40.0], "hola", &style).unwrap_err();
    assert!(err.to_string().contains("no-existe.ttf"), "{err}");
    assert!(!doc.is_modified());
}

/// Un carácter que ninguna fuente tiene no bloquea la edición: se avisa y el resto del texto se compone.
#[test]
fn caracter_inexistente_en_toda_fuente_avisa_sin_bloquear() {
    let mut doc = open();
    // Plano privado: ninguna fuente razonable lo tiene.
    let report = insert_text(
        &mut doc,
        0,
        [10.0, 10.0, 200.0, 40.0],
        "hola \u{10FFFD}",
        &Style::default(),
    )
    .unwrap();
    assert!(
        report
            .warnings
            .iter()
            .any(|w| w.contains("ninguna fuente instalada tiene")),
        "{report:?}"
    );
    assert!(doc.is_modified());
}

/// Emojis y símbolos mezclados con texto normal: se completan con otra fuente en lugar de dar error.
#[test]
fn emojis_y_simbolos_se_completan_con_otra_fuente() {
    let nuevo = "Proxy explícito: ✅ listo y ❌ no";
    let mut doc = open();
    let b = text_blocks(&doc, 0).unwrap().remove(0);
    let report = replace_text(&mut doc, 0, b.id, nuevo, &Style::default()).unwrap();
    assert!(
        report.warnings.iter().any(|w| w.contains("compuso con la fuente")),
        "{report:?}"
    );
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("e.pdf");
    doc.save(&out, &SaveOptions::default()).unwrap();
    let re = Document::open(&out, None).unwrap();
    let t = texto(&re).replace('\n', " ");
    assert!(t.contains("Proxy explícito:"), "{t}");
    assert!(t.contains("listo y"), "{t}");
    let sin_glifo = report
        .warnings
        .iter()
        .any(|w| w.contains("ninguna fuente instalada tiene"));
    if !sin_glifo {
        assert!(t.contains('✅') && t.contains('❌'), "{t}");
    }
    assert!(t.contains("Este es otro bloque"), "el otro bloque cambió: {t}");
}

#[test]
fn varias_fuentes_en_una_misma_linea_al_insertar() {
    let mut doc = open();
    let report = insert_text(
        &mut doc,
        0,
        [10.0, 10.0, 300.0, 60.0],
        "Привет ✅ мир ❌ ok",
        &Style::default(),
    )
    .unwrap();
    assert!(doc.is_modified(), "{report:?}");
}
