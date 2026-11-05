use pliego_annot::{StampPreset, StampSource};
use pliego_app_service::stamps::{Place, StampRequest, stamp_annot_copy};
use pliego_core::JobContext;
use pliego_pdf::PdfDocument;

fn two_pages(dir: &std::path::Path) -> std::path::PathBuf {
    let mut doc = PdfDocument::new();
    doc.new_page(pliego_pdf::Size::A4).unwrap();
    doc.new_page(pliego_pdf::Size::A4).unwrap();
    let p = dir.join("doc.pdf");
    doc.save(p.to_str().unwrap()).unwrap();
    p
}

fn annots(path: &std::path::Path, page: i32) -> usize {
    let d = PdfDocument::open(path.to_str().unwrap()).unwrap();
    d.load_pdf_page(page).unwrap().annotations().count()
}

#[test]
fn stamps_only_the_chosen_page_and_keeps_the_original() {
    let dir = tempfile::tempdir().unwrap();
    let src = two_pages(dir.path());
    let before = std::fs::read(&src).unwrap();
    let req = StampRequest {
        source: StampSource::Preset {
            preset: StampPreset::Aprobado,
        },
        place: Place::TopRight,
    };
    let r = stamp_annot_copy(&src, "2", None, &req, &JobContext::silent()).unwrap();
    let out = dir.path().join("doc (con sello).pdf");
    assert_eq!(r.outputs, vec![out.clone()]);
    assert_eq!(std::fs::read(&src).unwrap(), before);
    assert_eq!(annots(&out, 0), 0);
    assert_eq!(annots(&out, 1), 1);
}

#[test]
fn all_pages_and_empty_text_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let src = two_pages(dir.path());
    let req = StampRequest {
        source: StampSource::Text {
            text: "VISTO".into(),
            color: [0.0, 0.0, 1.0],
        },
        place: Place::Center,
    };
    stamp_annot_copy(&src, "all", None, &req, &JobContext::silent()).unwrap();
    let out = dir.path().join("doc (con sello).pdf");
    assert_eq!((annots(&out, 0), annots(&out, 1)), (1, 1));
    let empty = StampRequest {
        source: StampSource::Text {
            text: " ".into(),
            color: [0.0; 3],
        },
        place: Place::Center,
    };
    assert!(stamp_annot_copy(&src, "all", None, &empty, &JobContext::silent()).is_err());
    assert!(stamp_annot_copy(&src, "9", None, &req, &JobContext::silent()).is_err());
}

#[test]
fn json_from_the_panel_deserializes() {
    for s in [
        r#"{"source":{"kind":"preset","preset":"confidencial"},"place":"center"}"#,
        r#"{"source":{"kind":"text","text":"VISTO","color":[0,0,1]},"place":"bottom_left"}"#,
        r#"{"source":{"kind":"image","path":"C:\\firma.png"},"place":"top_right"}"#,
    ] {
        serde_json::from_str::<StampRequest>(s).unwrap_or_else(|e| panic!("{s}: {e}"));
    }
}
