mod common;

use common::*;
use pliego_core::JobContext;
use pliego_ocr::{OcrMode, OcrOptions, ocr_document};

fn opts() -> OcrOptions {
    OcrOptions {
        langs: "spa+eng".into(),
        threads: 2,
        ..OcrOptions::default()
    }
}

fn truth() -> String {
    normalize(&SPANISH.join(" "))
}

#[test]
fn clean_scan_becomes_searchable() {
    if !has_models(&["spa", "eng"]) {
        eprintln!("faltan modelos: prueba omitida");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("scan.pdf"), dir.path().join("ocr.pdf"));
    make_scan(
        &input,
        &ScanSpec {
            lines: SPANISH,
            skew: 0.0,
            noise: 0.0,
            quarter: 0,
        },
    );
    assert_eq!(extract_text(&input), "");

    let report = ocr_document(&input, &output, &opts(), &JobContext::silent()).unwrap();
    assert!(report.summary.contains("1 página"), "{}", report.summary);
    let text = extract_text(&output);
    let c = cer(&truth(), &text);
    assert!(c < 0.03, "CER {c:.3}\n{text}");

    // La búsqueda encuentra palabras y devuelve cajas dentro de la página.
    let doc = pliego_pdf::Document::open(&output).unwrap();
    let page = doc.load_page(0).unwrap();
    let hits = page
        .to_text_page(pliego_pdf::TextFlags::empty())
        .unwrap()
        .search("Pliego")
        .unwrap();
    assert_eq!(hits.len(), 1);
    let b = page.bounds().unwrap();
    let q = &hits[0];
    assert!(q.ul.x >= 0.0 && q.lr.x <= b.x1 && q.ul.y >= 0.0 && q.lr.y <= b.y1);
    // «Pliego» está en la segunda línea (~y = 62 + 16 pt desde arriba).
    assert!((q.ul.y - 62.0).abs() < 25.0, "y = {}", q.ul.y);
}

#[test]
fn skewed_noisy_scan_with_deskew() {
    if !has_models(&["spa", "eng"]) {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("scan.pdf"), dir.path().join("ocr.pdf"));
    make_scan(
        &input,
        &ScanSpec {
            lines: SPANISH,
            skew: 2.5,
            noise: 0.01,
            quarter: 0,
        },
    );
    let o = OcrOptions {
        deskew: true,
        denoise: true,
        ..opts()
    };
    ocr_document(&input, &output, &o, &JobContext::silent()).unwrap();
    let c = cer(&truth(), &extract_text(&output));
    assert!(c < 0.03, "CER {c:.3}");
}

#[test]
fn rotated_page_is_fixed_with_osd() {
    if !has_models(&["spa", "eng", "osd"]) {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    for quarter in [90u32, 180, 270] {
        let (input, output) = (
            dir.path().join("scan.pdf"),
            dir.path().join(format!("ocr{quarter}.pdf")),
        );
        make_scan(
            &input,
            &ScanSpec {
                lines: SPANISH,
                skew: 0.0,
                noise: 0.0,
                quarter,
            },
        );
        let o = OcrOptions {
            rotate_pages: true,
            ..opts()
        };
        ocr_document(&input, &output, &o, &JobContext::silent()).unwrap();
        let c = cer(&truth(), &extract_text(&output));
        assert!(c < 0.05, "giro {quarter}: CER {c:.3}");
        let doc = pliego_pdf::PdfDocument::open(&output).unwrap();
        let rot = doc
            .load_pdf_page(0)
            .unwrap()
            .object()
            .get_dict("Rotate")
            .unwrap()
            .unwrap()
            .as_int()
            .unwrap();
        assert_eq!(rot as u32, (360 - quarter) % 360, "giro {quarter}");
    }
}

#[test]
fn pages_with_text_are_skipped_unless_forced() {
    if !has_models(&["eng"]) {
        return;
    }
    let input = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/basic/hola.pdf");
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("out.pdf");
    let o = OcrOptions {
        langs: "eng".into(),
        ..OcrOptions::default()
    };
    let r = ocr_document(&input, &output, &o, &JobContext::silent()).unwrap();
    assert!(r.summary.contains("0 página"), "{}", r.summary);
    assert_eq!(extract_text(&output), extract_text(&input));

    let forced = OcrOptions {
        mode: OcrMode::Force,
        ..o
    };
    let r = ocr_document(&input, &output, &forced, &JobContext::silent()).unwrap();
    assert!(r.summary.contains("2 página"), "{}", r.summary);
}

#[test]
fn never_overwrites_input_and_checks_args() {
    let input = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/basic/hola.pdf");
    let err = ocr_document(&input, &input, &OcrOptions::default(), &JobContext::silent()).unwrap_err();
    assert!(err.to_string().contains("no puede ser el de entrada"), "{err}");

    let dir = tempfile::tempdir().unwrap();
    let o = OcrOptions {
        pages: "9".into(),
        tessdata_dir: Some(dir.path().into()),
        ..OcrOptions::default()
    };
    let err = ocr_document(&input, &dir.path().join("o.pdf"), &o, &JobContext::silent()).unwrap_err();
    assert!(err.to_string().contains("no existe"), "{err}");
}
