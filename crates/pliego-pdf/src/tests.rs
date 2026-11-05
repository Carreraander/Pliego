use crate::*;

/// Documento de dos páginas A4 con «Hola mundo» (página 1) y «Segunda» (página 2).
fn sample() -> PdfDocument {
    let mut doc = PdfDocument::new();
    let font = Font::new("Helvetica").unwrap();
    let fobj = doc.add_simple_font(&font, SimpleFontEncoding::Latin).unwrap();
    for text in ["Hola mundo", "Segunda"] {
        let mut page = doc.new_page(Size::A4).unwrap();
        let mut res = page.resources().unwrap();
        let mut fonts = doc.new_dict().unwrap();
        fonts.dict_put("F1", fobj.try_clone().unwrap()).unwrap();
        res.dict_put("Font", fonts).unwrap();
        let stream = format!("BT /F1 24 Tf 72 700 Td ({text}) Tj ET");
        page.insert_contents(&mut doc, stream.as_bytes(), true).unwrap();
    }
    doc
}

fn reopen(doc: &PdfDocument, opts: &WriteOptions) -> PdfDocument {
    PdfDocument::from_bytes(&doc.to_bytes(opts).unwrap()).unwrap()
}

#[test]
fn pages_text_search_and_render() {
    let doc = reopen(&sample(), &WriteOptions::new());
    assert_eq!(doc.page_count().unwrap(), 2);
    let page = doc.load_page(0).unwrap();
    let b = page.bounds().unwrap();
    assert_eq!((b.width().round(), b.height().round()), (595.0, 842.0));
    assert!(page.text().unwrap().contains("Hola mundo"));
    let hits = page.search("mundo", 8).unwrap();
    assert_eq!(hits.len(), 1);
    // El texto está a 700 pt del borde inferior => cerca de 842-700 desde arriba.
    assert!(hits[0].ul.y > 100.0 && hits[0].ul.y < 160.0, "{:?}", hits[0]);

    let pix = page
        .to_pixmap(&Matrix::new_scale(0.5, 0.5), Colorspace::Rgb, false, true)
        .unwrap();
    assert_eq!((pix.width(), pix.height()), (298, 421));
    assert!(pix.samples().iter().any(|&v| v < 128), "debería haber píxeles oscuros");
    let tile = page
        .render_region(&Matrix::IDENTITY, IRect::new(0, 0, 100, 100), Colorspace::Rgb, true)
        .unwrap();
    assert_eq!((tile.width(), tile.height()), (100, 100));
    assert!(tile.samples().iter().all(|&v| v == 255), "esquina en blanco");
}

#[test]
fn structured_text_has_fonts_and_geometry() {
    let doc = reopen(&sample(), &WriteOptions::new());
    let tp = doc.load_page(0).unwrap().to_text_page(TextFlags::empty()).unwrap();
    let blocks = tp.blocks();
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].kind, TextBlockKind::Text);
    let line = &blocks[0].lines[0];
    assert_eq!(line.text(), "Hola mundo");
    let c = &line.chars[0];
    assert_eq!(c.ch, Some('H'));
    assert!((c.size - 24.0).abs() < 0.01);
    assert!(c.font.as_ref().unwrap().name.contains("Helvetica"));
    assert!(tp.words().iter().any(|w| w.text == "mundo"));
    assert!(tp.to_text().unwrap().contains("Hola"));
}

#[test]
fn object_model_roundtrip() {
    let doc = sample();
    let mut catalog = doc.catalog().unwrap();
    assert!(catalog.is_dict().unwrap());
    catalog.dict_put_name("PageMode", "UseOutlines").unwrap();
    assert_eq!(
        catalog.dict_get_name("PageMode").unwrap().as_deref(),
        Some("UseOutlines")
    );
    // Reescribir y volver a leer.
    let value = Value::Dict(vec![
        ("A".into(), Value::Int(3)),
        (
            "B".into(),
            Value::Array(vec![Value::Real(1.5), Value::Name("X".into()), Value::Bool(true)]),
        ),
        ("S".into(), Value::String(b"hola".to_vec())),
    ]);
    let obj = doc.object_from_value(&value).unwrap();
    let mut catalog = doc.catalog().unwrap();
    catalog.dict_put("Extra", obj).unwrap();
    let back = doc.catalog().unwrap().get_dict("Extra").unwrap().unwrap();
    assert_eq!(back.to_value(8).unwrap(), value);
    assert_eq!(back.get_dict("A").unwrap().unwrap().as_int().unwrap(), 3);
    let mut keys: Vec<_> = back.dict_iter().unwrap().into_iter().map(|(k, _)| k).collect();
    keys.sort();
    assert_eq!(keys, ["A", "B", "S"]);
    // Sobrevive al guardado.
    let saved = reopen(&doc, &WriteOptions::new());
    let extra = saved.catalog().unwrap().get_dict("Extra").unwrap().unwrap();
    assert_eq!(extra.get_dict("S").unwrap().unwrap().as_bytes().unwrap(), b"hola");
    // Referencias indirectas.
    let page0 = saved.find_page(0).unwrap();
    assert_eq!(saved.lookup_page_number(&page0).unwrap(), 0);
    let r = saved.new_indirect(page0.as_indirect().unwrap(), 0).unwrap();
    assert!(r.is_indirect().unwrap() && r.resolve().unwrap().is_some());
}

