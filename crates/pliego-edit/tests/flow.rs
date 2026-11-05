//! Recolocación del texto al editar (W43): lo de debajo sube o baja, lo que no cabe pasa a la página siguiente.

use pliego_document::Document;
use pliego_edit::*;

/// PDF con las páginas dadas (flujo de contenido de cada una), todas con Helvetica como `/F1`.
fn build(pages: &[String]) -> Vec<u8> {
    let n = pages.len();
    let mut objs: Vec<Vec<u8>> = vec![b"<< /Type /Catalog /Pages 2 0 R >>".to_vec()];
    let kids: Vec<String> = (0..n).map(|i| format!("{} 0 R", 4 + 2 * i)).collect();
    objs.push(format!("<< /Type /Pages /Kids [{}] /Count {n} >>", kids.join(" ")).into_bytes());
    objs.push(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec());
    for (i, content) in pages.iter().enumerate() {
        objs.push(
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 3 0 R >> >> /Contents {} 0 R >>",
                5 + 2 * i
            )
            .into_bytes(),
        );
        let mut v = format!("<< /Length {} >>\nstream\n", content.len()).into_bytes();
        v.extend_from_slice(content.as_bytes());
        v.extend_from_slice(b"\nendstream");
        objs.push(v);
    }
    let mut out = b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend_from_slice(o);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
    for o in offsets {
        out.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objs.len() + 1
        )
        .as_bytes(),
    );
    out
}

/// Párrafo de varias líneas (cada una con su `Td`), la primera con la línea base en `y` (usuario).
fn para(x: f32, y: f32, lines: &[&str]) -> String {
    let mut s = format!("BT /F1 12 Tf 14 TL {x} {y} Td\n");
    for (i, l) in lines.iter().enumerate() {
        if i > 0 {
            s.push_str("T*\n");
        }
        s.push_str(&format!("({l}) Tj\n"));
    }
    s.push_str("ET\n");
    s
}

fn page1() -> String {
    // Alto 842: P1 empieza en y=700 (página: 142), P2 a 14 pt de hueco, P3 casi al final, y un pie.
    let mut s = String::new();
    s += &para(
        72.0,
        700.0,
        &[
            "Primer parrafo linea uno",
            "Primer parrafo linea dos",
            "Primer parrafo linea tres",
        ],
    );
    s += &para(72.0, 640.0, &["Segundo parrafo linea uno", "Segundo parrafo linea dos"]);
    s += &para(72.0, 590.0, &["Tercer parrafo unica linea"]);
    s += &para(72.0, 40.0, &["Pie de pagina"]);
    s
}

fn open(pages: &[String]) -> Document {
    Document::open_bytes(&build(pages), None).unwrap()
}

fn block_with(doc: &Document, page: usize, needle: &str) -> TextBlock {
    text_blocks(doc, page)
        .unwrap()
        .into_iter()
        .find(|b| b.text.contains(needle))
        .unwrap_or_else(|| {
            panic!(
                "no hay «{needle}» en la página {page}: {:#?}",
                text_blocks(doc, page).unwrap()
            )
        })
}

const LONG: &str = "Texto nuevo mucho mas largo que el original para que ocupe varias lineas al componerlo dentro del \
    mismo ancho del parrafo y de esa manera empuje hacia abajo todo lo que hay debajo de el en la pagina";

