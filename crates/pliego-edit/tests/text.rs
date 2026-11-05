use pliego_document::{Document, SaveOptions};
use pliego_edit::*;
use pliego_pdf::{Colorspace, Matrix};

const CORPUS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/corpus/edit/parrafos.pdf");

fn open() -> Document {
    Document::open(CORPUS, None).unwrap()
}

/// Renderiza la página 0 a RGB a escala 1.
fn render(doc: &Document) -> (usize, usize, Vec<u8>) {
    let page = doc.pdf().load_page(0).unwrap();
    let pm = page
        .to_pixmap(&Matrix::new_scale(1.0, 1.0), Colorspace::Rgb, false, true)
        .unwrap();
    (pm.width() as usize, pm.height() as usize, pm.samples().to_vec())
}

/// Número de píxeles distintos fuera de `rect` (con un margen de 2 px).
fn diff_outside(a: &(usize, usize, Vec<u8>), b: &(usize, usize, Vec<u8>), rect: RectPts) -> usize {
    assert_eq!((a.0, a.1), (b.0, b.1));
    let mut n = 0;
    for y in 0..a.1 {
        for x in 0..a.0 {
            let inside = (x as f32) >= rect[0] - 2.0
                && (x as f32) <= rect[2] + 2.0
                && (y as f32) >= rect[1] - 2.0
                && (y as f32) <= rect[3] + 2.0;
            if inside {
                continue;
            }
            let i = (y * a.0 + x) * 3;
            if a.2[i..i + 3] != b.2[i..i + 3] {
                n += 1;
            }
        }
    }
    n
}

fn page_text(doc: &Document) -> String {
    text_blocks(doc, 0)
        .unwrap()
        .iter()
        .map(|b| b.text.clone())
        .collect::<Vec<_>>()
        .join("\n---\n")
}

#[test]
fn detecta_dos_bloques_con_su_fuente() {
    let doc = open();
    let blocks = text_blocks(&doc, 0).unwrap();
    assert_eq!(blocks.len(), 2, "{blocks:#?}");
    assert_eq!(blocks[0].lines.len(), 3);
    assert!(blocks[0].text.starts_with("Primer parrafo"));
    assert_eq!(blocks[0].family, FontFamily::Sans);
    assert_eq!(blocks[1].family, FontFamily::Serif);
    assert!((blocks[0].font_size - 12.0).abs() < 0.1);
}

#[test]
fn reemplazar_deja_el_resto_identico() {
    let mut doc = open();
    let before = render(&doc);
    let blocks = text_blocks(&doc, 0).unwrap();
    let b = &blocks[0];
    let report = replace_text(
        &mut doc,
        0,
        b.id,
        "Texto nuevo ñandú con acentos: áéíóú.",
        &Style::default(),
    )
    .unwrap();
    assert!(report.warnings.iter().any(|w| w.contains("fuente")));
    let after = render(&doc);
    assert_eq!(diff_outside(&before, &after, b.bbox), 0, "cambió algo fuera del bloque");
    let text = page_text(&doc).replace('\n', " ");
    assert!(text.contains("Texto nuevo ñandú con acentos: áéíóú."), "{text}");
    assert!(!text.contains("Primer parrafo"), "{text}");
    assert!(text.contains("Este es otro bloque"), "{text}");
    assert!(doc.is_modified());
    assert!(doc.undo().unwrap());
    assert!(page_text(&doc).contains("Primer parrafo"));
}

#[test]
fn reflujo_largo_y_aviso_de_desborde() {
    let mut doc = open();
    let b = text_blocks(&doc, 0).unwrap().remove(0);
    let largo = "palabra ".repeat(80);
    let report = replace_text(&mut doc, 0, b.id, &largo, &Style::default()).unwrap();
    assert!(report.warnings.iter().any(|w| w.contains("más líneas")), "{report:?}");
    let blocks = text_blocks(&doc, 0).unwrap();
    let nuevo = blocks.iter().find(|x| x.text.starts_with("palabra")).unwrap();
    assert!(nuevo.lines.len() > 3);
    // Ninguna línea sobrepasa el borde derecho original.
    assert!(nuevo.bbox[2] <= b.bbox[2] + 1.0);
}

#[test]
fn caracteres_que_ninguna_fuente_tiene_fallan_con_mensaje() {
    let mut doc = open();
    let b = text_blocks(&doc, 0).unwrap().remove(0);
    let err = replace_text(&mut doc, 0, b.id, "\u{10FFFD}", &Style::default()).unwrap_err();
    assert!(err.to_string().contains("ninguna fuente"), "{err}");
    assert!(!doc.is_modified());
}

#[test]
fn bloque_inexistente_es_error_y_no_modifica() {
    let mut doc = open();
    assert!(replace_text(&mut doc, 0, 99, "x", &Style::default()).is_err());
    assert!(replace_text(&mut doc, 5, 0, "x", &Style::default()).is_err());
    assert!(!doc.is_modified());
}

#[test]
fn insertar_texto_guarda_y_reabre() {
    let mut doc = open();
    let style = Style {
        size: Some(20.0),
        color: Some([1.0, 0.0, 0.0]),
        bold: Some(true),
        align: Align::Center,
        ..Style::default()
    };
    insert_text(&mut doc, 0, [72.0, 300.0, 400.0, 360.0], "Texto añadido", &style).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("salida.pdf");
    doc.save(&out, &SaveOptions::default()).unwrap();
    let reopened = Document::open(&out, None).unwrap();
    let blocks = text_blocks(&reopened, 0).unwrap();
    let nuevo = blocks
        .iter()
        .find(|b| b.text.contains("Texto añadido"))
        .expect("bloque nuevo");
    assert!(nuevo.bold);
    assert!((nuevo.font_size - 20.0).abs() < 0.1);
    assert!(nuevo.color[0] > 0.9 && nuevo.color[1] < 0.1);
    assert!(nuevo.bbox[0] > 72.0, "centrado");
}

#[test]
fn enlaces_externo_e_interno() {
    let mut doc = open();
    add_link(
        &mut doc,
        0,
        [72.0, 100.0, 200.0, 120.0],
        &LinkTarget::Uri {
            uri: "https://example.org".into(),
        },
    )
    .unwrap();
    add_link(&mut doc, 0, [72.0, 130.0, 200.0, 150.0], &LinkTarget::Page { page: 0 }).unwrap();
    assert!(add_link(&mut doc, 0, [0.0, 0.0, 1.0, 1.0], &LinkTarget::Page { page: 9 }).is_err());
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("l.pdf");
    doc.save(&out, &SaveOptions::default()).unwrap();
    let re = Document::open(&out, None).unwrap();
    let page = re.pdf().load_page(0).unwrap();
    let links: Vec<_> = page.links().unwrap().collect();
    assert_eq!(links.len(), 2, "{links:?}");
    assert!(links.iter().any(|l| l.uri == "https://example.org"));
}

#[test]
fn saltos_de_linea_no_fuerzan_otra_fuente_y_el_aviso_es_claro() {
    let mut doc = open();
    let b = text_blocks(&doc, 0).unwrap().remove(0);
    let report = replace_text(&mut doc, 0, b.id, "Primera línea\nsegunda línea", &Style::default()).unwrap();
    assert!(
        report.warnings.iter().all(|w| !w.contains("no tienen")),
        "usó fuente de reserva: {report:?}"
    );
    assert!(
        report.warnings.iter().all(|w| !w.contains("en lugar de «Helvetica»")),
        "{report:?}"
    );
}
