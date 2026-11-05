//! Maquetador propio de texto y Markdown (W40, bloque 8): contenido, geometría, saltos de página, enlaces y validez.
//! Los PDF se leen con MuPDF (motor independiente del que los escribe) y se validan con qpdf si está instalado.

use std::path::{Path, PathBuf};
use std::process::Command;

use pliego_convert::{
    Block, PageSizeSpec, TextExportOptions, TextToPdfOptions, markdown_to_pdf, pdf_to_text, structured_text_pages,
    text_to_pdf,
};
use pliego_core::{JobContext, PageRange, Report};
use pliego_pdf::Document;

fn md_to_pdf(dir: &Path, md: &str) -> (PathBuf, Report) {
    md_to_pdf_with(dir, md, &TextToPdfOptions::default())
}

fn md_to_pdf_with(dir: &Path, md: &str, o: &TextToPdfOptions) -> (PathBuf, Report) {
    let src = dir.join("a.md");
    std::fs::write(&src, md).unwrap();
    let pdf = dir.join("a.pdf");
    let r = markdown_to_pdf(&src, &pdf, o, &JobContext::silent()).unwrap();
    qpdf_check(&pdf);
    (pdf, r)
}

fn qpdf_check(pdf: &Path) {
    if Command::new("qpdf").arg("--version").output().is_ok() {
        let out = Command::new("qpdf").arg("--check").arg(pdf).output().unwrap();
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(out.status.success() && !text.contains("WARNING"), "{text}");
    }
}

