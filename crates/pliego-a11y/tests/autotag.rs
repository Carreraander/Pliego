use mupdf::{Colorspace, Matrix};
use pliego_a11y::{AutotagOptions, IssueCode, autotag_file, check_file};
use pliego_core::{Error, JobContext};

fn build_untagged() -> Vec<u8> {
    pdf_from_content(CONTENT)
}

const CONTENT: &str = "\
q 0.9 g 40 40 515 3 re f Q\n\
BT /F2 26 Tf 72 780 Td (Informe anual) Tj ET\n\
BT /F1 12 Tf 72 740 Td (Primera linea del parrafo de introduccion.) Tj ET\n\
BT /F1 12 Tf 72 726 Td (Segunda linea del mismo parrafo.) Tj ET\n\
BT /F2 18 Tf 72 690 Td (Resultados) Tj ET\n\
BT /F1 12 Tf 72 660 Td (\\225) Tj ET\n\
BT /F1 12 Tf 90 660 Td (Primer punto de la lista) Tj ET\n\
BT /F1 12 Tf 72 640 Td (\\225) Tj ET\n\
BT /F1 12 Tf 90 640 Td (Segundo punto de la lista) Tj ET\n\
BT /F1 12 Tf 72 600 Td (1. Un elemento numerado) Tj ET\n\
q 100 0 0 50 72 520 cm /Im1 Do Q\n\
BT /F1 12 Tf 72 480 Td (Texto final despues de la imagen.) Tj ET\n";

fn pdf_from_content(content: &str) -> Vec<u8> {
    let objs: Vec<String> = vec![
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold >>".into(),
        "<< /Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 8 /Length 1 >>\nstream\nA\nendstream".into(),
        format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()),
        "<< /Type /Page /Parent 6 0 R /MediaBox [0 0 595 842] /Contents 4 0 R /Resources << /Font << /F1 1 0 R /F2 2 0 R >> /XObject << /Im1 3 0 R >> >> >>".into(),
        "<< /Type /Pages /Kids [5 0 R] /Count 1 >>".into(),
        "<< /Type /Catalog /Pages 6 0 R >>".into(),
    ];
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offs = vec![];
    for (i, o) in objs.iter().enumerate() {
        offs.push(out.len());
        out.extend(format!("{} 0 obj\n{o}\nendobj\n", i + 1).bytes());
    }
    let x = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).bytes());
    for o in offs {
        out.extend(format!("{o:010} 00000 n \n").bytes());
    }
    out.extend(
        format!(
            "trailer\n<< /Size {} /Root 7 0 R >>\nstartxref\n{x}\n%%EOF\n",
            objs.len() + 1
        )
        .bytes(),
    );
    out
}

fn ink(p: &std::path::Path) -> usize {
    let doc = mupdf::Document::open(p.to_str().unwrap()).unwrap();
    let page = doc.load_page(0).unwrap();
    let pix = page
        .to_pixmap(&Matrix::IDENTITY, &Colorspace::device_gray(), false, false)
        .unwrap();
    pix.samples().iter().filter(|&&b| b < 200).count()
}

#[test]
fn tags_headings_lists_paragraphs_and_figures() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("sin.pdf");
    std::fs::write(&input, build_untagged()).unwrap();
    assert!(check_file(&input, None).unwrap().has(IssueCode::NoStructureTree));

    let out = dir.path().join("con.pdf");
    let opts = AutotagOptions {
        language: Some("es-ES".into()),
        title: Some("Informe".into()),
    };
    let rep = autotag_file(&input, &out, &opts, &JobContext::silent()).unwrap();
    assert!(rep.summary.contains("2 encabezado(s)"), "{}", rep.summary);
    assert!(rep.summary.contains("1 lista(s) con 3 elemento(s)"), "{}", rep.summary);
    assert!(rep.summary.contains("1 figura(s)"), "{}", rep.summary);

    let r = check_file(&out, None).unwrap();
    assert!(r.tagged, "{:#?}", r.issues);
    for gone in [
        IssueCode::NoStructureTree,
        IssueCode::NotTagged,
        IssueCode::NoLanguage,
        IssueCode::NoTitle,
        IssueCode::NoTabOrder,
    ] {
        assert!(!r.has(gone), "{gone:?}: {:#?}", r.issues);
    }
    // jerarquía correcta: H1 «Informe anual» (26) y H2 «Resultados» (18), sin saltos
    assert!(
        !r.has(IssueCode::HeadingSkipped) && !r.has(IssueCode::FirstHeadingNotH1),
        "{:#?}",
        r.issues
    );
    // la figura queda pendiente de texto alternativo
    assert!(r.has(IssueCode::FigureWithoutAlt));

    // el texto y el aspecto se conservan
    let (a, b) = (ink(&input), ink(&out));
    assert_eq!(a, b, "el dibujo de la página ha cambiado");
    let doc = mupdf::Document::open(out.to_str().unwrap()).unwrap();
    let text = doc
        .load_page(0)
        .unwrap()
        .to_text_page(mupdf::TextPageFlags::empty())
        .unwrap()
        .to_text()
        .unwrap();
    assert!(
        text.contains("Informe anual") && text.contains("Segundo punto"),
        "{text}"
    );
}

