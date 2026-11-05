use pliego_core::JobContext;
use pliego_pdf::PdfDocument;
use pliego_pdf::{Document, TextFlags};
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

fn doc(pages: usize) -> PdfDocument {
    PdfDocument::from_bytes(&build_pdf(pages)).unwrap()
}

fn reopen(d: &PdfDocument) -> Document {
    let mut buf = Vec::new();
    d.write_to(&mut buf).unwrap();
    Document::from_bytes(&buf, "application/pdf").unwrap()
}

fn text(d: &Document, i: i32) -> String {
    d.load_page(i)
        .unwrap()
        .to_text_page(TextFlags::empty())
        .unwrap()
        .to_text()
        .unwrap()
}

/// Rectángulo visual (y hacia abajo) de la primera aparición del texto.
fn find(d: &Document, i: i32, needle: &str) -> pliego_pdf::Rect {
    let hits = d.load_page(i).unwrap().search(needle, 1).unwrap();
    let q = hits.first().unwrap_or_else(|| panic!("no se encontró {needle:?}"));
    let xs = [q.ul.x, q.ur.x, q.ll.x, q.lr.x];
    let ys = [q.ul.y, q.ur.y, q.ll.y, q.lr.y];
    pliego_pdf::Rect::new(
        xs.iter().cloned().fold(f32::MAX, f32::min),
        ys.iter().cloned().fold(f32::MAX, f32::min),
        xs.iter().cloned().fold(f32::MIN, f32::max),
        ys.iter().cloned().fold(f32::MIN, f32::max),
    )
}

fn ctx() -> JobContext<'static> {
    JobContext::silent()
}

fn sctx() -> StampContext {
    StampContext {
        file: "informe.pdf".into(),
        date: "2026-11-03".into(),
    }
}

#[test]
fn header_footer_variables_and_position() {
    let mut d = doc(3);
    let hf = HeaderFooter {
        header: Slots {
            left: "{file}".into(),
            center: String::new(),
            right: "{date}".into(),
        },
        footer: Slots {
            center: "Página {page} de {pages}".into(),
            ..Default::default()
        },
        ..Default::default()
    };
    header_footer(&mut d, &[0, 1, 2], &hf, &sctx(), &ctx()).unwrap();
    let r = reopen(&d);
    assert!(text(&r, 1).contains("Página 2 de 3"));
    assert!(text(&r, 0).contains("informe.pdf"));
    assert!(text(&r, 2).contains("2026-11-03"));
    assert!(text(&r, 0).contains("P1"), "el contenido original se conserva");
    // Encabezado arriba, pie abajo y centrado.
    assert!(find(&r, 0, "informe.pdf").y1 < 80.0);
    let foot = find(&r, 0, "Página 1 de 3");
    assert!(foot.y0 > 842.0 - 80.0);
    assert!(((foot.x0 + foot.x1) / 2.0 - 297.5).abs() < 3.0);
}

#[test]
fn only_selected_pages_are_stamped() {
    let mut d = doc(3);
    let hf = HeaderFooter {
        footer: Slots {
            left: "SELLO".into(),
            ..Default::default()
        },
        ..Default::default()
    };
    header_footer(&mut d, &[1], &hf, &sctx(), &ctx()).unwrap();
    let r = reopen(&d);
    assert!(!text(&r, 0).contains("SELLO"));
    assert!(text(&r, 1).contains("SELLO"));
    assert!(!text(&r, 2).contains("SELLO"));
    assert!(header_footer(&mut d, &[9], &hf, &sctx(), &ctx()).is_err());
}

#[test]
fn rotated_page_keeps_header_visually_on_top() {
    let mut d = doc(1);
    d.load_pdf_page(0).unwrap().set_rotation(90).unwrap();
    let hf = HeaderFooter {
        header: Slots {
            left: "ARRIBA".into(),
            ..Default::default()
        },
        ..Default::default()
    };
    header_footer(&mut d, &[0], &hf, &sctx(), &ctx()).unwrap();
    let r = reopen(&d);
    let b = r.load_page(0).unwrap().bounds().unwrap();
    assert_eq!((b.width(), b.height()), (842.0, 595.0));
    let hit = find(&r, 0, "ARRIBA");
    assert!(hit.y1 < 80.0, "{hit:?}");
    assert!(hit.x0 < 80.0, "{hit:?}");
}

#[test]
fn bates_is_consecutive_with_padding() {
    let mut d = doc(3);
    let o = BatesOptions {
        prefix: "ABC-".into(),
        suffix: "/X".into(),
        digits: 5,
        start: 41,
        ..Default::default()
    };
    bates(&mut d, &[0, 1, 2], &o, &ctx()).unwrap();
    let r = reopen(&d);
    assert!(text(&r, 0).contains("ABC-00041/X"));
    assert!(text(&r, 2).contains("ABC-00043/X"));
    let h = find(&r, 1, "ABC-00042/X");
    assert!(h.x1 > 595.0 - 80.0 && h.y0 > 842.0 - 80.0, "{h:?}");
}

#[test]
fn watermark_text_front_and_behind() {
    let wm = |behind| Watermark {
        content: WatermarkContent::Text {
            text: "BORRADOR".into(),
            style: TextStyle {
                size: 60.0,
                ..Default::default()
            },
        },
        opacity: 0.25,
        rotation: 45.0,
        behind,
    };
    for behind in [false, true] {
        let mut d = doc(2);
        watermark(&mut d, &[0, 1], &wm(behind), &ctx()).unwrap();
        let r = reopen(&d);
        assert!(text(&r, 0).contains("BORRADOR") && text(&r, 1).contains("BORRADOR"));
        assert!(text(&r, 0).contains("P1"));
        // La opacidad queda registrada como estado gráfico.
        let mut raw = Vec::new();
        d.write_to_with_options(&mut raw, &pliego_pdf::WriteOptions::default())
            .unwrap();
        assert!(String::from_utf8_lossy(&raw).contains("/ca .25"));
    }
}

#[test]
fn background_color_renders() {
    let mut d = doc(1);
    background(
        &mut d,
        &[0],
        &Background {
            fill: BackgroundFill::Color { rgb: [1.0, 0.0, 0.0] },
            opacity: 1.0,
        },
        &ctx(),
    )
    .unwrap();
    let r = reopen(&d);
    let pix = r
        .load_page(0)
        .unwrap()
        .to_pixmap(
            &pliego_pdf::Matrix::new_scale(0.5, 0.5),
            pliego_pdf::Colorspace::Rgb,
            false,
            false,
        )
        .unwrap();
    let s = pix.samples();
    assert_eq!((s[0], s[1], s[2]), (255, 0, 0));
}

#[test]
fn options_roundtrip_json() {
    let hf = HeaderFooter {
        footer: Slots {
            center: "{page}".into(),
            ..Default::default()
        },
        ..Default::default()
    };
    let j = serde_json::to_string(&hf).unwrap();
    assert_eq!(serde_json::from_str::<HeaderFooter>(&j).unwrap(), hf);
    let b: BatesOptions = serde_json::from_str(r#"{"prefix":"X"}"#).unwrap();
    assert_eq!((b.digits, b.start), (6, 1));
}
