use pliego_convert::{
    HtmlBackend, TextExportOptions, TextToPdfOptions, html_to_pdf, markdown_to_pdf, pdf_to_text, structured_text_pages,
    text_to_pdf,
};
use pliego_core::{JobContext, PageRange};

fn text_of(pdf: &std::path::Path) -> String {
    let out = pdf.with_extension("txt");
    pdf_to_text(
        pdf,
        None,
        &PageRange::all(),
        &out,
        &TextExportOptions::default(),
        &JobContext::silent(),
    )
    .unwrap();
    std::fs::read_to_string(out).unwrap()
}

#[test]
fn plain_text_roundtrip_with_accents_and_many_pages() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("a.txt");
    let mut body = String::from("Canción de España: ñandú, acción & <etiqueta>\n");
    for i in 0..300 {
        body.push_str(&format!(
            "Línea número {i} con texto suficiente para ocupar espacio en la página\n"
        ));
    }
    std::fs::write(&src, &body).unwrap();
    let pdf = dir.path().join("a.pdf");
    let r = text_to_pdf(&src, &pdf, &TextToPdfOptions::default(), &JobContext::silent()).unwrap();
    assert!(
        !r.summary.starts_with("1 "),
        "debería ocupar varias páginas: {}",
        r.summary
    );
    let t = text_of(&pdf);
    assert!(t.contains("Canción de España"), "{t:.200}");
    assert!(t.contains("<etiqueta>"));
    assert!(t.contains("Línea número 299"));
}

#[test]
fn markdown_has_headings_and_lists() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("a.md");
    std::fs::write(&src, "# Título grande\n\nUn párrafo con **negrita**.\n\n- uno\n- dos\n").unwrap();
    let pdf = dir.path().join("a.pdf");
    markdown_to_pdf(&src, &pdf, &TextToPdfOptions::default(), &JobContext::silent()).unwrap();
    let t = text_of(&pdf);
    assert!(t.contains("Título grande") && t.contains("negrita") && t.contains("dos"));
    // El encabezado es más grande que el párrafo.
    let page = &structured_text_pages(&pdf, None, &PageRange::all()).unwrap()[0];
    let sizes: Vec<f32> = page
        .blocks
        .iter()
        .filter_map(|b| match b {
            pliego_convert::Block::Text { lines, .. } => Some(lines[0].spans[0].size),
            _ => None,
        })
        .collect();
    assert!(sizes[0] > sizes[1], "{sizes:?}");
}

#[test]
fn html_and_backend_and_margins() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("a.html");
    std::fs::write(
        &src,
        "<html><body><h1>Hola mundo</h1><p>Texto &amp; más</p></body></html>",
    )
    .unwrap();
    let pdf = dir.path().join("a.pdf");
    html_to_pdf(&src, &pdf, &TextToPdfOptions::default(), &JobContext::silent()).unwrap();
    assert!(text_of(&pdf).contains("Hola mundo"));
    let page = &structured_text_pages(&pdf, None, &PageRange::all()).unwrap()[0];
    let x0 = page.blocks[0].bbox().x0;
    assert!(x0 >= 50.0, "el texto debe respetar el margen de 54 pt, x0={x0}");

    let o = TextToPdfOptions {
        backend: HtmlBackend::WebView2,
        ..Default::default()
    };
    let e = html_to_pdf(&src, &dir.path().join("b.pdf"), &o, &JobContext::silent()).unwrap_err();
    assert!(e.to_string().contains("WebView2"));
}

#[test]
fn non_utf8_is_a_clear_error() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("l1.txt");
    std::fs::write(&src, [0x48, 0xF3, 0x6C, 0x61]).unwrap();
    let e = text_to_pdf(
        &src,
        &dir.path().join("o.pdf"),
        &TextToPdfOptions::default(),
        &JobContext::silent(),
    )
    .unwrap_err();
    assert!(e.to_string().contains("UTF-8"));
}