fn text_of(pdf: &Path) -> String {
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

fn page_texts(pdf: &Path) -> Vec<String> {
    text_of(pdf).split('\u{c}').map(str::to_owned).collect()
}

#[test]
fn markdown_covers_every_construct() {
    let dir = tempfile::tempdir().unwrap();
    let md = "# Título uno\n\n## Subtítulo\n\nPárrafo con **negrita**, *cursiva*, ~~tachado~~ y `código`.\n\n\
- uno\n- dos\n  - dos.uno\n\n1. primero\n2. segundo\n\n- [x] hecho\n- [ ] pendiente\n\n> cita\n\n\
```\nfn main() {}\n```\n\n---\n\n| A | B |\n|---|---|\n| celda1 | celda2 |\n\nFin.\n";
    let (pdf, r) = md_to_pdf(dir.path(), md);
    assert!(r.warnings.is_empty(), "{:?}", r.warnings);
    let t = text_of(&pdf);
    for needle in [
        "Título uno",
        "Subtítulo",
        "negrita",
        "cursiva",
        "tachado",
        "código",
        "uno",
        "dos.uno",
        "1.",
        "2.",
        "primero",
        "[x]",
        "[ ]",
        "pendiente",
        "cita",
        "fn main() {}",
        "celda1",
        "celda2",
        "Fin.",
    ] {
        assert!(t.contains(needle), "falta {needle:?} en:\n{t}");
    }
    // Orden de lectura: cada pieza aparece después de la anterior.
    let pos = |s: &str| t.find(s).unwrap();
    assert!(
        pos("Título uno") < pos("Subtítulo") && pos("Subtítulo") < pos("negrita") && pos("negrita") < pos("celda1")
    );
    assert!(pos("celda1") < pos("Fin."));
}

#[test]
fn text_never_leaves_the_margins() {
    let dir = tempfile::tempdir().unwrap();
    let long_word = "x".repeat(300);
    let md = format!(
        "# Un título largo largo largo largo largo largo largo largo largo largo largo largo largo largo largo\n\n{long_word}\n\n\
| col | otra |\n|---|---|\n| {long_word} | y |\n\n> {long_word}\n\n- {long_word}\n"
    );
    let (pdf, _) = md_to_pdf(dir.path(), &md);
    let pages = structured_text_pages(&pdf, None, &PageRange::all()).unwrap();
    assert!(!pages.is_empty());
    for (n, p) in pages.iter().enumerate() {
        for b in &p.blocks {
            if let Block::Text { bbox, lines } = b {
                assert!(
                    bbox.x0 >= 53.0 && bbox.x1 <= p.width - 53.0,
                    "página {n}: {bbox:?} {:?}",
                    lines
                        .iter()
                        .map(|l| (l.text().chars().take(20).collect::<String>(), l.bbox.x1))
                        .collect::<Vec<_>>()
                );
                // Los cuadros de MuPDF incluyen el ascendente de la fuente, que sobresale unos puntos de la línea.
                assert!(bbox.y0 >= 48.0 && bbox.y1 <= p.height - 48.0, "página {n}: {bbox:?}");
            }
        }
    }
}

#[test]
fn a_heading_is_never_left_alone_at_the_bottom() {
    let dir = tempfile::tempdir().unwrap();
    // Se prueban muchas alturas de relleno: sea cual sea, el título y su texto van en la misma página.
    for k in 25..40 {
        let mut md = String::new();
        for i in 0..k {
            md.push_str(&format!("Relleno {i}\n\n"));
        }
        md.push_str("## Encabezado final\n\nTexto del encabezado.\n");
        let (pdf, _) = md_to_pdf(dir.path(), &md);
        for p in page_texts(&pdf) {
            if p.contains("Encabezado final") {
                assert!(p.contains("Texto del encabezado."), "k={k}: título huérfano en:\n{p}");
            }
        }
    }
}

#[test]
fn long_documents_paginate_and_keep_every_line() {
    let dir = tempfile::tempdir().unwrap();
    let mut md = String::new();
    for i in 0..400 {
        md.push_str(&format!(
            "Párrafo número {i} con palabras suficientes para ocupar un poco de sitio.\n\n"
        ));
    }
    let (pdf, r) = md_to_pdf(dir.path(), &md);
    assert!(!r.summary.starts_with("1 "), "{}", r.summary);
    let t = text_of(&pdf);
    for i in [0, 199, 399] {
        assert!(t.contains(&format!("Párrafo número {i} ")), "falta el {i}");
    }
    assert!(Document::open(&pdf).unwrap().page_count().unwrap() > 5);
}

#[test]
fn links_become_annotations_with_ascii_uris() {
    let dir = tempfile::tempdir().unwrap();
    let (pdf, _) = md_to_pdf(
        dir.path(),
        "Visita [Pliego](https://pdfpliego.com/ñandú) o [esto](https://example.com/a).\n",
    );
    let doc = Document::open(&pdf).unwrap();
    let links = doc.load_page(0).unwrap().links().unwrap();
    let uris: Vec<String> = links.into_iter().map(|l| l.uri).collect();
    assert!(
        uris.contains(&"https://pdfpliego.com/%C3%B1and%C3%BA".to_string()),
        "{uris:?}"
    );
    assert!(uris.contains(&"https://example.com/a".to_string()), "{uris:?}");
}

#[test]
fn unsupported_characters_are_reported_and_typography_is_transcribed() {
    let dir = tempfile::tempdir().unwrap();
    let (pdf, r) = md_to_pdf(dir.path(), "Flecha → y menos − y euro € y 日本 y “comillas” — raya.\n");
    assert_eq!(r.warnings.len(), 1, "{:?}", r.warnings);
    assert!(r.warnings[0].starts_with("2 carácter"), "{:?}", r.warnings);
    let t = text_of(&pdf);
    assert!(
        t.contains("->") && t.contains("- y euro") && t.contains('€') && t.contains("“comillas”") && t.contains('—')
    );
    assert!(t.contains("??"));
}

#[test]
fn empty_and_odd_inputs_still_give_valid_pdfs() {
    let dir = tempfile::tempdir().unwrap();
    for md in [
        "",
        "   \n\n  \n",
        "\n\n\n",
        "<div>solo html</div>",
        "![alt](x.png)",
        "- \n- \n",
        "|a|\n|-|\n",
        "\0\u{7f}\u{1}texto",
    ] {
        let (pdf, _) = md_to_pdf(dir.path(), md);
        assert!(Document::open(&pdf).unwrap().page_count().unwrap() >= 1, "{md:?}");
    }
    let t = text_of(&md_to_pdf(dir.path(), "<p>Hola &amp; adiós</p>\n\nuna<br>otra").0);
    assert!(
        t.contains("Hola & adiós") && t.contains("una") && t.contains("otra"),
        "{t}"
    );
}

#[test]
fn page_size_font_size_and_margins_are_respected() {
    let dir = tempfile::tempdir().unwrap();
    let o = TextToPdfOptions {
        page_size: PageSizeSpec::Letter,
        margin_pt: 100.0,
        font_size_pt: 20.0,
        ..Default::default()
    };
    let (pdf, _) = md_to_pdf_with(dir.path(), "Hola mundo\n", &o);
    let page = &structured_text_pages(&pdf, None, &PageRange::all()).unwrap()[0];
    assert_eq!((page.width.round(), page.height.round()), (612.0, 792.0));
    let Block::Text { lines, bbox } = &page.blocks[0] else {
        panic!()
    };
    assert!(bbox.x0 >= 99.0 && bbox.y0 >= 94.0, "{bbox:?}");
    assert!((lines[0].spans[0].size - 20.0).abs() < 0.01);
}

#[test]
fn plain_text_keeps_spaces_blank_lines_and_wraps() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("a.txt");
    let long = "palabra ".repeat(60);
    std::fs::write(&src, format!("primera\n\n\n    sangrada\r\n{long}\nfin\ttab\n")).unwrap();
    let pdf = dir.path().join("a.pdf");
    let r = text_to_pdf(&src, &pdf, &TextToPdfOptions::default(), &JobContext::silent()).unwrap();
    qpdf_check(&pdf);
    assert!(r.warnings.is_empty());
    let t = text_of(&pdf);
    assert!(t.contains("primera") && t.contains("sangrada") && t.contains("fin"));
    let page = &structured_text_pages(&pdf, None, &PageRange::all()).unwrap()[0];
    for b in &page.blocks {
        if let Block::Text { bbox, .. } = b {
            assert!(bbox.x1 <= page.width - 53.0, "se sale por la derecha: {bbox:?}");
        }
    }
    // La sangría se conserva (los espacios iniciales forman parte de la línea).
    let indented = page
        .blocks
        .iter()
        .filter_map(|b| match b {
            Block::Text { lines, .. } => Some(lines),
            _ => None,
        })
        .flatten()
        .find(|l| l.text().contains("sangrada"))
        .unwrap();
    // PDFium reduce los espacios iniciales a uno en el texto, pero la caja conserva su anchura: «    sangrada» son 12
    // caracteres de Courier 11 (6,6 pt cada uno).
    let w = indented.bbox.x1 - indented.bbox.x0;
    assert!(
        indented.spans[0].text.starts_with(' ') && w > 12.0 * 6.6 - 1.0,
        "{indented:?}"
    );
}

