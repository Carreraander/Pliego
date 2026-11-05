//! Pruebas de la redacción nativa: se construyen PDF pequeños a mano y se comprueba el resultado con MuPDF como
//! lector independiente y con los bytes del archivo.

use lopdf::Object as L;

use super::super::PdfDocument as Native;
use crate::{Document, Rect, RedactImageMethod, RedactLineArtMethod, RedactOptions, RedactTextMethod, WriteOptions};

/// PDF con los objetos dados (el 1 es el catálogo; la numeración sigue el orden).
fn build(objs: &[Vec<u8>]) -> Vec<u8> {
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

fn stream(dict: &str, data: &[u8]) -> Vec<u8> {
    let mut v = format!("<< {dict} /Length {} >>\nstream\n", data.len()).into_bytes();
    v.extend_from_slice(data);
    v.extend_from_slice(b"\nendstream");
    v
}

/// Catálogo, árbol y una página (objetos 1-3 y 4) más `extra` a partir del 5. `page_extra` va en el diccionario de la página.
fn one_page(content: &str, resources: &str, page_extra: &str, extra: Vec<Vec<u8>>) -> Vec<u8> {
    let mut objs = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << {resources} >> /Contents 4 0 R {page_extra} >>")
            .into_bytes(),
        stream("", content.as_bytes()),
    ];
    objs.extend(extra);
    build(&objs)
}

const HELV: &str = "/Font << /F1 5 0 R >>";

fn helv() -> Vec<u8> {
    b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec()
}

fn opts() -> RedactOptions {
    RedactOptions {
        black_boxes: false,
        image_method: RedactImageMethod::Pixels,
        line_art: RedactLineArtMethod::RemoveIfCovered,
        text: RedactTextMethod::Remove,
    }
}

/// Zonas (espacio de página) donde MuPDF encuentra `needle`.
fn find(bytes: &[u8], needle: &str) -> Vec<Rect> {
    let doc = Document::from_bytes(bytes, "application/pdf").unwrap();
    doc.load_page(0)
        .unwrap()
        .search(needle, 16)
        .unwrap()
        .into_iter()
        .map(Rect::from)
        .collect()
}

fn text(bytes: &[u8]) -> String {
    let doc = Document::from_bytes(bytes, "application/pdf").unwrap();
    doc.load_page(0).unwrap().text().unwrap()
}

fn redact(bytes: &[u8], areas: &[Rect], o: RedactOptions) -> (Vec<u8>, super::RedactReport) {
    let mut doc = Native::from_bytes(bytes).unwrap();
    let list: Vec<(i32, Rect)> = areas.iter().map(|r| (0, *r)).collect();
    let rep = doc.redact_areas(&list, o).unwrap();
    let out = doc.to_bytes(&WriteOptions::new()).unwrap();
    (out, rep)
}

