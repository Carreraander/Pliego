mod common;

use std::path::PathBuf;

use common::{Opts, build, write};
use pliego_core::{Error, JobContext};
use pliego_pdfa::{Level, convert, validate};

fn corpus(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus")
        .join(rel)
}

fn convert_to(input: &std::path::Path, level: Level) -> (PathBuf, pliego_core::Report, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("salida.pdf");
    let report = convert(input, &out, level, &JobContext::silent()).unwrap();
    (out, report, dir)
}

#[test]
fn plain_document_becomes_pdfa_at_every_level() {
    for level in [Level::A1b, Level::A2b, Level::A2u, Level::A3b] {
        let (out, report, _d) = convert_to(&corpus("basic/hola.pdf"), level);
        let v = validate(&out, level).unwrap();
        assert!(v.conforms(), "{level:?}: {:?}", v.issues);
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
        assert!(report.summary.contains("fuente(s) incrustada(s)"), "{}", report.summary);
        // el texto sigue ahí
        let doc = mupdf::Document::open(out.to_str().unwrap()).unwrap();
        let page = doc.load_page(0).unwrap();
        let text = page
            .to_text_page(mupdf::TextPageFlags::empty())
            .unwrap()
            .to_text()
            .unwrap();
        assert!(text.contains("Hola Pliego"), "{text}");
    }
}

#[test]
fn fixes_every_defect_of_the_fixtures() {
    let dir = tempfile::tempdir().unwrap();
    let all = Opts {
        no_xmp: true,
        no_intent: true,
        javascript: true,
        transparency: true,
        hidden_annot: true,
        lzw: true,
        ocg: true,
        embedded_file: true,
        af: false,
        ..Default::default()
    };
    let input = write(dir.path(), "malo.pdf", &build(&all, 2, "B"));
    for level in [Level::A1b, Level::A2b, Level::A3b] {
        let (out, report, _d) = convert_to(&input, level);
        let v = validate(&out, level).unwrap();
        assert!(v.conforms(), "{level:?}: {:?} / {:?}", v.issues, report.warnings);
    }
    // Aviso al quitar capas en PDF/A-1.
    let (_, report, _d) = convert_to(&input, Level::A1b);
    assert!(report.warnings.iter().any(|w| w.contains("capas")));
}

#[test]
fn pdfa3_keeps_attachments_and_associates_them() {
    let dir = tempfile::tempdir().unwrap();
    let input = write(
        dir.path(),
        "adj.pdf",
        &build(
            &Opts {
                embedded_file: true,
                ..Default::default()
            },
            3,
            "B",
        ),
    );
    let (out, _r, _d) = convert_to(&input, Level::A3b);
    assert!(validate(&out, Level::A3b).unwrap().conforms());
    let doc = mupdf::pdf::PdfDocument::open(out.to_str().unwrap()).unwrap();
    let af = doc.catalog().unwrap().get_dict("AF").unwrap().expect("AF");
    assert_eq!(af.len().unwrap(), 1);
    // ISO 19005-3 6.8: el flujo incrustado declara su tipo MIME
    let spec = af.get_array(0).unwrap().unwrap();
    let ef = spec.get_dict("EF").unwrap().unwrap().get_dict("F").unwrap().unwrap();
    assert_eq!(
        ef.get_dict("Subtype").unwrap().unwrap().as_name().unwrap(),
        b"text/plain"
    );
    // y para 2b se quitan
    let (out2, r2, _d2) = convert_to(&input, Level::A2b);
    assert!(validate(&out2, Level::A2b).unwrap().conforms());
    assert!(r2.summary.contains("adjunto"));
}

#[test]
fn metadata_keeps_info_and_matches_xmp() {
    let dir = tempfile::tempdir().unwrap();
    let (out, _r, _d) = convert_to(&corpus("basic/hola.pdf"), Level::A2b);
    let _ = dir;
    let doc = mupdf::pdf::PdfDocument::open(out.to_str().unwrap()).unwrap();
    let cat = doc.catalog().unwrap();
    let meta = cat.get_dict("Metadata").unwrap().unwrap();
    // PDF/A-1 y 2 exigen que el flujo XMP no tenga filtros
    assert!(meta.get_dict("Filter").unwrap().is_none());
    let xmp = String::from_utf8(meta.read_stream().unwrap()).unwrap();
    assert!(
        xmp.contains("<pdfaid:part>2</pdfaid:part>") && xmp.contains("xmp:CreateDate"),
        "{xmp}"
    );
    let info = doc.trailer().unwrap().get_dict("Info").unwrap().unwrap();
    assert!(info.get_dict("Producer").unwrap().is_some() && info.get_dict("ModDate").unwrap().is_some());
}

