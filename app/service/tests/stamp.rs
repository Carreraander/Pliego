use mupdf::{Document, TextPageFlags};
use pliego_app_service::stamp::{StampOp, stamp_copy, stamp_file};
use pliego_core::JobContext;
use pliego_stamp::*;

/// PDF mínimo de `pages` páginas A4 con el texto "P<N>".
fn build_pdf(pages: usize) -> Vec<u8> {
    let mut objs: Vec<String> = vec!["<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into()];
    let pages_id = 2 + pages * 2;
    for i in 0..pages {
        let s = format!("BT /F1 24 Tf 250 400 Td (P{}) Tj ET", i + 1);
        objs.push(format!("<< /Length {} >>\nstream\n{s}\nendstream", s.len()));
        objs.push(format!(
            "<< /Type /Page /Parent {pages_id} 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 1 0 R >> >> /Contents {} 0 R >>",
            2 + 2 * i
        ));
    }
    let kids: Vec<String> = (0..pages).map(|i| format!("{} 0 R", 3 + 2 * i)).collect();
    objs.push(format!("<< /Type /Pages /Kids [{}] /Count {pages} >>", kids.join(" ")));
    objs.push(format!("<< /Type /Catalog /Pages {pages_id} 0 R >>"));
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offs = Vec::new();
    for (i, b) in objs.iter().enumerate() {
        offs.push(out.len());
        out.extend(format!("{} 0 obj\n{b}\nendobj\n", i + 1).bytes());
    }
    let x = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).bytes());
    for o in offs {
        out.extend(format!("{o:010} 00000 n \n").bytes());
    }
    out.extend(
        format!(
            "trailer\n<< /Size {} /Root {} 0 R >>\nstartxref\n{x}\n%%EOF\n",
            objs.len() + 1,
            objs.len()
        )
        .bytes(),
    );
    out
}

fn text_of(path: &std::path::Path, i: i32) -> String {
    let d = Document::open(path.to_str().unwrap()).unwrap();
    d.load_page(i)
        .unwrap()
        .to_text_page(TextPageFlags::empty())
        .unwrap()
        .to_text()
        .unwrap()
}

fn source(dir: &std::path::Path, pages: usize) -> std::path::PathBuf {
    let p = dir.join("origen.pdf");
    std::fs::write(&p, build_pdf(pages)).unwrap();
    p
}

#[test]
fn json_from_the_ui_round_trips() {
    let op: StampOp = serde_json::from_str(
        r#"{"op":"watermark","content":{"kind":"text","text":"BORRADOR","style":{}},"opacity":0.2,"rotation":30,"behind":false}"#,
    )
    .unwrap();
    assert!(matches!(op, StampOp::Watermark(_)));
    assert_eq!(op.suffix(), "marca de agua");
    let bates: StampOp = serde_json::from_str(r#"{"op":"bates","prefix":"X-"}"#).unwrap();
    assert!(matches!(bates, StampOp::Bates(ref b) if b.prefix == "X-" && b.digits == 6));
}

#[test]
fn stamps_only_the_chosen_pages_and_leaves_the_original() {
    let dir = tempfile::tempdir().unwrap();
    let src = source(dir.path(), 3);
    let before = std::fs::read(&src).unwrap();
    let out = dir.path().join("salida.pdf");
    let op = StampOp::Bates(BatesOptions {
        prefix: "DOC-".into(),
        ..Default::default()
    });
    let report = stamp_file(&src, &out, None, "2-3", &op, &JobContext::silent()).unwrap();
    assert_eq!(report.outputs, vec![out.clone()]);
    assert_eq!(std::fs::read(&src).unwrap(), before);
    assert!(!text_of(&out, 0).contains("DOC-"));
    assert!(text_of(&out, 1).contains("DOC-000001"));
    assert!(text_of(&out, 2).contains("DOC-000002"));
}

#[test]
fn header_footer_expands_variables() {
    let dir = tempfile::tempdir().unwrap();
    let src = source(dir.path(), 2);
    let out = dir.path().join("salida.pdf");
    let op = StampOp::HeaderFooter(HeaderFooter {
        footer: Slots {
            center: "{file} {page}/{pages}".into(),
            ..Default::default()
        },
        ..Default::default()
    });
    stamp_file(&src, &out, None, "all", &op, &JobContext::silent()).unwrap();
    assert!(text_of(&out, 1).contains("origen.pdf 2/2"));
}

#[test]
fn watermark_text_lands_on_the_page() {
    let dir = tempfile::tempdir().unwrap();
    let src = source(dir.path(), 1);
    let out = dir.path().join("salida.pdf");
    let op = StampOp::Watermark(Watermark {
        content: WatermarkContent::Text {
            text: "BORRADOR".into(),
            style: TextStyle::default(),
        },
        opacity: 0.3,
        rotation: 0.0,
        behind: false,
    });
    stamp_file(&src, &out, None, "all", &op, &JobContext::silent()).unwrap();
    assert!(text_of(&out, 0).contains("BORRADOR"));
}

#[test]
fn background_saves_a_valid_pdf() {
    let dir = tempfile::tempdir().unwrap();
    let src = source(dir.path(), 1);
    let out = dir.path().join("salida.pdf");
    let op = StampOp::Background(Background {
        fill: BackgroundFill::Color { rgb: [1.0, 0.9, 0.6] },
        opacity: 1.0,
    });
    stamp_file(&src, &out, None, "all", &op, &JobContext::silent()).unwrap();
    assert!(text_of(&out, 0).contains("P1"));
}

#[test]
fn bad_page_range_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let src = source(dir.path(), 2);
    let out = dir.path().join("salida.pdf");
    let op = StampOp::Bates(BatesOptions::default());
    assert!(stamp_file(&src, &out, None, "9", &op, &JobContext::silent()).is_err());
    assert!(stamp_file(&src, &out, None, "x-y", &op, &JobContext::silent()).is_err());
    assert!(!out.exists());
}

#[test]
fn every_json_shape_the_panels_send_deserializes() {
    let shapes = [
        r##"{"op":"watermark","content":{"kind":"image","path":"C:\\logo.png","scale":0.4},"opacity":0.3,"rotation":0,"behind":true}"##,
        r##"{"op":"header_footer","header":{"left":"","center":"X","right":""},"footer":{"left":"","center":"{page}","right":""},"style":{"font":"Courier","size":12,"color":[1,0,0]},"margin":40}"##,
        r##"{"op":"background","fill":{"kind":"color","rgb":[1,0.9,0.6]},"opacity":0.5}"##,
        r##"{"op":"background","fill":{"kind":"image","path":"a.png"},"opacity":1}"##,
        r##"{"op":"bates","prefix":"X-","suffix":"","digits":6,"start":1,"corner":"top_left","style":{"font":"Helvetica","size":10,"color":[0,0,0]}}"##,
    ];
    for s in shapes {
        serde_json::from_str::<StampOp>(s).unwrap_or_else(|e| panic!("{s}: {e}"));
    }
}

#[test]
fn copy_is_named_next_to_the_original_and_never_overwrites() {
    let dir = tempfile::tempdir().unwrap();
    let src = source(dir.path(), 1);
    let op = StampOp::Bates(BatesOptions::default());
    let a = stamp_copy(&src, "all", None, &op, &JobContext::silent()).unwrap();
    let b = stamp_copy(&src, "all", None, &op, &JobContext::silent()).unwrap();
    assert_eq!(a.outputs, vec![dir.path().join("origen (Bates).pdf")]);
    assert_eq!(b.outputs, vec![dir.path().join("origen (Bates 2).pdf")]);
}