#[test]
fn streams_read_and_write() {
    let mut doc = PdfDocument::new();
    let dict = doc.new_dict().unwrap();
    let mut s = doc.add_stream(b"q 1 0 0 1 0 0 cm Q", Some(&dict), false).unwrap();
    assert!(s.is_stream().unwrap());
    assert_eq!(s.read_stream().unwrap(), b"q 1 0 0 1 0 0 cm Q");
    s.write_stream(b"nuevo").unwrap();
    assert_eq!(s.read_stream().unwrap(), b"nuevo");
}

#[test]
fn save_options_compress_and_garbage() {
    let doc = sample();
    let plain = doc.to_bytes(&WriteOptions::new()).unwrap();
    let mut o = WriteOptions::new();
    o.set_compress(true).set_garbage_level(2);
    let small = doc.to_bytes(&o).unwrap();
    let has = |b: &[u8]| b.windows(10).any(|w| w == b"Hola mundo");
    assert!(has(&plain), "sin comprimir el texto del flujo es visible");
    assert!(!has(&small), "comprimido ya no lo es");
    assert_eq!(PdfDocument::from_bytes(&small).unwrap().page_count().unwrap(), 2);
}

#[test]
fn encryption_and_password() {
    let doc = sample();
    let mut o = WriteOptions::new();
    o.set_encryption(Encryption::Aes256)
        .set_user_password("abrir")
        .set_owner_password("dueño")
        .set_permissions(Permission::PRINT);
    let enc = doc.to_bytes(&o).unwrap();
    assert!(matches!(
        PdfDocument::from_bytes_with_password(&enc, None),
        Err(pliego_core::Error::Password)
    ));
    assert!(matches!(
        PdfDocument::from_bytes_with_password(&enc, Some("mal")),
        Err(pliego_core::Error::Password)
    ));
    let open = PdfDocument::from_bytes_with_password(&enc, Some("abrir")).unwrap();
    assert_eq!(open.page_count().unwrap(), 2);
    // Con la contraseña de propietario se abre con todos los permisos; con la de usuario, solo los concedidos.
    assert!(open.permissions().contains(Permission::PRINT));
    assert!(!open.metadata(MetadataName::Encryption).unwrap().is_empty());
}

#[test]
fn page_operations_and_labels() {
    let mut doc = reopen(&sample(), &WriteOptions::new());
    doc.duplicate_page(0).unwrap();
    assert_eq!(doc.page_count().unwrap(), 3);
    doc.move_page(2, 0).unwrap();
    doc.delete_pages(PageSelection::Pages(vec![1])).unwrap();
    assert_eq!(doc.page_count().unwrap(), 2);
    assert!(doc.delete_pages(PageSelection::Pages(vec![9])).is_err());

    let src = reopen(&sample(), &WriteOptions::new());
    let r = doc.insert_pdf(&src, InsertPdfOptions::default()).unwrap();
    assert_eq!(r.inserted_pages, PageRange::new(2, 4));
    assert_eq!(doc.page_count().unwrap(), 4);

    let mut p = doc.load_pdf_page(0).unwrap();
    p.set_rotation(90).unwrap();
    assert_eq!(p.rotation().unwrap(), 90);
    let b = p.bounds().unwrap();
    assert!(b.width() > b.height(), "girada: {b:?}");
}

#[test]
fn embedded_files_roundtrip() {
    let mut doc = sample();
    let info = doc
        .add_embedded_file("nota.txt", b"contenido", EmbeddedFileOptions::new("nota.txt"))
        .unwrap();
    assert_eq!(info.size, 9);
    let saved = reopen(&doc, &WriteOptions::new());
    let list = saved.embedded_files().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(saved.load_embedded_file("nota.txt").unwrap().unwrap(), b"contenido");
    let mut saved = saved;
    assert!(saved.delete_embedded_file("nota.txt").unwrap());
    assert!(saved.embedded_files().unwrap().is_empty());
}

