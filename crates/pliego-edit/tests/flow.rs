//! Recolocación del texto al editar (W43): lo de debajo sube o baja, lo que no cabe pasa a la página siguiente.

use pliego_document::Document;
use pliego_edit::*;

/// PDF con las páginas dadas (flujo de contenido de cada una), todas con Helvetica como `/F1`.
fn build(pages: &[String]) -> Vec<u8> {
    let n = pages.len();
    let mut objs: Vec<Vec<u8>> = vec![b"<< /Type /Catalog /Pages 2 0 R >>".to_vec()];
    let kids: Vec<String> = (0..n).map(|i| format!("{} 0 R", 5 + 2 * i)).collect();
    objs.push(format!("<< /Type /Pages /Kids [{}] /Count {n} >>", kids.join(" ")).into_bytes());
    objs.push(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec());
    // Imagen de 8x4 píxeles (`/Im1`) para los logos de las pruebas.
    let mut img = b"<< /Type /XObject /Subtype /Image /Width 8 /Height 4 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Length 96 >>\nstream\n".to_vec();
    img.extend((0..96u8).map(|i| 40 + i));
    img.extend_from_slice(b"\nendstream");
    objs.push(img);
    for (i, content) in pages.iter().enumerate() {
        objs.push(
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 3 0 R >> /XObject << /Im1 4 0 R >> >> /Contents {} 0 R >>",
                6 + 2 * i
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

/// Cuerpo de texto de ancho de columna completo (líneas de unos 400 pt) con un título corto encima.
fn page_with_short_title() -> String {
    let mut s = para(72.0, 760.0, &["Titulo corto"]);
    s += &para(
        72.0,
        700.0,
        &[
            "Este parrafo del cuerpo ocupa casi todo el ancho de la columna de texto de la pagina",
            "y termina aqui en la segunda linea sin mas",
        ],
    );
    s += &para(72.0, 40.0, &["Pie de pagina"]);
    s
}

#[test]
fn a_short_line_is_composed_at_the_width_of_its_column() {
    let mut doc = open(&[page_with_short_title()]);
    let title = block_with(&doc, 0, "Titulo");
    let body = block_with(&doc, 0, "Este parrafo");
    assert!(title.bbox[2] < body.bbox[2] - 150.0, "el título es estrecho");
    replace_text_flow(&mut doc, 0, title.id, LONG, &Style::default(), &FlowOptions::default()).unwrap();
    let new = block_with(&doc, 0, "Texto nuevo");
    // Ya no queda una columna del ancho del título: llega al borde derecho del cuerpo.
    assert!(new.bbox[2] > title.bbox[2] + 200.0, "{:?} vs {:?}", new.bbox, body.bbox);
    assert!(new.bbox[2] <= body.bbox[2] + 0.6, "sin pasarse de la columna");
    assert!(new.lines.len() <= 3, "{} líneas", new.lines.len());
}

#[test]
fn a_paragraph_that_does_not_fit_continues_on_the_next_page() {
    // Un párrafo de una línea cerca del pie, con otro texto debajo, que se alarga mucho.
    let mut page = para(72.0, 150.0, &["Ultimo parrafo de la pagina"]);
    page += &para(72.0, 110.0, &["Texto de debajo"]);
    page += &para(72.0, 40.0, &["Pie de pagina"]);
    let page2 = para(72.0, 700.0, &["Pagina dos primer parrafo"]);
    let mut doc = open(&[page, page2]);
    let b = block_with(&doc, 0, "Ultimo");
    let words = format!("{LONG} {LONG} {LONG} {LONG} {LONG}");
    let r = replace_text_flow(&mut doc, 0, b.id, &words, &Style::default(), &FlowOptions::default()).unwrap();
    let pie = block_with(&doc, 0, "Pie");
    // En la página 1 nada invade el pie, y el texto no se pierde: sigue en la 2, por delante de lo que había.
    let p0 = text_blocks(&doc, 0).unwrap();
    for blk in p0.iter().filter(|x| !x.text.contains("Pie")) {
        assert!(blk.bbox[3] < pie.bbox[1], "{:?} invade el pie {:?}", blk.bbox, pie.bbox);
    }
    let p1 = text_blocks(&doc, 1).unwrap();
    let count = |blocks: &[TextBlock]| {
        blocks
            .iter()
            .filter(|x| x.text.contains("Texto nuevo") || x.text.contains("pagina"))
            .map(|x| {
                x.text
                    .split_whitespace()
                    .filter(|w| *w == "Texto" || *w == "nuevo")
                    .count()
            })
            .sum::<usize>()
    };
    assert_eq!(count(&p0) + count(&p1), 10, "no se pierde texto: {p0:#?} {p1:#?}");
    let tail = p1
        .iter()
        .find(|x| x.text.contains("nuevo"))
        .expect("el resto sigue en la página 2");
    let own = block_with(&doc, 1, "Pagina dos");
    assert!(
        tail.bbox[1] < own.bbox[1],
        "el resto va delante de lo que ya estaba en la página 2"
    );
    // Lo que estaba debajo del párrafo también pasó, detrás del resto del texto.
    let below = block_with(&doc, 1, "Texto de debajo");
    assert!(
        below.bbox[1] >= tail.bbox[3] - 1.0,
        "{:?} tras {:?}",
        below.bbox,
        tail.bbox
    );
    assert!(r.warnings.iter().any(|w| w.contains("página 2")), "{:?}", r.warnings);
    assert_eq!(doc.pdf().page_count().unwrap(), 2);
    dump(&doc, "continua");
}

fn uri(u: &str) -> LinkTarget {
    LinkTarget::Uri { uri: u.into() }
}

#[test]
fn links_follow_the_text_they_cover_when_it_moves() {
    let mut doc = open(&[page1()]);
    let (p2, p3, pie) = (
        block_with(&doc, 0, "Segundo"),
        block_with(&doc, 0, "Tercer"),
        block_with(&doc, 0, "Pie"),
    );
    add_link(&mut doc, 0, p2.bbox, &uri("https://uno.example")).unwrap();
    add_link(&mut doc, 0, p3.bbox, &uri("https://dos.example")).unwrap();
    add_link(&mut doc, 0, pie.bbox, &uri("https://pie.example")).unwrap();
    let b0 = block_with(&doc, 0, "Primer");
    replace_text_flow(&mut doc, 0, b0.id, LONG, &Style::default(), &FlowOptions::default()).unwrap();
    let (q2, q3, qpie) = (
        block_with(&doc, 0, "Segundo"),
        block_with(&doc, 0, "Tercer"),
        block_with(&doc, 0, "Pie"),
    );
    let l = links(&doc, 0).unwrap();
    let find = |u: &str| {
        l.iter()
            .find(|x| x.target == Some(uri(u)))
            .unwrap_or_else(|| panic!("falta {u}: {l:#?}"))
    };
    // Cada enlace sigue cubriendo su texto; el del pie no se mueve.
    for (u, b) in [
        ("https://uno.example", &q2),
        ("https://dos.example", &q3),
        ("https://pie.example", &qpie),
    ] {
        let r = find(u).rect;
        assert!(
            (r[1] - b.bbox[1]).abs() < 0.6 && (r[3] - b.bbox[3]).abs() < 0.6,
            "{u}: enlace {r:?} frente al texto {:?}",
            b.bbox
        );
    }
    assert!((find("https://pie.example").rect[1] - pie.bbox[1]).abs() < 0.1);
}

#[test]
fn links_follow_the_text_to_the_next_page() {
    let page2 = para(72.0, 700.0, &["Pagina dos primer parrafo"]);
    let mut doc = open(&[full_page(), page2]);
    let ultimo = block_with(&doc, 0, "Relleno 11");
    add_link(&mut doc, 0, ultimo.bbox, &uri("https://ultimo.example")).unwrap();
    let b0 = block_with(&doc, 0, "Primer");
    replace_text_flow(&mut doc, 0, b0.id, LONG, &Style::default(), &FlowOptions::default()).unwrap();
    assert!(
        links(&doc, 0).unwrap().is_empty(),
        "el enlace salió de la página 1: {:?}",
        links(&doc, 0).unwrap()
    );
    let moved = block_with(&doc, 1, "Relleno 11");
    let l = links(&doc, 1).unwrap();
    assert_eq!(l.len(), 1, "{l:#?}");
    assert_eq!(l[0].target, Some(uri("https://ultimo.example")));
    assert!(
        (l[0].rect[1] - moved.bbox[1]).abs() < 0.6 && (l[0].rect[3] - moved.bbox[3]).abs() < 0.6,
        "{:?} frente a {:?}",
        l[0].rect,
        moved.bbox
    );
    assert!((l[0].rect[0] - moved.bbox[0]).abs() < 0.6 && (l[0].rect[2] - moved.bbox[2]).abs() < 0.6);
}

/// Página con cuatro párrafos, un logo centrado de 200×80 (fuera de la columna del primer párrafo por x si éste
/// es corto) y una línea centrada con un enlace debajo, más el pie.
fn page_with_logo() -> String {
    let mut s = para(
        72.0,
        760.0,
        &["Texto de abertura linea uno", "Texto de abertura linea dos"],
    );
    s += &para(72.0, 700.0, &["Segundo bloque linea uno", "Segundo bloque linea dos"]);
    s += &para(72.0, 640.0, &["Tercer bloque linea uno", "Tercer bloque linea dos"]);
    s += &para(72.0, 580.0, &["Cuarto bloque unica linea"]);
    s += "q 200 0 0 80 197.5 430 cm /Im1 Do Q\n";
    s += &para(200.0, 400.0, &["Mas informacion en pdfpliego.com"]);
    s += &para(72.0, 40.0, &["Pie de pagina"]);
    s
}

/// Caja de cada elemento (bloques de texto e imágenes) de la página, con un nombre para los mensajes.
fn elements(doc: &Document, page: usize) -> Vec<(String, [f32; 4])> {
    let mut v: Vec<(String, [f32; 4])> = text_blocks(doc, page)
        .unwrap()
        .into_iter()
        .map(|b| (b.text.chars().take(18).collect(), b.bbox))
        .collect();
    v.extend(
        images(doc, page)
            .unwrap()
            .into_iter()
            .map(|i| ("[logo]".to_string(), i.bbox)),
    );
    v
}

fn assert_no_overlaps(doc: &Document, page: usize) {
    let e = elements(doc, page);
    for (i, (na, a)) in e.iter().enumerate() {
        for (nb, b) in &e[i + 1..] {
            let (w, h) = (a[2].min(b[2]) - a[0].max(b[0]), a[3].min(b[3]) - a[1].max(b[1]));
            assert!(
                w <= 1.0 || h <= 1.0,
                "«{na}» {a:?} se solapa con «{nb}» {b:?} en la página {page}: {e:#?}"
            );
        }
    }
}

fn find<'a>(e: &'a [(String, [f32; 4])], needle: &str) -> &'a [f32; 4] {
    &e.iter()
        .find(|(n, _)| n.contains(needle))
        .unwrap_or_else(|| panic!("falta «{needle}»: {e:#?}"))
        .1
}

#[test]
fn long_text_moves_everything_below_including_a_centred_logo_and_its_line() {
    let mut doc = open(&[page_with_logo()]);
    let before = elements(&doc, 0);
    let (logo0, line0) = (*find(&before, "[logo]"), *find(&before, "Mas informacion"));
    let b0 = block_with(&doc, 0, "Texto de abertura");
    replace_text_flow(&mut doc, 0, b0.id, LONG, &Style::default(), &FlowOptions::default()).unwrap();
    let after = elements(&doc, 0);
    let delta = find(&after, "Segundo")[1] - find(&before, "Segundo")[1];
    assert!(delta > 10.0, "el texto largo debe empujar lo de debajo: {delta}");
    for (name, was, now) in [
        ("logo", logo0, *find(&after, "[logo]")),
        ("línea", line0, *find(&after, "Mas informacion")),
    ] {
        assert!(
            (now[1] - was[1] - delta).abs() < 0.8,
            "{name}: {was:?} → {now:?}, debía bajar {delta}"
        );
    }
    assert_no_overlaps(&doc, 0);
}

/// Filas (y superior e inferior, en puntos) del logo: franjas anchas de gris liso, que el texto no produce.
/// Con `None`, no hay logo en la página. El logo dentro de un formulario XObject no se lista como imagen.
fn logo_rows(doc: &Document, page: usize) -> Option<(f32, f32)> {
    let pm = doc
        .pdf()
        .load_page(page as i32)
        .unwrap()
        .to_pixmap(
            &pliego_pdf::Matrix::new_scale(1.0, 1.0),
            pliego_pdf::Colorspace::Rgb,
            false,
            false,
        )
        .unwrap();
    let n = usize::from(pm.n());
    let (w, h) = (pm.width() as usize, pm.height() as usize);
    let rows: Vec<usize> = (0..h)
        .filter(|&y| {
            let px = |x: usize| pm.samples()[y * pm.stride() + x * n];
            // Una tira de 150 píxeles seguidos oscuros y sin saltos bruscos (el texto no la produce).
            let mut run = 0;
            (1..w).any(|x| {
                let (a, b) = (px(x - 1), px(x));
                run = if a < 150 && b < 150 && a.abs_diff(b) <= 12 {
                    run + 1
                } else {
                    0
                };
                run >= 150
            })
        })
        .collect();
    Some((*rows.first()? as f32, (*rows.last()? + 1) as f32))
}

#[test]
fn huge_text_keeps_the_logo_and_its_line_together_after_the_text_without_overlaps() {
    let mut doc = open(&[page_with_logo()]);
    let before = elements(&doc, 0);
    let logo0 = logo_rows(&doc, 0).expect("el logo se ve en la original");
    let line0 = *find(&before, "Mas informacion");
    let gap0 = line0[1] - logo0.1;
    let b0 = block_with(&doc, 0, "Texto de abertura");
    let words = std::iter::repeat_n(LONG, 12).collect::<Vec<_>>().join(" ");
    let r = replace_text_flow(&mut doc, 0, b0.id, &words, &Style::default(), &FlowOptions::default()).unwrap();
    dump(&doc, "logo-enorme");
    let pages = doc.pdf().page_count().unwrap() as usize;
    assert_eq!(pages, 3, "{:?}", r.warnings);
    for p in 0..pages {
        assert_no_overlaps(&doc, p);
    }
    // Nada de lo que seguía al párrafo se queda en las dos primeras páginas encima del texto largo.
    assert!(logo_rows(&doc, 0).is_none(), "el logo no debe quedarse en la página 1");
    assert!(logo_rows(&doc, 1).is_none(), "el logo no debe quedarse en la página 2");
    // En la última: todo lo que seguía, en el mismo orden y con las mismas separaciones, sin que el resto del párrafo
    // quede pegado a ello (va en la página anterior).
    let p1 = elements(&doc, 2);
    let logo = logo_rows(&doc, 2).expect("el logo pasó a la última página");
    let line = *find(&p1, "Mas informacion");
    assert!(
        ((line[1] - logo.1) - gap0).abs() < 1.5,
        "separación logo-línea: {} frente a {gap0}",
        line[1] - logo.1
    );
    let segundo = find(&p1, "Segundo");
    let cuarto = find(&p1, "Cuarto");
    assert!(
        segundo[3] <= find(&p1, "Tercer")[1] && cuarto[3] < logo.0 && logo.1 < line[1],
        "el orden se conserva: {p1:#?}"
    );
    // El resto del párrafo ocupa la página 2 desde arriba y no toca el pie ni se sale.
    let mid = elements(&doc, 1);
    assert!(
        mid.iter().all(|(_, b)| b[3] <= 842.0 - 30.0),
        "el texto sobrante no debe pasar del margen inferior: {mid:#?}"
    );
}

#[test]
fn text_longer_than_several_pages_gets_whole_pages_and_the_rest_follows_it() {
    let mut doc = open(&[page_with_logo()]);
    let b0 = block_with(&doc, 0, "Texto de abertura");
    let words = std::iter::repeat_n(LONG, 40).collect::<Vec<_>>().join(" ");
    let r = replace_text_flow(&mut doc, 0, b0.id, &words, &Style::default(), &FlowOptions::default()).unwrap();
    let pages = doc.pdf().page_count().unwrap() as usize;
    assert!(pages >= 5, "{pages} páginas: {:?}", r.warnings);
    let mut lines = 0;
    for p in 0..pages {
        assert_no_overlaps(&doc, p);
        for (_, b) in elements(&doc, p) {
            assert!(b[3] <= 842.0 - 30.0 || p == 0, "página {p}: {b:?}");
        }
        lines += text_blocks(&doc, p)
            .unwrap()
            .iter()
            .filter(|b| b.text.contains("Texto nuevo"))
            .count();
    }
    assert!(lines >= pages - 1, "el texto debe estar repartido por las páginas");
    let last = elements(&doc, pages - 1);
    assert!(
        last.iter().any(|(n, _)| n.contains("Mas informacion")) && last.iter().any(|(n, _)| n.contains("Segundo")),
        "lo que seguía va tras el texto: {last:#?}"
    );
}