#[test]
fn growing_a_paragraph_pushes_the_rest_down_but_not_the_footer() {
    let mut doc = open(&[page1()]);
    let (p2, p3, pie) = (
        block_with(&doc, 0, "Segundo"),
        block_with(&doc, 0, "Tercer"),
        block_with(&doc, 0, "Pie"),
    );
    let b0 = block_with(&doc, 0, "Primer");
    let r = replace_text_flow(&mut doc, 0, b0.id, LONG, &Style::default(), &FlowOptions::default()).unwrap();
    let new_lines = block_with(&doc, 0, "Texto nuevo").lines.len();
    assert!(new_lines > 3, "el texto nuevo debe ocupar más líneas: {new_lines}");
    let delta = 14.0 * (new_lines as f32 - 3.0);
    let (q2, q3, qpie) = (
        block_with(&doc, 0, "Segundo"),
        block_with(&doc, 0, "Tercer"),
        block_with(&doc, 0, "Pie"),
    );
    assert!(
        (q2.bbox[1] - p2.bbox[1] - delta).abs() < 0.6,
        "{} vs {} + {delta}",
        q2.bbox[1],
        p2.bbox[1]
    );
    assert!((q3.bbox[1] - p3.bbox[1] - delta).abs() < 0.6);
    assert!((qpie.bbox[1] - pie.bbox[1]).abs() < 0.1, "el pie no se mueve");
    // Los párrafos de abajo conservan su texto y su fuente.
    assert_eq!(q2.text, p2.text);
    assert_eq!(q2.font_name, p2.font_name);
    assert!(!r.warnings.iter().any(|w| w.contains("invadir")), "{:?}", r.warnings);
}

#[test]
fn shrinking_pulls_the_rest_up() {
    let mut doc = open(&[page1()]);
    let (p2, p3) = (block_with(&doc, 0, "Segundo"), block_with(&doc, 0, "Tercer"));
    let b0 = block_with(&doc, 0, "Primer");
    replace_text_flow(
        &mut doc,
        0,
        b0.id,
        "Una sola linea",
        &Style::default(),
        &FlowOptions::default(),
    )
    .unwrap();
    let (q2, q3) = (block_with(&doc, 0, "Segundo"), block_with(&doc, 0, "Tercer"));
    assert!(
        (p2.bbox[1] - q2.bbox[1] - 28.0).abs() < 0.6,
        "{:?} {:?}",
        p2.bbox,
        q2.bbox
    );
    assert!((p3.bbox[1] - q3.bbox[1] - 28.0).abs() < 0.6);
}

#[test]
fn without_flow_nothing_below_moves() {
    let mut doc = open(&[page1()]);
    let p2 = block_with(&doc, 0, "Segundo");
    let b0 = block_with(&doc, 0, "Primer");
    let r = replace_text_flow(&mut doc, 0, b0.id, LONG, &Style::default(), &FlowOptions::NONE).unwrap();
    let q2 = block_with(&doc, 0, "Segundo");
    assert!((q2.bbox[1] - p2.bbox[1]).abs() < 0.1);
    assert!(r.warnings.iter().any(|w| w.contains("invadir")), "{:?}", r.warnings);
}

/// Una página llena hasta el límite: lo que baja de más pasa a la siguiente.
fn full_page() -> String {
    let mut s = String::new();
    s += &para(
        72.0,
        700.0,
        &[
            "Primer parrafo linea uno",
            "Primer parrafo linea dos",
            "Primer parrafo linea tres",
        ],
    );
    // Doce párrafos de dos líneas hasta cerca del margen inferior (y de usuario ~ 90).
    for i in 0..12 {
        let y = 640.0 - 50.0 * i as f32;
        s += &para(
            72.0,
            y,
            &[&format!("Relleno {i} linea uno"), &format!("Relleno {i} linea dos")],
        );
    }
    s += &para(72.0, 40.0, &["Pie de pagina"]);
    s
}