// --- HTML básico (W41a) --------------------------------------------------------------------------------------

fn html_to_pdf_file(dir: &Path, html: &str) -> (PathBuf, Report) {
    html_to_pdf_with(dir, html, &TextToPdfOptions::default())
}

fn html_to_pdf_with(dir: &Path, html: &str, o: &TextToPdfOptions) -> (PathBuf, Report) {
    let src = dir.join("a.html");
    std::fs::write(&src, html).unwrap();
    let pdf = dir.join("h.pdf");
    let r = pliego_convert::html_to_pdf(&src, &pdf, o, &JobContext::silent()).unwrap();
    qpdf_check(&pdf);
    (pdf, r)
}

#[test]
fn html_covers_every_construct_in_reading_order() {
    let dir = tempfile::tempdir().unwrap();
    let html = "<!DOCTYPE html><html><head><title>NO SALE</title><style>p{color:red}</style><script>var x='NI ESTO';</script></head>\
<body><h1>Título uno</h1><h2>Subtítulo</h2>\
<p>Párrafo con <b>negrita</b>, <i>cursiva</i>, <s>tachado</s> y <code>código</code> &amp; &eacute;ñe &#8364; &#x41;.</p>\
<ul><li>uno</li><li>dos<ul><li>dos.uno</li></ul></li></ul>\
<ol><li>primero</li><li>segundo</li></ol>\
<blockquote>cita</blockquote><pre>fn main() {\n    1 &lt; 2\n}</pre><hr>\
<table><tr><th>A</th><th>B</th></tr><tr><td>celda1</td><td>celda2</td></tr></table>\
<p>Fin <a href=\"https://example.com/x\">enlace</a>.</p></body></html>";
    let (pdf, r) = html_to_pdf_file(dir.path(), html);
    assert!(r.warnings.is_empty(), "{:?}", r.warnings);
    let t = text_of(&pdf);
    for needle in [
        "Título uno",
        "Subtítulo",
        "negrita",
        "cursiva",
        "tachado",
        "código",
        "& éñe € A.",
        "dos.uno",
        "primero",
        "segundo",
        "cita",
        "fn main() {",
        "1 < 2",
        "celda1",
        "celda2",
        "Fin",
        "enlace",
    ] {
        assert!(t.contains(needle), "falta {needle:?} en:\n{t}");
    }
    for ignored in ["NO SALE", "NI ESTO", "color:red"] {
        assert!(!t.contains(ignored), "no debería salir {ignored:?}:\n{t}");
    }
    let pos = |s: &str| t.find(s).unwrap();
    assert!(
        pos("Título uno") < pos("Subtítulo") && pos("Subtítulo") < pos("negrita") && pos("negrita") < pos("celda1")
    );
    assert!(pos("celda1") < pos("Fin"));
    // El enlace queda como anotación con su dirección.
    let bytes = std::fs::read(&pdf).unwrap();
    assert!(
        bytes
            .windows(b"https://example.com/x".len())
            .any(|w| w == b"https://example.com/x"),
        "falta la anotación del enlace"
    );
}

