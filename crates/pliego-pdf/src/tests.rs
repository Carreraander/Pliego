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
    // El flujo debe ser lo bastante grande para que comprimirlo compense (un flujo diminuto se deja como está).
    let mut doc = PdfDocument::new();
    let font = Font::new("Helvetica").unwrap();
    let fobj = doc.add_simple_font(&font, SimpleFontEncoding::Latin).unwrap();
    for _ in 0..2 {
        let mut page = doc.new_page(Size::A4).unwrap();
        let mut res = page.resources().unwrap();
        let mut fonts = doc.new_dict().unwrap();
        fonts.dict_put("F1", fobj.try_clone().unwrap()).unwrap();
        res.dict_put("Font", fonts).unwrap();
        let stream = format!("BT /F1 24 Tf 72 700 Td (Hola mundo) Tj ET\n{}", "q Q\n".repeat(400));
        page.insert_contents(&mut doc, stream.as_bytes(), true).unwrap();
    }
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
    let mut doc = reopen(&sample(), &WriteOptions::new());
    let hit = doc.load_page(0).unwrap().search("Hola", 1).unwrap();
    assert!(!hit.is_empty());
    let (x0, y0) = (hit[0].ul.x.min(hit[0].ll.x), hit[0].ul.y.min(hit[0].ur.y));
    let (x1, y1) = (hit[0].ur.x.max(hit[0].lr.x), hit[0].ll.y.max(hit[0].lr.y));
    let report = doc
        .redact_areas(&[(0, Rect::new(x0, y0, x1, y1))], RedactOptions::default())
        .unwrap();
    assert_eq!(report.pages_changed, 1);
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
fn formats_other_than_pdf_are_rejected() {
    assert!(Document::from_bytes(b"<html><body><p>Hola</p></body></html>", "html").is_err());
    assert!(Document::from_bytes(b"<html><body><p>Hola</p></body></html>", "application/pdf").is_err());
    assert!(Document::open("/no/existe.pdf").is_err());
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

// --- Abrir solo para ver (PDF grandes) ---

/// Escribe `sample()` en una carpeta temporal y lo abre solo para ver.
fn view_only_sample(dir: &std::path::Path, opts: &WriteOptions) -> (std::path::PathBuf, PdfDocument) {
    let path = dir.join("v.pdf");
    std::fs::write(&path, sample().to_bytes(opts).unwrap()).unwrap();
    let doc = PdfDocument::open_for_viewing(&path, None).unwrap();
    (path, doc)
}

#[test]
fn view_only_reads_like_a_normal_open() {
    let dir = tempfile::tempdir().unwrap();
    let (path, v) = view_only_sample(dir.path(), &WriteOptions::new());
    let full = PdfDocument::open(&path).unwrap();
    assert_eq!(v.page_count().unwrap(), full.page_count().unwrap());
    for i in 0..2 {
        let (a, b) = (v.load_page(i).unwrap(), full.load_page(i).unwrap());
        assert_eq!(a.bounds().unwrap(), b.bounds().unwrap());
        assert_eq!(a.text().unwrap(), b.text().unwrap());
        let ctm = Matrix::new_scale(0.5, 0.5);
        let (pa, pb) = (
            a.to_pixmap(&ctm, Colorspace::Rgb, false, true).unwrap(),
            b.to_pixmap(&ctm, Colorspace::Rgb, false, true).unwrap(),
        );
        assert_eq!(pa.to_rgba(), pb.to_rgba());
    }
    assert_eq!(
        v.load_page(0).unwrap().search("mundo", 4).unwrap(),
        full.load_page(0).unwrap().search("mundo", 4).unwrap()
    );
}

#[test]
fn view_only_cannot_be_modified_or_saved() {
    let dir = tempfile::tempdir().unwrap();
    let (_, mut v) = view_only_sample(dir.path(), &WriteOptions::new());
    for r in [
        v.to_bytes(&WriteOptions::new()).map(drop),
        v.save(dir.path().join("x.pdf")),
        v.to_bytes(WriteOptions::new().set_incremental(true)).map(drop),
    ] {
        assert!(matches!(r, Err(pliego_core::Error::Unsupported(_))), "{r:?}");
    }
    assert!(!dir.path().join("x.pdf").exists());
    // Un cambio en el modelo no puede acabar en la vista: PDFium dejaría de leer el archivo.
    v.delete_page(0).unwrap();
    assert!(matches!(
        v.load_page(0).unwrap().text(),
        Err(pliego_core::Error::Unsupported(_))
    ));
}

#[test]
fn view_only_with_password() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("c.pdf");
    let mut o = WriteOptions::new();
    o.set_encryption(Encryption::Aes256)
        .set_user_password("abrir")
        .set_owner_password("dueño");
    std::fs::write(&path, sample().to_bytes(&o).unwrap()).unwrap();
    assert!(matches!(
        PdfDocument::open_for_viewing(&path, None),
        Err(pliego_core::Error::Password)
    ));
    assert!(matches!(
        PdfDocument::open_for_viewing(&path, Some("mal")),
        Err(pliego_core::Error::Password)
    ));
    let v = PdfDocument::open_for_viewing(&path, Some("abrir")).unwrap();
    assert_eq!(v.page_count().unwrap(), 2);
    assert!(v.load_page(1).unwrap().text().unwrap().contains("Segunda"));
}

