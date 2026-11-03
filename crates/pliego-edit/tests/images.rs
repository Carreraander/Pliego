use mupdf::{Colorspace, Matrix};
use pliego_document::{Document, SaveOptions};
use pliego_edit::*;

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/corpus/edit/");

fn open() -> Document {
    Document::open(format!("{DIR}imagenes.pdf"), None).unwrap()
}

fn png(name: &str) -> Vec<u8> {
    std::fs::read(format!("{DIR}{name}")).unwrap()
}

/// Color RGB del píxel (x, y) de la página 0 a escala 1.
fn pixel(doc: &Document, x: usize, y: usize) -> [u8; 3] {
    let page = doc.pdf().load_page(0).unwrap();
    let pm = page
        .to_pixmap(&Matrix::new_scale(1.0, 1.0), &Colorspace::device_rgb(), false, true)
        .unwrap();
    let i = (y * pm.width() as usize + x) * 3;
    pm.samples()[i..i + 3].try_into().unwrap()
}

const ROJO: [u8; 3] = [255, 0, 0];
const AZUL: [u8; 3] = [0, 0, 255];
const BLANCO: [u8; 3] = [255, 255, 255];

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.6
}

#[test]
fn lista_imagenes_con_matrices_anidadas() {
    let doc = open();
    let imgs = images(&doc, 0).unwrap();
    assert_eq!(imgs.len(), 2);
    let b = imgs[0].bbox;
    assert!(
        near(b[0], 72.0) && near(b[1], 162.0) && near(b[2], 172.0) && near(b[3], 242.0),
        "{b:?}"
    );
    assert_eq!(imgs[0].name, "Im1");
    assert!(imgs[0].xref > 0);
    let b = imgs[1].bbox;
    assert!(
        near(b[0], 200.0) && near(b[1], 282.0) && near(b[2], 260.0) && near(b[3], 342.0),
        "{b:?}"
    );
    assert_eq!(pixel(&doc, 100, 200), ROJO);
    assert_eq!(pixel(&doc, 230, 310), AZUL);
}

#[test]
fn mover_y_redimensionar() {
    let mut doc = open();
    transform_object(
        &mut doc,
        0,
        0,
        &Transform {
            dx: 100.0,
            dy: 50.0,
            scale_x: 0.5,
            scale_y: 0.5,
            ..Transform::default()
        },
    )
    .unwrap();
    let b = images(&doc, 0).unwrap()[0].bbox;
    // Centro original (122, 202) -> (222, 252); tamaño 50x40.
    assert!(
        near(b[0], 197.0) && near(b[2], 247.0) && near(b[1], 232.0) && near(b[3], 272.0),
        "{b:?}"
    );
    assert_eq!(pixel(&doc, 100, 200), BLANCO, "queda rastro en la posición vieja");
    assert_eq!(pixel(&doc, 222, 252), ROJO);
    assert_eq!(images(&doc, 0).unwrap().len(), 2);
}

#[test]
fn girar_90_intercambia_ancho_y_alto() {
    let mut doc = open();
    transform_object(
        &mut doc,
        0,
        0,
        &Transform {
            rotate_degrees: 90.0,
            ..Transform::default()
        },
    )
    .unwrap();
    let b = images(&doc, 0).unwrap()[0].bbox;
    assert!(near(b[2] - b[0], 80.0) && near(b[3] - b[1], 100.0), "{b:?}");
    assert!(
        near((b[0] + b[2]) / 2.0, 122.0) && near((b[1] + b[3]) / 2.0, 202.0),
        "{b:?}"
    );
}

#[test]
fn borrar_imagen() {
    let mut doc = open();
    delete_object(&mut doc, 0, 0).unwrap();
    let imgs = images(&doc, 0).unwrap();
    assert_eq!(imgs.len(), 1);
    assert_eq!(imgs[0].name, "Im2");
    assert_eq!(pixel(&doc, 100, 200), BLANCO);
    assert_eq!(pixel(&doc, 230, 310), AZUL, "la otra imagen no cambia");
    assert!(delete_object(&mut doc, 0, 5).is_err());
}

#[test]
fn recortar_oculta_sin_borrar() {
    let mut doc = open();
    crop_image(&mut doc, 0, 0, [72.0, 162.0, 122.0, 242.0]).unwrap();
    assert_eq!(pixel(&doc, 90, 200), ROJO);
    assert_eq!(pixel(&doc, 150, 200), BLANCO);
}

#[test]
fn reemplazar_conserva_posicion() {
    let mut doc = open();
    let report = replace_image(&mut doc, 0, 0, &png("azul.png")).unwrap();
    assert!(report.warnings.iter().any(|w| w.contains("proporción")), "{report:?}");
    assert_eq!(pixel(&doc, 100, 200), AZUL);
    assert_eq!(pixel(&doc, 230, 310), AZUL);
    let b = images(&doc, 0).unwrap()[0].bbox;
    assert!(near(b[0], 72.0) && near(b[2], 172.0));
    assert!(replace_image(&mut doc, 0, 0, b"no es una imagen").is_err());
}

#[test]
fn insertar_guarda_y_reabre() {
    let mut doc = open();
    insert_image(&mut doc, 0, [300.0, 400.0, 360.0, 460.0], &png("rojo.png")).unwrap();
    assert_eq!(images(&doc, 0).unwrap().len(), 3);
    assert_eq!(pixel(&doc, 330, 430), ROJO);
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("o.pdf");
    doc.save(&out, &SaveOptions::default()).unwrap();
    let re = Document::open(&out, None).unwrap();
    assert_eq!(images(&re, 0).unwrap().len(), 3);
    assert_eq!(pixel(&re, 330, 430), ROJO);
    assert!(doc.undo().unwrap());
    assert_eq!(images(&doc, 0).unwrap().len(), 2);
}