#[test]
fn layers_toggle() {
    let mut doc = sample();
    let x = doc.add_optional_content_group("Notas").unwrap();
    let saved_groups = doc.optional_content_groups().unwrap();
    assert_eq!(saved_groups.len(), 1);
    assert_eq!(saved_groups[0].name.as_deref(), Some("Notas"));
    assert!(saved_groups[0].enabled);
    doc.set_optional_content_enabled(x, false).unwrap();
    assert!(!doc.optional_content_enabled(x).unwrap());
}

#[test]
fn annotations_create_read_modify_delete() {
    let doc = reopen(&sample(), &WriteOptions::new());
    let mut page = doc.load_pdf_page(0).unwrap();
    let hits = page.search("Hola", 4).unwrap();
    let mut hl = page.add_highlight_annotation(&hits).unwrap();
    hl.set_color(AnnotationColor::Rgb {
        red: 1.0,
        green: 1.0,
        blue: 0.0,
    })
    .unwrap();
    hl.set_contents("importante").unwrap();
    hl.update().unwrap();
    let mut sq = page
        .add_square_annotation(Rect::new(100.0, 100.0, 200.0, 160.0))
        .unwrap();
    sq.set_border_width(2.0).unwrap();
    sq.set_opacity(0.5).unwrap();
    let ln = page
        .add_line_annotation(Point::new(10.0, 10.0), Point::new(90.0, 40.0))
        .unwrap();
    assert_eq!(ln.line().unwrap().1, Point::new(90.0, 40.0));

    let all: Vec<_> = page.annotations().collect();
    assert_eq!(all.len(), 3);
    let types: Vec<_> = all.iter().map(|a| a.r#type().unwrap()).collect();
    assert!(types.contains(&PdfAnnotationType::Highlight) && types.contains(&PdfAnnotationType::Line));
    let h = all
        .iter()
        .find(|a| a.r#type().unwrap() == PdfAnnotationType::Highlight)
        .unwrap();
    assert_eq!(h.contents().unwrap().as_deref(), Some("importante"));
    assert_eq!(h.quad_points().unwrap().len(), 1);
    assert!(matches!(h.color().unwrap(), Some(AnnotationColor::Rgb { red, .. }) if red > 0.9));

    let xref = all[0].xref().unwrap();
    let victim = page.annotations().find(|a| a.xref().unwrap() == xref).unwrap();
    page.delete_annotation(victim).unwrap();
    assert_eq!(page.annotations().count(), 2);

    // Se guardan y vuelven a leerse.
    drop(all);
    drop(page);
    let saved = reopen(&doc, &WriteOptions::new());
    assert_eq!(saved.load_pdf_page(0).unwrap().annotations().count(), 2);
}

#[test]
fn links_add_and_read() {
    let mut doc = reopen(&sample(), &WriteOptions::new());
    let mut page = doc.load_pdf_page(0).unwrap();
    page.add_links(
        &mut doc,
        &[
            NewLink {
                bounds: Rect::new(10.0, 10.0, 100.0, 30.0),
                target: LinkTarget::Uri("https://pdfpliego.com".into()),
            },
            NewLink {
                bounds: Rect::new(10.0, 40.0, 100.0, 60.0),
                target: LinkTarget::Page {
                    page: 1,
                    kind: DestinationKind::Fit,
                },
            },
        ],
    )
    .unwrap();
    let saved = reopen(&doc, &WriteOptions::new());
    let links: Vec<_> = saved.load_page(0).unwrap().links().unwrap().collect();
    assert_eq!(links.len(), 2);
    assert!(links.iter().any(|l| l.uri.starts_with("https://pdfpliego.com")));
    assert!(links.iter().any(|l| l.dest.map(|d| d.page) == Some(1)));
}

#[test]
fn images_insert_list_extract() {
    let mut doc = reopen(&sample(), &WriteOptions::new());
    // PNG 2x2 válido, generado con la propia capa.
    let mut px = Pixmap::new_with_w_h(Colorspace::Rgb, 2, 2, false).unwrap();
    px.samples_mut()
        .copy_from_slice(&[255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 0]);
    let png = px.encode_png().unwrap();
    let mut page = doc.load_pdf_page(0).unwrap();
    let placed = page
        .insert_image(
            &mut doc,
            Rect::new(100.0, 100.0, 200.0, 200.0),
            ImageSource::Bytes {
                data: &png,
                format_hint: None,
            },
            InsertImageOptions::default(),
        )
        .unwrap();
    assert!(placed.xref > 0);
    let infos = page.images().unwrap();
    assert_eq!(infos.len(), 1);
    assert_eq!((infos[0].width, infos[0].height), (2, 2));
    let pl = page.image_placements().unwrap();
    assert_eq!(pl.len(), 1);
    assert!((pl[0].1.a - 100.0).abs() < 0.01, "{:?}", pl[0].1);
    assert_eq!(doc.extract_image(infos[0].xref).unwrap().width, 2);
    let img = Image::from_bytes(&png).unwrap();
    assert_eq!((img.width(), img.height()), (2, 2));
    assert_eq!(img.to_pixmap().unwrap().width(), 2);
    assert!(page.delete_image(&mut doc, infos[0].xref).unwrap());
    assert!(page.images().unwrap().is_empty());
}

#[test]
fn redaction_removes_text() {
    let doc = reopen(&sample(), &WriteOptions::new());
    let mut page = doc.load_pdf_page(0).unwrap();
    let hit = page.search("Hola", 1).unwrap();
    page.add_redact_annotation_quads(&hit).unwrap();
    assert!(page.redact(RedactOptions::default()).unwrap());
    drop(page);
    let saved = reopen(&doc, &WriteOptions::new());
    let text = saved.load_page(0).unwrap().text().unwrap();
    assert!(!text.contains("Hola") && text.contains("mundo"), "{text:?}");
}

#[test]
fn font_metrics_and_outline() {
    struct Count(usize);
    impl PathWalker for Count {
        fn move_to(&mut self, _: f32, _: f32) {
            self.0 += 1;
        }
        fn line_to(&mut self, _: f32, _: f32) {
            self.0 += 1;
        }
        fn curve_to(&mut self, _: f32, _: f32, _: f32, _: f32, _: f32, _: f32) {
            self.0 += 1;
        }
        fn close(&mut self) {}
    }
    let f = Font::new("Times-Roman").unwrap();
    assert!(f.is_serif() && !f.is_bold());
    let g = f.encode_character('A' as i32).unwrap();
    assert!(g > 0);
    let w = f.advance_glyph(g).unwrap();
    assert!(w > 0.5 && w < 1.0);
    let mut c = Count(0);
    assert!(f.outline_glyph(g, &Matrix::IDENTITY, &mut c).unwrap());
    assert!(c.0 > 4);
    assert!(f.ascender() > 0.5);
}

#[test]
fn formats_other_than_pdf() {
    let doc = Document::from_bytes(b"<html><body><p>Hola</p></body></html>", "html").unwrap();
    assert!(!doc.is_pdf());
    assert!(doc.into_pdf().is_err());
    assert!(Document::open("/no/existe.pdf").is_err());
}

#[test]
fn pdf_writer_makes_a_pdf() {
    let src = reopen(&sample(), &WriteOptions::new());
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("salida.pdf");
    {
        let mut w = PdfWriter::new(&out, "").unwrap();
        let page = src.load_page(0).unwrap();
        w.write_page(Rect::new(0.0, 0.0, 595.0, 842.0), &page, &Matrix::IDENTITY)
            .unwrap();
    }
    let doc = PdfDocument::open(&out).unwrap();
    assert_eq!(doc.page_count().unwrap(), 1);
    assert!(doc.load_page(0).unwrap().text().unwrap().contains("Hola"));
}

#[test]
fn open_with_password_from_disk_and_save_to_path() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("a.pdf");
    sample().save(&p).unwrap();
    let doc = PdfDocument::open_with_password(&p, None).unwrap();
    assert_eq!(doc.page_count().unwrap(), 2);
    let mut o = WriteOptions::new();
    o.set_compress(true);
    let q = dir.path().join("b.pdf");
    doc.save_with_options(&q, &o).unwrap();
    assert!(q.metadata().unwrap().len() > 0);
    let g = Document::open_with_password(&q, None).unwrap();
    assert!(g.is_pdf());
    assert!(g.into_pdf().is_ok());
}

#[test]
fn page_labels() {
    let mut doc = reopen(&sample(), &WriteOptions::new());
    doc.duplicate_page(0).unwrap();
    doc.duplicate_page(0).unwrap();
    doc.set_page_label_rule(PageLabelRule::new(0, PageLabelStyle::LowerRoman))
        .unwrap();
    let mut decimal = PageLabelRule::new(2, PageLabelStyle::Decimal);
    decimal.prefix = "A-".into();
    doc.set_page_label_rule(decimal).unwrap();
    let labels: Vec<_> = (0..4).map(|i| doc.page_label(i).unwrap()).collect();
    assert_eq!(labels, ["i", "ii", "A-1", "A-2"]);
}