#[test]
fn view_only_recycles_pdfium_without_changing_results() {
    // Un escaneado de 40 páginas con imágenes de ~1 MB lee más de RECYCLE_AFTER_BYTES: el documento de PDFium se
    // cierra y se vuelve a abrir al menos una vez durante el recorrido, y cada página sigue dando lo mismo.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("scan.pdf");
    scan_pdf(&path, 40, 1_000_000);
    let v = PdfDocument::open_for_viewing(&path, None).unwrap();
    let full = PdfDocument::open(&path).unwrap();
    assert_eq!(v.page_count().unwrap(), 40);
    for i in 0..40 {
        let ctm = Matrix::new_scale(0.1, 0.1);
        let a = v
            .load_page(i)
            .unwrap()
            .to_pixmap(&ctm, Colorspace::Rgb, false, true)
            .unwrap();
        let b = full
            .load_page(i)
            .unwrap()
            .to_pixmap(&ctm, Colorspace::Rgb, false, true)
            .unwrap();
        assert_eq!(a.to_rgba(), b.to_rgba(), "página {i}");
    }
}

/// PDF «escaneado»: una imagen DCT (JPEG gris de ruido) por página. Mínimo, sin depender de `pliego-qa`.
fn scan_pdf(path: &std::path::Path, pages: usize, image_bytes: usize) {
    let side = (image_bytes as f64).sqrt() as u16;
    let mut state = 0x5eedu64;
    let px: Vec<u8> = (0..usize::from(side) * usize::from(side))
        .map(|_| {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (state >> 56) as u8
        })
        .collect();
    let mut jpeg = Vec::new();
    jpeg_encoder::Encoder::new(&mut jpeg, 85)
        .encode(&px, side, side, jpeg_encoder::ColorType::Luma)
        .unwrap();
    assert!(jpeg.len() > image_bytes / 2, "la imagen de ruido debe pesar de verdad");
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    let mut obj = |out: &mut Vec<u8>, body: Vec<u8>| {
        offsets.push(out.len());
        out.extend(format!("{} 0 obj\n", offsets.len()).as_bytes());
        out.extend(body);
        out.extend(b"\nendobj\n");
    };
    obj(&mut out, b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
    let kids: Vec<String> = (0..pages).map(|p| format!("{} 0 R", 3 + 3 * p)).collect();
    obj(
        &mut out,
        format!("<< /Type /Pages /Count {pages} /Kids [{}] >>", kids.join(" ")).into_bytes(),
    );
    for p in 0..pages {
        let (image, content) = (4 + 3 * p, 5 + 3 * p);
        obj(
            &mut out,
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Contents {content} 0 R \
                 /Resources << /XObject << /Im0 {image} 0 R >> >> >>"
            )
            .into_bytes(),
        );
        let mut img = format!(
            "<< /Type /XObject /Subtype /Image /Width {side} /Height {side} /ColorSpace /DeviceGray \
             /BitsPerComponent 8 /Filter /DCTDecode /Length {} >>\nstream\n",
            jpeg.len()
        )
        .into_bytes();
        img.extend(&jpeg);
        img.extend(b"\nendstream");
        obj(&mut out, img);
        let c = b"q 200 0 0 200 0 0 cm /Im0 Do Q";
        let mut s = format!("<< /Length {} >>\nstream\n", c.len()).into_bytes();
        s.extend(c);
        s.extend(b"\nendstream");
        obj(&mut out, s);
    }
    let xref = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len() + 1).as_bytes());
    for o in &offsets {
        out.extend(format!("{o:010} 00000 n \n").as_bytes());
    }
    out.extend(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            offsets.len() + 1
        )
        .as_bytes(),
    );
    std::fs::write(path, out).unwrap();
}

fn set_page_mode(doc: &PdfDocument) {
    doc.catalog().unwrap().dict_put_name("PageMode", "UseOutlines").unwrap();
}

