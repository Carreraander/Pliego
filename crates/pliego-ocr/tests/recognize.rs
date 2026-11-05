use std::path::PathBuf;

use pliego_ocr::{GrayImage, OcrEngine, TesseractEngine, available_languages, ocr_page_image};
use pliego_pdf::{Colorspace, Document, Matrix};

fn render_hola(dpi: f32) -> GrayImage {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/basic/hola.pdf");
    let doc = Document::open(&path).unwrap();
    let page = doc.load_page(0).unwrap();
    let s = dpi / 72.0;
    let pix = page
        .to_pixmap(&Matrix::new_scale(s, s), Colorspace::Gray, false, false)
        .unwrap();
    GrayImage::from_interleaved(pix.width(), pix.height(), pix.n() as usize, pix.samples()).unwrap()
}

#[test]
fn recognizes_known_text() {
    if !available_languages().iter().any(|l| l == "eng") {
        eprintln!("sin modelo eng: prueba omitida");
        return;
    }
    let words = ocr_page_image(&render_hola(300.0), "eng").unwrap();
    let text: Vec<&str> = words.iter().map(|w| w.text.as_str()).collect();
    assert!(text.contains(&"Hola") && text.contains(&"Pliego"), "palabras: {text:?}");
    let w = words.iter().find(|w| w.text == "Pliego").unwrap();
    assert!(w.bbox.x1 > w.bbox.x0 && w.bbox.y1 > w.bbox.y0 && w.confidence > 50.0);
}

#[test]
fn missing_language_is_clear_error() {
    let err = TesseractEngine::new("zzz", None).err().unwrap().to_string();
    assert!(err.contains("falta el modelo"), "{err}");
}

#[test]
fn engine_reports_version() {
    assert!(TesseractEngine::version().starts_with('5'));
}

#[allow(dead_code)]
fn _trait_object_is_send(e: Box<dyn OcrEngine>) -> impl Send {
    e
}