#[test]
fn html_collapses_whitespace_and_keeps_pre() {
    let dir = tempfile::tempdir().unwrap();
    let (pdf, _) = html_to_pdf_file(
        dir.path(),
        "<p>uno\n\n   dos \t tres</p><pre>\n  a   b\n\n  c</pre><p>x<br>y<br/>z</p>",
    );
    let t = text_of(&pdf);
    assert!(t.contains("uno dos tres"), "{t}");
    // Los espacios repetidos de `pre` se conservan en la composición; PDFium los extrae como uno solo.
    assert!(t.contains("a b") || t.contains("a   b"), "{t}");
    assert!(t.contains("x") && t.contains("y") && t.contains("z"));
}

#[test]
fn html_malformed_input_never_panics_and_gives_a_valid_pdf() {
    let dir = tempfile::tempdir().unwrap();
    let deep = format!("{}texto profundo{}", "<div>".repeat(2000), "</div>".repeat(2000));
    let many_cells = format!("<table><tr>{}</tr></table>", "<td>c</td>".repeat(500));
    for html in [
        "",
        "   ",
        "solo texto sin etiquetas",
        "<p>sin cerrar <b>negrita <i>cursiva",
        "</p></div></b>cierres huérfanos",
        "<p>a < b y c > d &noexiste; &#99999999; &#xZZ; &amp",
        "<a href=",
        "<!-- comentario sin cerrar",
        "<script>nunca se cierra",
        "<table><td>sin tr</td></table><tr><td>fuera</td></tr>",
        "<ul><li>a<li>b<li>c</ul>",
        "<img src=x alt='una imagen'><input value=no>",
        "<p>\0\u{1}\u{7f}texto</p>",
        &deep,
        &many_cells,
    ] {
        let (pdf, _) = html_to_pdf_file(dir.path(), html);
        assert!(Document::open(&pdf).unwrap().page_count().unwrap() >= 1, "{html:.60?}");
    }
    let t = text_of(&html_to_pdf_file(dir.path(), "<ul><li>a<li>b<li>c</ul>").0);
    assert!(t.contains('a') && t.contains('b') && t.contains('c'), "{t}");
    let t = text_of(&html_to_pdf_file(dir.path(), &deep).0);
    assert!(t.contains("texto profundo"), "{t}");
    let t = text_of(&html_to_pdf_file(dir.path(), "<img src=x alt='una imagen'>").0);
    assert!(t.contains("una imagen"), "{t}");
}

#[test]
fn html_respects_margins_and_paginates() {
    let dir = tempfile::tempdir().unwrap();
    let long_word = "x".repeat(300);
    let body: String = (0..300)
        .map(|i| format!("<p>Párrafo número {i} {long_word}</p>"))
        .collect();
    let (pdf, _) = html_to_pdf_file(dir.path(), &format!("<body>{body}</body>"));
    let pages = structured_text_pages(&pdf, None, &PageRange::all()).unwrap();
    assert!(pages.len() > 5);
    for (n, p) in pages.iter().enumerate() {
        for b in &p.blocks {
            if let Block::Text { bbox, .. } = b {
                assert!(
                    bbox.x0 >= 53.0 && bbox.x1 <= p.width - 53.0,
                    "página {}: {bbox:?}",
                    n + 1
                );
            }
        }
    }
}