#[test]
fn refuses_when_fonts_cannot_be_embedded() {
    // Fuente no estándar sin incrustar: se rechaza en lugar de declarar PDF/A en falso.
    let dir = tempfile::tempdir().unwrap();
    let mut bytes = build(
        &Opts {
            helvetica_not_embedded: true,
            ..Default::default()
        },
        2,
        "B",
    );
    let s = String::from_utf8_lossy(&bytes).replace("/BaseFont /Helvetica", "/BaseFont /Garamond");
    bytes = s.into_bytes();
    let input = write(dir.path(), "g.pdf", &bytes);
    let out = dir.path().join("o.pdf");
    let err = convert(&input, &out, Level::A2b, &JobContext::silent()).unwrap_err();
    assert!(
        matches!(err, Error::Unsupported(ref m) if m.contains("Garamond")),
        "{err}"
    );
    assert!(!out.exists());
}

#[test]
fn never_overwrites_input_and_respects_cancel() {
    let input = corpus("basic/hola.pdf");
    assert!(convert(&input, &input, Level::A2b, &JobContext::silent()).is_err());
    let ctx = JobContext::silent();
    ctx.cancel.cancel();
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("o.pdf");
    assert!(matches!(convert(&input, &out, Level::A2b, &ctx), Err(Error::Cancelled)));
    assert!(!out.exists());
}

#[test]
fn substituted_fonts_still_render_text() {
    use mupdf::{Colorspace, Matrix};
    let render = |p: &std::path::Path| -> usize {
        let doc = mupdf::Document::open(p.to_str().unwrap()).unwrap();
        let page = doc.load_page(0).unwrap();
        let pix = page
            .to_pixmap(&Matrix::IDENTITY, &Colorspace::device_gray(), false, false)
            .unwrap();
        pix.samples().iter().filter(|&&b| b < 128).count()
    };
    let original = corpus("basic/hola.pdf");
    let (out, _r, _d) = convert_to(&original, Level::A2b);
    let (a, b) = (render(&original), render(&out));
    assert!(b > 200, "el texto convertido no se dibuja ({b} píxeles oscuros)");
    // mismo tamaño de tinta, con tolerancia por el suavizado de las curvas
    assert!(
        (a as f64 - b as f64).abs() / (a as f64) < 0.25,
        "original {a}, convertido {b}"
    );
}

#[test]
fn adds_trailer_id_when_missing() {
    let dir = tempfile::tempdir().unwrap();
    let input = write(
        dir.path(),
        "sin_id.pdf",
        &build(
            &Opts {
                no_id: true,
                ..Default::default()
            },
            2,
            "B",
        ),
    );
    assert!(
        validate(&input, Level::A2b)
            .unwrap()
            .has(pliego_pdfa::Rule::NoTrailerId)
    );
    let (out, _r, _d) = convert_to(&input, Level::A2b);
    assert!(validate(&out, Level::A2b).unwrap().conforms());
    let doc = mupdf::pdf::PdfDocument::open(out.to_str().unwrap()).unwrap();
    let id = doc.trailer().unwrap().get_dict("ID").unwrap().expect("ID");
    assert_eq!(id.len().unwrap(), 2);
}

#[test]
fn page_additional_actions_entry_is_removed_for_pdfa2_and_3() {
    // ISO 19005-2/-3 6.5.2 (veraPDF 1.30.3 sobre protect/sucio.pdf)
    for level in [Level::A2b, Level::A2u, Level::A3b] {
        let (out, _r, _d) = convert_to(&corpus("protect/sucio.pdf"), level);
        let v = validate(&out, level).unwrap();
        assert!(v.conforms(), "{level:?}: {:?}", v.issues);
        let doc = mupdf::pdf::PdfDocument::open(out.to_str().unwrap()).unwrap();
        let page = doc.find_page(0).unwrap();
        assert!(page.get_dict("AA").unwrap().is_none(), "{level:?}");
    }
}