fn has(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

/// Todos los flujos del archivo, descomprimidos (para buscar restos).
fn all_streams(bytes: &[u8]) -> Vec<u8> {
    let doc = Native::from_bytes(bytes).unwrap();
    let mut all = Vec::new();
    for n in 1..doc.count_objects().unwrap() as i32 {
        if let Ok(s) = doc.xref_stream(n) {
            all.extend(s);
        }
    }
    all
}

#[test]
fn removes_text_and_keeps_neighbours() {
    let content = "BT /F1 11 Tf 72 780 Td (Nombre: Ana Lopez   DNI 12345678Z) Tj 0 -14 Td (otra linea intacta) Tj ET";
    let pdf = one_page(content, HELV, "", vec![helv()]);
    let hits = find(&pdf, "12345678Z");
    assert_eq!(hits.len(), 1);
    let (out, rep) = redact(&pdf, &hits, opts());
    let t = text(&out);
    assert!(!t.contains("12345678Z"), "{t:?}");
    assert!(t.contains("Nombre: Ana Lopez"), "{t:?}");
    assert!(t.contains("otra linea intacta"), "{t:?}");
    assert_eq!(rep.glyphs_removed, 9);
    assert!(!has(&all_streams(&out), b"12345678Z"));
    assert!(!has(&all_streams(&out), b"2345678"));
    // El texto que queda no se movió.
    let before = find(&pdf, "DNI");
    let after = find(&out, "DNI");
    assert_eq!(before.len(), 1);
    assert!((before[0].x0 - after[0].x0).abs() < 0.05, "{before:?} {after:?}");
}

#[test]
fn tj_array_keeps_kerning_and_positions() {
    let content = "BT /F1 12 Tf 50 700 Td [(Hol) -120 (a ) 40 (mundo secreto)] TJ ET";
    let pdf = one_page(content, HELV, "", vec![helv()]);
    let before = find(&pdf, "Hola");
    let secret = find(&pdf, "secreto");
    let (out, _) = redact(&pdf, &secret, opts());
    let t = text(&out);
    assert!(!t.contains("secreto") && t.contains("mundo"), "{t:?}");
    let after = find(&out, "Hola");
    assert!((before[0].x0 - after[0].x0).abs() < 0.05 && (before[0].x1 - after[0].x1).abs() < 0.05);
    let mundo_b = find(&pdf, "mundo");
    let mundo_a = find(&out, "mundo");
    assert!((mundo_b[0].x0 - mundo_a[0].x0).abs() < 0.05);
}

#[test]
fn text_none_leaves_text() {
    let pdf = one_page("BT /F1 12 Tf 50 700 Td (secreto) Tj ET", HELV, "", vec![helv()]);
    let hits = find(&pdf, "secreto");
    let mut o = opts();
    o.text = RedactTextMethod::None;
    let (out, rep) = redact(&pdf, &hits, o);
    assert!(text(&out).contains("secreto"));
    assert_eq!(rep.glyphs_removed, 0);
}

#[test]
fn quote_operators_and_leading() {
    let content = "BT /F1 12 Tf 14 TL 50 700 Td (primera) Tj (segunda secreta) ' 3 1 (tercera) \" ET";
    let pdf = one_page(content, HELV, "", vec![helv()]);
    let hits = find(&pdf, "secreta");
    let (out, _) = redact(&pdf, &hits, opts());
    let t = text(&out);
    assert!(
        !t.contains("secreta") && t.contains("primera") && t.contains("tercera"),
        "{t:?}"
    );
    // «tercera» no se movió.
    let b = find(&pdf, "tercera");
    let a = find(&out, "tercera");
    assert!((b[0].x0 - a[0].x0).abs() < 0.05 && (b[0].y0 - a[0].y0).abs() < 0.05);
}

#[test]
fn black_boxes_are_drawn() {
    let pdf = one_page("BT /F1 12 Tf 50 700 Td (secreto) Tj ET", HELV, "", vec![helv()]);
    let hits = find(&pdf, "secreto");
    let mut o = opts();
    o.black_boxes = true;
    let (out, _) = redact(&pdf, &hits, o);
    let streams = String::from_utf8_lossy(&all_streams(&out)).into_owned();
    assert!(streams.contains("0 g") && streams.contains(" re f"), "{streams}");
    assert!(!text(&out).contains("secreto"));
}

#[test]
fn form_xobject_is_copied_not_modified() {
    // La misma forma se usa en dos sitios; solo uno cae bajo la zona.
    let form = stream(
        "/Type /XObject /Subtype /Form /BBox [0 0 200 20] /Resources << /Font << /F1 5 0 R >> >>",
        b"BT /F1 12 Tf 0 5 Td (clave secreta) Tj ET",
    );
    let content = "q 1 0 0 1 50 600 cm /Fm1 Do Q q 1 0 0 1 50 300 cm /Fm1 Do Q";
    let pdf = one_page(
        content,
        &format!("{HELV} /XObject << /Fm1 6 0 R >>"),
        "",
        vec![helv(), form],
    );
    let hits = find(&pdf, "secreta");
    assert_eq!(hits.len(), 2);
    // La zona de la copia de arriba (y menor en espacio de página = más arriba).
    let top = hits.iter().min_by(|a, b| a.y0.total_cmp(&b.y0)).unwrap();
    let (out, rep) = redact(&pdf, &[*top], opts());
    assert_eq!(rep.forms_rewritten, 1);
    let doc = Document::from_bytes(&out, "application/pdf").unwrap();
    let t = doc.load_page(0).unwrap().text().unwrap();
    assert_eq!(t.matches("secreta").count(), 1, "{t:?}");
    assert_eq!(t.matches("clave").count(), 2, "{t:?}");
}

/// Imagen RGB 8x8 sin filtro con un valor distinto por píxel.
fn rgb_image() -> Vec<u8> {
    let mut data = Vec::new();
    for y in 0..8u8 {
        for x in 0..8u8 {
            data.extend_from_slice(&[200, 10 + x, 10 + y]);
        }
    }
    stream(
        "/Type /XObject /Subtype /Image /Width 8 /Height 8 /ColorSpace /DeviceRGB /BitsPerComponent 8",
        &data,
    )
}

fn image_pixels(bytes: &[u8], obj: i32) -> Vec<u8> {
    Native::from_bytes(bytes).unwrap().xref_stream(obj).unwrap()
}

#[test]
fn image_pixels_are_erased_in_a_copy() {
    // Imagen de 80x80 en (100,500)-(180,580): cada píxel de la fuente mide 10x10 puntos.
    let content = "q 80 0 0 80 100 500 cm /Im1 Do Q";
    let pdf = one_page(content, "/XObject << /Im1 5 0 R >>", "", vec![rgb_image()]);
    // Zona en espacio de página (y hacia abajo): cubre las columnas 2-3 y las filas de arriba 0-1.
    let area = Rect::new(120.0, 842.0 - 580.0, 140.0, 842.0 - 560.0);
    let (out, rep) = redact(&pdf, &[area], opts());
    assert_eq!(rep.images_erased, 1);
    // La imagen original se conserva intacta y la copia tiene los píxeles en blanco.
    let orig = image_pixels(&pdf, 5);
    assert_eq!(orig.len(), 8 * 8 * 3);
    let doc = Native::from_bytes(&out).unwrap();
    let mut found = false;
    for n in 1..doc.count_objects().unwrap() as i32 {
        if let Ok(Some(o)) = doc.xref_object(n)
            && o.get_dict("Subtype")
                .ok()
                .flatten()
                .is_some_and(|s| s.as_name().ok() == Some(b"Image".to_vec()))
        {
            let px = doc.xref_stream(n).unwrap();
            assert_eq!(px.len(), 8 * 8 * 3);
            let at = |x: usize, y: usize| &px[(y * 8 + x) * 3..(y * 8 + x) * 3 + 3];
            assert_eq!(at(2, 0), [255, 255, 255]);
            assert_eq!(at(3, 1), [255, 255, 255]);
            assert_eq!(at(0, 0), [200, 10, 10]);
            assert_eq!(at(5, 5), [200, 15, 15]);
            found = true;
        }
    }
    assert!(found);
}

#[test]
fn image_remove_and_none_methods() {
    let content = "q 80 0 0 80 100 500 cm /Im1 Do Q";
    let pdf = one_page(content, "/XObject << /Im1 5 0 R >>", "", vec![rgb_image()]);
    let area = Rect::new(120.0, 842.0 - 580.0, 140.0, 842.0 - 560.0);
    let mut o = opts();
    o.image_method = RedactImageMethod::Remove;
    let (out, rep) = redact(&pdf, &[area], o);
    assert_eq!(rep.images_removed, 1);
    assert!(!has(&all_streams(&out), &[200, 10, 10]));
    o.image_method = RedactImageMethod::None;
    let (out, rep) = redact(&pdf, &[area], o);
    assert_eq!(rep.images_removed + rep.images_erased, 0);
    assert!(has(&all_streams(&out), &[200, 10, 10]));
}

#[test]
fn line_art_covered_removed_clips_kept() {
    let content = "q 1 0 0 rg 100 100 20 20 re f Q q 0 1 0 rg 300 300 200 200 re f Q q 50 50 400 400 re W n 0 0 1 rg 105 105 5 5 re f Q";
    let pdf = one_page(content, "", "", vec![]);
    let area = Rect::new(95.0, 842.0 - 125.0, 125.0, 842.0 - 95.0);
    let (out, rep) = redact(&pdf, &[area], opts());
    assert_eq!(rep.paths_removed, 2);
    let s = String::from_utf8_lossy(&all_streams(&out)).into_owned();
    assert!(!s.contains("100 100 20 20 re"), "{s}");
    assert!(s.contains("300 300 200 200 re"), "{s}");
    assert!(s.contains("50 50 400 400 re"), "{s}");
}

#[test]
fn opaque_font_loses_whole_string() {
    // Symbol sin /Widths: no se puede medir, se trata la operación entera.
    let font = b"<< /Type /Font /Subtype /Type1 /BaseFont /Symbol >>".to_vec();
    let content = "BT /F1 12 Tf 50 700 Td (abcdef) Tj 0 -30 Td (otra) Tj ET";
    let pdf = one_page(content, HELV, "", vec![font]);
    let area = Rect::new(52.0, 842.0 - 705.0, 70.0, 842.0 - 695.0);
    let (out, rep) = redact(&pdf, &[area], opts());
    assert!(rep.glyphs_removed >= 6);
    assert!(!rep.warnings.is_empty());
    let s = all_streams(&out);
    assert!(!has(&s, b"abcdef"));
    assert!(has(&s, b"otra"));
}

#[test]
fn identity_h_cid_font_is_measured() {
    // Tres glifos de 2 bytes; el segundo mide 2000 (/W) y el resto 500 (/DW).
    let font = b"<< /Type /Font /Subtype /Type0 /BaseFont /X /Encoding /Identity-H /DescendantFonts [<< /Type /Font /Subtype /CIDFontType2 /BaseFont /X /DW 500 /W [2 [2000]] /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> >>] >>".to_vec();
    let content = "BT /F1 10 Tf 100 700 Td <000100020003> Tj ET";
    let pdf = one_page(content, HELV, "", vec![font]);
    // Primer glifo: x 100-105; segundo: 105-125; tercero: 125-130. Zona sobre el tercero.
    let area = Rect::new(126.0, 842.0 - 710.0, 129.0, 842.0 - 700.0);
    let (out, rep) = redact(&pdf, &[area], opts());
    assert_eq!(rep.glyphs_removed, 1);
    let s = String::from_utf8_lossy(&all_streams(&out)).into_owned();
    assert!(s.contains("<0001") || s.contains("\\000\\001") || !s.is_empty());
    assert!(!has(&all_streams(&out), &[0, 3]));
    assert!(has(&all_streams(&out), &[0, 1, 0, 2]));
}

#[test]
fn annotations_in_area_are_removed_and_fields_scrubbed() {
    let annot = b"<< /Type /Annot /Subtype /Widget /FT /Tx /T (campo) /V (secreto) /Rect [100 100 200 120] /AP << /N 7 0 R >> >>".to_vec();
    let other = b"<< /Type /Annot /Subtype /Text /Contents (nota) /Rect [400 400 420 420] >>".to_vec();
    let ap = stream("/Type /XObject /Subtype /Form /BBox [0 0 100 20]", b"BT ET");
    let pdf = one_page("", "", "/Annots [5 0 R 6 0 R]", vec![annot, other, ap]);
    let area = Rect::new(90.0, 842.0 - 125.0, 210.0, 842.0 - 95.0);
    let (out, rep) = redact(&pdf, &[area], opts());
    assert_eq!(rep.annotations_removed, 1);
    let doc = Native::from_bytes(&out).unwrap();
    let page = doc.load_pdf_page(0).unwrap().object();
    assert_eq!(page.get_dict("Annots").unwrap().unwrap().len().unwrap(), 1);
    assert!(!has(&out, b"secreto"));
}

#[test]
fn actual_text_is_scrubbed_where_text_was_removed() {
    let content = "/Span << /MCID 0 /ActualText (clave secreta) >> BDC BT /F1 12 Tf 50 700 Td (clave secreta) Tj ET EMC /Span << /Alt (otra cosa) >> BDC BT /F1 12 Tf 50 600 Td (intacto) Tj ET EMC";
    let pdf = one_page(content, HELV, "", vec![helv()]);
    let hits = find(&pdf, "secreta");
    let (out, _) = redact(&pdf, &hits, opts());
    let s = all_streams(&out);
    assert!(!has(&s, b"clave secreta"));
    assert!(has(&s, b"otra cosa"), "el Alt de contenido no tocado se conserva");
    assert!(has(&s, b"/MCID 0"));
}

#[test]
fn redact_marks_are_applied_and_removed() {
    let annot =
        b"<< /Type /Annot /Subtype /Redact /Rect [0 0 0 0] /QuadPoints [50 712 91 712 50 698 91 698] >>".to_vec();
    let content = "BT /F1 12 Tf 50 700 Td (secreto de estado) Tj ET";
    let pdf = one_page(content, HELV, "/Annots [6 0 R]", vec![helv(), annot]);
    let mut doc = Native::from_bytes(&pdf).unwrap();
    let rep = doc.apply_redactions(opts()).unwrap();
    assert_eq!(rep.pages_changed, 1);
    assert_eq!(rep.annotations_removed, 1);
    let out = doc.to_bytes(&WriteOptions::new()).unwrap();
    let t = text(&out);
    assert!(!t.contains("secreto") && t.contains("estado"), "{t:?}");
    let page = Native::from_bytes(&out).unwrap().load_pdf_page(0).unwrap().object();
    assert_eq!(page.get_dict("Annots").unwrap().unwrap().len().unwrap(), 0);
}

#[test]
fn rotated_page_uses_page_space_areas() {
    let content = "BT /F1 12 Tf 50 700 Td (secreto) Tj ET";
    let pdf = one_page(content, HELV, "/Rotate 90", vec![helv()]);
    let hits = find(&pdf, "secreto");
    assert_eq!(hits.len(), 1);
    let (out, _) = redact(&pdf, &hits, opts());
    assert!(!text(&out).contains("secreto"));
}

#[test]
fn thumbnails_are_dropped() {
    let pdf = one_page(
        "BT /F1 12 Tf 50 700 Td (secreto) Tj ET",
        HELV,
        "/Thumb 6 0 R",
        vec![
            helv(),
            stream("/Width 1 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 8", b"x"),
        ],
    );
    let hits = find(&pdf, "secreto");
    let (out, _) = redact(&pdf, &hits, opts());
    let doc = Native::from_bytes(&out).unwrap();
    assert!(
        doc.load_pdf_page(0)
            .unwrap()
            .object()
            .get_dict("Thumb")
            .unwrap()
            .is_none()
    );
}

#[test]
fn unused_value_marker() {
    // Evita avisos si cambian las importaciones.
    let _ = L::Null;
}

/// Texto de todas las páginas (hasta 4) según MuPDF.
fn texts(bytes: &[u8]) -> Option<Vec<String>> {
    let doc = Document::from_bytes(bytes, "application/pdf").ok()?;
    let n = doc.page_count().ok()?.min(4);
    let mut v = Vec::new();
    for i in 0..n {
        v.push(doc.load_page(i).ok()?.text().ok()?);
    }
    Some(v)
}

/// Redactar la página entera en todo el corpus no deja texto extraíble y la salida sigue abriéndose.
#[test]
fn corpus_full_page_redaction_leaves_no_text() {
    let mut checked = 0;
    let mut with_text = 0;
    let mut leaks: Vec<String> = Vec::new();
    for path in super::super::tests::corpus() {
        if path.ends_with("tiling-hang.pdf") {
            continue;
        }
        let bytes = std::fs::read(&path).unwrap();
        let Some(before) = texts(&bytes) else { continue };
        let Ok(mut doc) = Native::from_bytes(&bytes) else {
            continue;
        };
        if doc.needs_password().unwrap_or(true) {
            continue;
        }
        let pages = doc.page_count().unwrap().min(4);
        let mut areas = Vec::new();
        for p in 0..pages {
            let b = doc.load_pdf_page(p).unwrap().bounds().unwrap();
            areas.push((p, b));
        }
        let rep = match doc.redact_areas(&areas, opts()) {
            Ok(r) => r,
            Err(e) => panic!("{}: {e}", path.display()),
        };
        let out = doc.to_bytes(&WriteOptions::new()).unwrap();
        let Some(after) = texts(&out) else {
            panic!("{}: MuPDF no abre la salida", path.display())
        };
        for (i, (b, a)) in before.iter().zip(&after).enumerate() {
            if a.chars().any(|c| !c.is_whitespace()) {
                leaks.push(format!(
                    "{} p{}: queda {:?} (antes {} caracteres; avisos {:?})",
                    path.file_name().unwrap().to_string_lossy(),
                    i + 1,
                    a.chars().take(40).collect::<String>(),
                    b.len(),
                    rep.warnings
                ));
            }
        }
        if before.iter().any(|t| t.chars().any(|c| !c.is_whitespace())) {
            with_text += 1;
        }
        checked += 1;
    }
    assert!(with_text > 30, "solo {with_text} PDF con texto");
    assert!(checked > 60, "se comprobaron {checked} PDF");
    assert!(leaks.is_empty(), "{} fugas:\n{}", leaks.len(), leaks.join("\n"));
}

/// Redactar una palabra suelta en todo el corpus quita esa palabra y casi nada más (sin sobre-redacción).
#[test]
fn corpus_single_word_redaction_is_precise() {
    let mut tried = 0;
    let mut problems: Vec<String> = Vec::new();
    for path in super::super::tests::corpus() {
        if path.ends_with("tiling-hang.pdf") {
            continue;
        }
        let bytes = std::fs::read(&path).unwrap();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let Ok(doc) = Document::from_bytes(&bytes, "application/pdf") else {
            continue;
        };
        let Ok(page) = doc.load_page(0) else { continue };
        let Ok(tp) = page.to_text_page(crate::TextFlags::empty()) else {
            continue;
        };
        // Una palabra alfabética de 5 a 12 letras, que aparezca una sola vez en la página.
        let words = tp.words();
        let all = page.text().unwrap_or_default();
        let Some(w) = words.iter().find(|w| {
            let n = w.text.chars().count();
            (5..=12).contains(&n) && w.text.chars().all(char::is_alphabetic) && all.matches(&w.text).count() == 1
        }) else {
            continue;
        };
        let squash = |s: &str| s.chars().filter(|c| !c.is_whitespace()).collect::<String>();
        let before = squash(&all);
        let b = w.bounds;
        let dy = (b.y1 - b.y0) * 0.25;
        let area = Rect::new(b.x0, b.y0 + dy, b.x1, b.y1 - dy);
        let Ok(mut nd) = Native::from_bytes(&bytes) else {
            continue;
        };
        if nd.needs_password().unwrap_or(true) {
            continue;
        }
        nd.redact_areas(&[(0, area)], opts()).unwrap();
        let out = nd.to_bytes(&WriteOptions::new()).unwrap();
        let Some(after_pages) = texts(&out) else {
            problems.push(format!("{name}: no abre"));
            continue;
        };
        let after = squash(&after_pages[0]);
        tried += 1;
        if after.contains(&squash(&w.text)) {
            problems.push(format!("{name}: sigue «{}»", w.text));
        }
        let lost = before.chars().count().saturating_sub(after.chars().count());
        if lost > w.text.chars().count() + 3 {
            problems.push(format!(
                "{name}: «{}» ({} letras) pero se perdieron {lost} caracteres",
                w.text,
                w.text.chars().count()
            ));
        }
    }
    assert!(tried > 15, "solo {tried} documentos probados");
    assert!(
        problems.is_empty(),
        "{} problemas:\n{}",
        problems.len(),
        problems.join("\n")
    );
}

#[test]
fn jpeg_pixels_are_erased() {
    // JPEG RGB de 32x32 con una franja roja a la izquierda y azul a la derecha.
    let mut px = Vec::new();
    for _y in 0..32 {
        for x in 0..32 {
            px.extend_from_slice(if x < 16 { &[250, 0, 0] } else { &[0, 0, 250] });
        }
    }
    let mut jpg = Vec::new();
    jpeg_encoder::Encoder::new(&mut jpg, 95)
        .encode(&px, 32, 32, jpeg_encoder::ColorType::Rgb)
        .unwrap();
    let img = stream(
        "/Type /XObject /Subtype /Image /Width 32 /Height 32 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode",
        &jpg,
    );
    let content = "q 64 0 0 64 100 500 cm /Im1 Do Q";
    let pdf = one_page(content, "/XObject << /Im1 5 0 R >>", "", vec![img]);
    // Mitad izquierda de la imagen (x 100-132) y todo su alto.
    let area = Rect::new(100.0, 842.0 - 564.0, 132.0, 842.0 - 500.0);
    let (out, rep) = redact(&pdf, &[area], opts());
    assert_eq!(rep.images_erased, 1);
    // Se dibuja la página con MuPDF: la izquierda en blanco y la derecha sigue azul.
    let doc = Document::from_bytes(&out, "application/pdf").unwrap();
    let page = doc.load_page(0).unwrap();
    let pm = page
        .to_pixmap(&crate::Matrix::IDENTITY, crate::Colorspace::Rgb, false, false)
        .unwrap();
    let at = |x: usize, y: usize| {
        let i = y * pm.stride() + x * pm.n() as usize;
        let s = pm.samples();
        (s[i], s[i + 1], s[i + 2])
    };
    // y de página = 842 - 532 = 310 (a mitad de la imagen).
    let l = at(110, 310);
    let r = at(155, 310);
    assert!(l.0 > 240 && l.1 > 240 && l.2 > 240, "izquierda en blanco: {l:?}");
    assert!(r.2 > 200 && r.0 < 60, "derecha azul: {r:?}");
}

#[test]
fn inline_images_under_the_area_are_removed() {
    let content = "q 40 0 0 40 100 500 cm BI /W 2 /H 2 /BPC 8 /CS /G ID \x01\x02\x03\x04 EI Q BT /F1 12 Tf 300 300 Td (queda) Tj ET";
    let pdf = one_page(content, HELV, "", vec![helv()]);
    let area = Rect::new(105.0, 842.0 - 520.0, 120.0, 842.0 - 510.0);
    let (out, rep) = redact(&pdf, &[area], opts());
    assert_eq!(rep.images_removed, 1);
    let s = all_streams(&out);
    assert!(!has(&s, b"BI"), "{:?}", String::from_utf8_lossy(&s));
    assert!(has(&s, b"queda"));
}

#[test]
fn invisible_text_and_hidden_layers_are_removed_too() {
    // OCR invisible (Tr 3) bajo una imagen y texto en una capa opcional apagada.
    let content = "BT /F1 12 Tf 3 Tr 50 700 Td (texto del OCR) Tj ET /OC /oc1 BDC BT /F1 12 Tf 0 Tr 50 650 Td (capa oculta) Tj ET EMC";
    let pdf = one_page(
        content,
        &format!("{HELV} /Properties << /oc1 6 0 R >>"),
        "",
        vec![helv(), b"<< /Type /OCG /Name (oculta) >>".to_vec()],
    );
    let areas = [
        Rect::new(40.0, 842.0 - 715.0, 200.0, 842.0 - 695.0),
        Rect::new(40.0, 842.0 - 665.0, 200.0, 842.0 - 645.0),
    ];
    let (out, rep) = redact(&pdf, &areas, opts());
    let s = all_streams(&out);
    assert!(
        !has(&s, b"OCR") && !has(&s, b"oculta"),
        "{:?}",
        String::from_utf8_lossy(&s)
    );
    assert!(rep.glyphs_removed >= 20);
}

#[test]
fn encrypted_input_is_redacted_with_password_and_saved_decrypted_or_encrypted() {
    let pdf = one_page(
        "BT /F1 12 Tf 50 700 Td (secreto de estado) Tj ET",
        HELV,
        "",
        vec![helv()],
    );
    let hits = find(&pdf, "secreto");
    let plain = Native::from_bytes(&pdf).unwrap();
    let mut o = WriteOptions::new();
    o.set_encryption(crate::Encryption::Aes256)
        .set_user_password("u")
        .set_owner_password("o");
    let enc = plain.to_bytes(&o).unwrap();
    let mut doc = Native::from_bytes(&enc).unwrap();
    assert!(doc.needs_password().unwrap());
    assert!(doc.authenticate("u").unwrap());
    doc.redact_areas(&[(0, hits[0])], opts()).unwrap();
    let out = doc.to_bytes(&WriteOptions::new()).unwrap();
    assert!(!doc.can_be_saved_incrementally());
    let again = Native::from_bytes_with_password(&out, Some("u")).unwrap();
    let page = again.load_pdf_page(0).unwrap();
    let c = page.contents().unwrap().unwrap().read_stream().unwrap();
    assert!(
        !has(&c, b"secreto") && has(&c, b"estado"),
        "{:?}",
        String::from_utf8_lossy(&c)
    );
}

#[test]
fn shared_content_stream_between_pages_is_not_leaked() {
    // Dos páginas comparten el mismo flujo de contenido; se redacta solo en la primera.
    let objs = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 6 0 R >> >> /Contents 5 0 R >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 6 0 R >> >> /Contents 5 0 R >>".to_vec(),
        stream("", b"BT /F1 12 Tf 50 700 Td (secreto) Tj ET"),
        helv(),
    ];
    let pdf = build(&objs);
    let hits = find(&pdf, "secreto");
    let (out, _) = redact(&pdf, &hits, opts());
    let doc = Document::from_bytes(&out, "application/pdf").unwrap();
    assert!(!doc.load_page(0).unwrap().text().unwrap().contains("secreto"));
    assert!(doc.load_page(1).unwrap().text().unwrap().contains("secreto"));
}

#[test]
fn page_with_array_contents_and_inherited_resources() {
    let objs = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 /Resources << /Font << /F1 7 0 R >> >> /MediaBox [0 0 595 842] >>"
            .to_vec(),
        b"<< /Type /Page /Parent 2 0 R /Contents [4 0 R 5 0 R 6 0 R] >>".to_vec(),
        stream("", b"BT /F1 12 Tf 50 700 Td"),
        stream("", b"(secreto) Tj 0 -20 Td"),
        stream("", b"(publico) Tj ET"),
        helv(),
    ];
    let pdf = build(&objs);
    let hits = find(&pdf, "secreto");
    let (out, _) = redact(&pdf, &hits, opts());
    let t = text(&out);
    assert!(!t.contains("secreto") && t.contains("publico"), "{t:?}");
}