#[test]
fn alt_text_completes_the_flow() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("sin.pdf");
    std::fs::write(&input, build_untagged()).unwrap();
    let out = dir.path().join("con.pdf");
    autotag_file(&input, &out, &AutotagOptions::default(), &JobContext::silent()).unwrap();
    let r = check_file(&out, None).unwrap();
    let fig = r.issues.iter().find(|i| i.code == IssueCode::FigureWithoutAlt).unwrap();
    let fixed = dir.path().join("fin.pdf");
    let opts = pliego_a11y::FixOptions {
        language: Some("es".into()),
        title: Some("T".into()),
        alt_texts: vec![(fig.xref.unwrap(), "Logotipo".into())],
        tab_order: true,
    };
    pliego_a11y::fix_file(&out, &fixed, &opts, None).unwrap();
    let r = check_file(&fixed, None).unwrap();
    assert!(r.passes(), "{:#?}", r.issues);
}

#[test]
fn refuses_already_tagged_documents_and_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    let good = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/a11y/bueno.pdf");
    let out = dir.path().join("x.pdf");
    let err = autotag_file(&good, &out, &AutotagOptions::default(), &JobContext::silent()).unwrap_err();
    assert!(matches!(err, Error::InvalidInput(_)), "{err}");
    let input = dir.path().join("sin.pdf");
    std::fs::write(&input, build_untagged()).unwrap();
    assert!(autotag_file(&input, &input, &AutotagOptions::default(), &JobContext::silent()).is_err());
}

fn tag_summary(content: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("t.pdf");
    std::fs::write(&input, pdf_from_content(content)).unwrap();
    let out = dir.path().join("t2.pdf");
    let rep = autotag_file(&input, &out, &AutotagOptions::default(), &JobContext::silent()).unwrap();
    let r = check_file(&out, None).unwrap();
    assert!(!r.has(IssueCode::TableWithoutHeader), "{:#?}", r.issues);
    assert_eq!(ink(&input), ink(&out));
    rep.summary
}

#[test]
fn simple_table_is_detected_with_header_row() {
    let content = "\
BT /F2 20 Tf 72 780 Td (Ventas) Tj ET\n\
0 g 70 700 300 0.5 re f\n\
BT /F2 12 Tf 72 720 Td (Region) Tj ET BT /F2 12 Tf 200 720 Td (Total) Tj ET BT /F2 12 Tf 300 720 Td (Mes) Tj ET\n\
0 g 70 680 300 0.5 re f\n\
BT /F1 12 Tf 72 700 Td (Norte) Tj ET BT /F1 12 Tf 200 700 Td (120) Tj ET BT /F1 12 Tf 300 700 Td (Enero) Tj ET\n\
BT /F1 12 Tf 72 680 Td (Sur) Tj ET BT /F1 12 Tf 200 680 Td (95) Tj ET BT /F1 12 Tf 300 680 Td (Enero) Tj ET\n\
BT /F1 12 Tf 72 600 Td (Texto despues de la tabla.) Tj ET\n";
    assert!(tag_summary(content).contains("1 tabla(s)"));
}

#[test]
fn text_that_is_not_a_table_is_not_tagged_as_one() {
    // una sola fila de dos celdas
    let one = "BT /F1 12 Tf 72 700 Td (Izquierda) Tj ET BT /F1 12 Tf 300 700 Td (Derecha) Tj ET\n";
    assert!(tag_summary(one).contains("0 tabla(s)"));
    // dos columnas de texto corrido (celdas largas)
    let long = "a".repeat(80);
    let cols = format!(
        "BT /F1 12 Tf 72 700 Td ({long}) Tj ET BT /F1 12 Tf 300 700 Td ({long}) Tj ET\n\
         BT /F1 12 Tf 72 686 Td ({long}) Tj ET BT /F1 12 Tf 300 686 Td ({long}) Tj ET\n"
    );
    assert!(tag_summary(&cols).contains("0 tabla(s)"));
    // columnas que no se alinean
    let skew = "BT /F1 12 Tf 72 700 Td (A) Tj ET BT /F1 12 Tf 200 700 Td (B) Tj ET\n\
                BT /F1 12 Tf 72 686 Td (C) Tj ET BT /F1 12 Tf 260 686 Td (D) Tj ET\n";
    assert!(tag_summary(skew).contains("0 tabla(s)"));
}
