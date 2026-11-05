use pliego_document::Document;
use pliego_edit::*;
use pliego_pdf::{Colorspace, Matrix};

const CORPUS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/corpus/edit/parrafos.pdf");

fn pixel(doc: &Document, x: usize, y: usize) -> [u8; 3] {
    let page = doc.pdf().load_page(0).unwrap();
    let pm = page
        .to_pixmap(&Matrix::new_scale(1.0, 1.0), Colorspace::Rgb, false, true)
        .unwrap();
    let i = (y * pm.width() as usize + x) * 3;
    pm.samples()[i..i + 3].try_into().unwrap()
}

const BLANCO: [u8; 3] = [255, 255, 255];

fn es_rojo(p: [u8; 3]) -> bool {
    p[0] > 200 && p[1] < 80 && p[2] < 80
}

#[test]
fn lista_mueve_gira_y_borra() {
    let mut doc = Document::open(CORPUS, None).unwrap();
    let v = vectors(&doc, 0).unwrap();
    assert_eq!(v.len(), 1);
    let b = v[0].bbox;
    assert!(
        (b[0] - 72.0).abs() < 0.6 && (b[1] - 302.0).abs() < 0.6 && (b[2] - 272.0).abs() < 0.6,
        "{b:?}"
    );
    assert!(es_rojo(pixel(&doc, 100, 320)));

    transform_vector(
        &mut doc,
        0,
        0,
        &Transform {
            dx: 150.0,
            dy: 100.0,
            ..Transform::default()
        },
    )
    .unwrap();
    let b = vectors(&doc, 0).unwrap()[0].bbox;
    assert!((b[0] - 222.0).abs() < 0.6 && (b[1] - 402.0).abs() < 0.6, "{b:?}");
    assert_eq!(pixel(&doc, 100, 320), BLANCO);
    assert!(es_rojo(pixel(&doc, 300, 420)));

    transform_vector(
        &mut doc,
        0,
        0,
        &Transform {
            rotate_degrees: 90.0,
            ..Transform::default()
        },
    )
    .unwrap();
    let b = vectors(&doc, 0).unwrap()[0].bbox;
    assert!(
        (b[2] - b[0] - 40.0).abs() < 0.6 && (b[3] - b[1] - 200.0).abs() < 0.6,
        "{b:?}"
    );

    delete_vector(&mut doc, 0, 0).unwrap();
    assert!(vectors(&doc, 0).unwrap().is_empty());
    assert_eq!(pixel(&doc, 300, 420), BLANCO);
    assert!(delete_vector(&mut doc, 0, 0).is_err());
    // El texto sigue ahí.
    assert_eq!(text_blocks(&doc, 0).unwrap().len(), 2);
}