#[test]
fn overflow_moves_the_tail_to_the_next_page() {
    let page2 = para(72.0, 700.0, &["Pagina dos primer parrafo", "Pagina dos segunda linea"]);
    let mut doc = open(&[full_page(), page2]);
    let ultimo = block_with(&doc, 0, "Relleno 11");
    let p2_before = block_with(&doc, 1, "Pagina dos");
    let b0 = block_with(&doc, 0, "Primer");
    let r = replace_text_flow(&mut doc, 0, b0.id, LONG, &Style::default(), &FlowOptions::default()).unwrap();
    // Lo que ya no cabe salió de la página 1 y está al principio de la 2.
    let p0: String = text_blocks(&doc, 0)
        .unwrap()
        .iter()
        .map(|b| b.text.clone())
        .collect::<Vec<_>>()
        .join("|");
    let p1: Vec<TextBlock> = text_blocks(&doc, 1).unwrap();
    assert!(!p0.contains("Relleno 11"), "{p0}");
    assert!(p0.contains("Pie de pagina"));
    let moved = p1
        .iter()
        .find(|b| b.text.contains("Relleno 11"))
        .expect("el último pasó a la página 2");
    let own = block_with(&doc, 1, "Pagina dos");
    assert!(moved.bbox[1] < own.bbox[1], "lo que llegó va arriba de lo que había");
    assert!(own.bbox[1] > p2_before.bbox[1], "lo que había en la página 2 bajó");
    assert!(
        (moved.bbox[2] - moved.bbox[0] - (ultimo.bbox[2] - ultimo.bbox[0])).abs() < 0.6,
        "misma anchura"
    );
    assert!(r.warnings.iter().any(|w| w.contains("página 2")), "{:?}", r.warnings);
    assert_eq!(doc.pdf().page_count().unwrap(), 2);
    dump(&doc, "desborde");
}

/// `PLIEGO_DUMP_DIR=/ruta`: guarda cada página como PNG para mirarla a ojo.
fn dump(doc: &Document, name: &str) {
    let Ok(dir) = std::env::var("PLIEGO_DUMP_DIR") else {
        return;
    };
    for i in 0..doc.pdf().page_count().unwrap() {
        let pm = doc
            .pdf()
            .load_page(i)
            .unwrap()
            .to_pixmap(
                &pliego_pdf::Matrix::new_scale(1.0, 1.0),
                pliego_pdf::Colorspace::Rgb,
                false,
                true,
            )
            .unwrap();
        std::fs::write(format!("{dir}/{name}-{i}.png"), pm.encode_png().unwrap()).unwrap();
    }
}

#[test]
fn overflow_on_the_last_page_creates_a_new_one() {
    let mut doc = open(&[full_page()]);
    let b0 = block_with(&doc, 0, "Primer");
    let r = replace_text_flow(&mut doc, 0, b0.id, LONG, &Style::default(), &FlowOptions::default()).unwrap();
    assert_eq!(doc.pdf().page_count().unwrap(), 2, "{:?}", r.warnings);
    let p1 = text_blocks(&doc, 1).unwrap();
    assert!(p1.iter().any(|b| b.text.contains("Relleno 11")), "{p1:#?}");
    assert!(
        r.warnings.iter().any(|w| w.contains("se creó la página 2")),
        "{:?}",
        r.warnings
    );
    // La página nueva tiene el mismo tamaño.
    let (a, b) = (
        doc.pdf().load_pdf_page(0).unwrap().bounds().unwrap(),
        doc.pdf().load_pdf_page(1).unwrap().bounds().unwrap(),
    );
    assert!((a.width() - b.width()).abs() < 0.1 && (a.height() - b.height()).abs() < 0.1);
}

#[test]
fn undo_restores_the_whole_flow() {
    let mut doc = open(&[full_page()]);
    let before = text_blocks(&doc, 0).unwrap();
    let b0 = block_with(&doc, 0, "Primer");
    replace_text_flow(&mut doc, 0, b0.id, LONG, &Style::default(), &FlowOptions::default()).unwrap();
    assert!(doc.undo().unwrap());
    assert_eq!(doc.pdf().page_count().unwrap(), 1);
    let after = text_blocks(&doc, 0).unwrap();
    assert_eq!(before.len(), after.len());
    for (a, b) in before.iter().zip(&after) {
        assert_eq!(a.text, b.text);
        assert!((a.bbox[1] - b.bbox[1]).abs() < 0.1);
    }
}