#[test]
fn lean_save_appends_only_the_change() {
    for object_streams in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("grande.pdf");
        if object_streams {
            // Referencias en un flujo (PDF 1.5): el añadido también las escribe así.
            let mut o = WriteOptions::new();
            o.set_object_streams(true);
            std::fs::write(&path, sample().to_bytes(&o).unwrap()).unwrap();
        } else {
            scan_pdf(&path, 3, 300_000);
        }
        let original = std::fs::read(&path).unwrap();
        let pages = PdfDocument::open(&path).unwrap().page_count().unwrap();
        let lean = PdfDocument::open_lean(&path, None).unwrap();
        set_page_mode(&lean);
        let out = dir.path().join("salida.pdf");
        lean.save_lean(&out).unwrap();

        let saved = std::fs::read(&out).unwrap();
        assert!(saved.starts_with(&original), "el original queda intacto al principio");
        assert!(
            saved.len() < original.len() + 3000,
            "solo se añade lo cambiado ({} bytes)",
            saved.len() - original.len()
        );
        let again = PdfDocument::open(&out).unwrap();
        assert_eq!(again.page_count().unwrap(), pages);
        assert_eq!(
            again.catalog().unwrap().dict_get_name("PageMode").unwrap().as_deref(),
            Some("UseOutlines")
        );
        // Las páginas se ven igual que en el original (los flujos grandes siguen ahí).
        let (a, b) = (PdfDocument::open(&path).unwrap(), again);
        let ctm = Matrix::new_scale(0.5, 0.5);
        for i in 0..pages {
            let px = |d: &PdfDocument| {
                d.load_page(i)
                    .unwrap()
                    .to_pixmap(&ctm, Colorspace::Rgb, false, true)
                    .unwrap()
                    .to_rgba()
            };
            assert_eq!(px(&a), px(&b), "página {i}");
        }
        // Y lo guardado se puede volver a editar de la misma forma.
        let second = PdfDocument::open_lean(&out, None).unwrap();
        second
            .catalog()
            .unwrap()
            .dict_put_name("PageLayout", "TwoColumnLeft")
            .unwrap();
        let out2 = dir.path().join("salida2.pdf");
        second.save_lean(&out2).unwrap();
        let again2 = PdfDocument::open(&out2).unwrap();
        assert_eq!(
            again2.catalog().unwrap().dict_get_name("PageMode").unwrap().as_deref(),
            Some("UseOutlines")
        );
        assert_eq!(
            again2
                .catalog()
                .unwrap()
                .dict_get_name("PageLayout")
                .unwrap()
                .as_deref(),
            Some("TwoColumnLeft")
        );
    }
}

#[test]
fn lean_save_refuses_changes_to_skipped_streams_and_leaves_no_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("grande.pdf");
    scan_pdf(&path, 2, 300_000);
    let lean = PdfDocument::open_lean(&path, None).unwrap();
    let res = lean.load_page(0).unwrap().resources().unwrap();
    let xobjects = res.get_dict("XObject").unwrap().unwrap();
    let mut image = xobjects.get_dict("Im0").unwrap().unwrap();
    image.dict_put_name("Foo", "Bar").unwrap();
    let out = dir.path().join("salida.pdf");
    let r = lean.save_lean(&out);
    assert!(matches!(r, Err(pliego_core::Error::Unsupported(_))), "{r:?}");
    assert!(!out.exists());
}

#[test]
fn lean_documents_cannot_be_serialized_whole_and_notice_a_changed_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("grande.pdf");
    scan_pdf(&path, 2, 100_000);
    let lean = PdfDocument::open_lean(&path, None).unwrap();
    assert!(matches!(
        lean.to_bytes(&WriteOptions::new()),
        Err(pliego_core::Error::Unsupported(_))
    ));
    set_page_mode(&lean);
    // Si otro programa cambia el archivo, no se guarda encima de una versión que no es la que se editó.
    let mut bytes = std::fs::read(&path).unwrap();
    bytes.extend(b"\n% otro programa\n");
    std::fs::write(&path, bytes).unwrap();
    let out = dir.path().join("salida.pdf");
    assert!(matches!(lean.save_lean(&out), Err(pliego_core::Error::Unsupported(_))));
    assert!(!out.exists());
    // Un documento normal no se guarda así.
    assert!(matches!(
        PdfDocument::open(&path).unwrap().save_lean(&out),
        Err(pliego_core::Error::Unsupported(_))
    ));
}

#[test]
fn lean_refuses_encrypted_documents() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("c.pdf");
    let mut o = WriteOptions::new();
    o.set_encryption(Encryption::Aes256)
        .set_user_password("abrir")
        .set_owner_password("duenio");
    std::fs::write(&path, sample().to_bytes(&o).unwrap()).unwrap();
    let r = PdfDocument::open_lean(&path, Some("abrir"));
    assert!(matches!(r, Err(pliego_core::Error::Unsupported(_))), "{:?}", r.err());
}

#[test]
fn view_only_and_lean_read_objects_stored_in_object_streams() {
    // Casi todos los PDF modernos guardan catálogo, páginas y anotaciones dentro de flujos de objetos.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("os.pdf");
    let mut o = WriteOptions::new();
    o.set_object_streams(true);
    std::fs::write(&path, sample().to_bytes(&o).unwrap()).unwrap();
    let full = PdfDocument::open(&path).unwrap();
    for doc in [
        PdfDocument::open_for_viewing(&path, None).unwrap(),
        PdfDocument::open_lean(&path, None).unwrap(),
    ] {
        assert_eq!(doc.page_count().unwrap(), 2);
        assert!(doc.catalog().unwrap().dict_get_name("Type").unwrap().is_some());
        for i in 0..2 {
            assert_eq!(
                doc.load_page(i).unwrap().text().unwrap(),
                full.load_page(i).unwrap().text().unwrap()
            );
            assert_eq!(
                doc.load_page(i).unwrap().bounds().unwrap(),
                full.load_page(i).unwrap().bounds().unwrap()
            );
        }
    }
}
