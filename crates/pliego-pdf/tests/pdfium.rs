//! Pruebas del motor PDFium de `pliego-pdf` (W37). Solo se compilan con `--features pdfium` y necesitan la biblioteca
//! de PDFium: `PLIEGO_PDFIUM_DLL=<ruta absoluta a pdfium.dll / libpdfium.so>`. Sin ella se omiten con un aviso, para
//! que `cargo test --workspace` siga en verde en máquinas sin la DLL (la CI Linux la descarga; véase `ci.yml`).
#![cfg(feature = "pdfium")]

use std::path::PathBuf;

use pliego_pdf::{
    Colorspace, Document, Encryption, Matrix, PdfDocument, Pixmap, RenderBackend, TextFlags, WriteOptions,
    pdfium_available, pdfium_init_from_environment,
};

fn corpus(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus")
        .join(name)
}

/// `true` si PDFium está cargado; si no, la prueba se omite.
fn pdfium() -> bool {
    if pdfium_available() {
        return true;
    }
    match pdfium_init_from_environment() {
        Ok(_) => true,
        Err(e) => {
            eprintln!("OMITIDA: no hay PDFium ({e}); define PLIEGO_PDFIUM_DLL");
            false
        }
    }
}

fn open(name: &str, backend: RenderBackend) -> Document {
    let mut d = Document::open(corpus(name)).unwrap();
    d.set_render_backend(backend).unwrap();
    d
}

fn norm(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn text_matches_mupdf_on_simple_pages() {
    if !pdfium() {
        return;
    }
    for name in [
        "basic/hola.pdf",
        "compare/base.pdf",
        "edit/corpus/dos-columnas.pdf",
        "edit/corpus/girada-90.pdf",
    ] {
        let p = corpus(name);
        if !p.exists() {
            continue;
        }
        let a = open(name, RenderBackend::Mupdf);
        let b = open(name, RenderBackend::Pdfium);
        let (ta, tb) = (
            a.load_page(0).unwrap().text().unwrap(),
            b.load_page(0).unwrap().text().unwrap(),
        );
        assert_eq!(b.load_page(0).unwrap().render_backend(), RenderBackend::Pdfium);
        assert_eq!(norm(&ta), norm(&tb), "{name}");
    }
}

#[test]
fn bounds_and_pixmap_size_match_mupdf() {
    if !pdfium() {
        return;
    }
    for name in [
        "basic/hola.pdf",
        "edit/corpus/girada-90.pdf",
        "edit/corpus/girada-180.pdf",
        "edit/corpus/girada-270.pdf",
        "edit/corpus/cropbox.pdf",
        "edit/corpus/mediabox-desplazada.pdf",
    ] {
        let a = open(name, RenderBackend::Mupdf).load_page(0).unwrap();
        let b = open(name, RenderBackend::Pdfium).load_page(0).unwrap();
        let (ra, rb) = (a.bounds().unwrap(), b.bounds().unwrap());
        assert!(
            (ra.x1 - rb.x1).abs() < 0.01 && (ra.y1 - rb.y1).abs() < 0.01,
            "{name}: {ra:?} {rb:?}"
        );
        for s in [1.0_f32, 2.0, 150.0 / 72.0] {
            let m = Matrix::new_scale(s, s);
            let pa = a.to_pixmap(&m, Colorspace::Rgb, false, true).unwrap();
            let pb = b.to_pixmap(&m, Colorspace::Rgb, false, true).unwrap();
            assert_eq!((pa.width(), pa.height()), (pb.width(), pb.height()), "{name} @{s}");
        }
    }
}

/// Fracción de píxeles que difieren más de `tol` entre dos mapas del mismo tamaño, desplazando `b` (dx, dy).
fn diff(a: &Pixmap, b: &Pixmap, dx: i32, dy: i32, tol: u8) -> f64 {
    let (w, h) = (a.width() as i32, a.height() as i32);
    let n = a.n() as usize;
    let (mut bad, mut total) = (0u64, 0u64);
    for y in 2..h - 2 {
        for x in 2..w - 2 {
            let (bx, by) = (x + dx, y + dy);
            if bx < 0 || by < 0 || bx >= w || by >= h {
                continue;
            }
            let ia = (y as usize * w as usize + x as usize) * n;
            let ib = (by as usize * w as usize + bx as usize) * n;
            let d = (0..3)
                .map(|i| a.samples()[ia + i].abs_diff(b.samples()[ib + i]))
                .max()
                .unwrap();
            total += 1;
            if d > tol {
                bad += 1;
            }
        }
    }
    bad as f64 / total.max(1) as f64
}

#[test]
fn render_is_aligned_with_mupdf() {
    if !pdfium() {
        return;
    }
    // Si el dibujo estuviera desplazado medio píxel o mal escalado, el mejor desplazamiento no sería (0, 0).
    for name in [
        "basic/hola.pdf",
        "edit/corpus/girada-90.pdf",
        "edit/corpus/girada-270.pdf",
        "edit/corpus/cropbox.pdf",
        "edit/corpus/mediabox-desplazada.pdf",
        "edit/corpus/con-imagen-y-vectores.pdf",
    ] {
        let a = open(name, RenderBackend::Mupdf).load_page(0).unwrap();
        let b = open(name, RenderBackend::Pdfium).load_page(0).unwrap();
        let m = Matrix::new_scale(2.0, 2.0);
        let pa = a.to_pixmap(&m, Colorspace::Rgb, false, true).unwrap();
        let pb = b.to_pixmap(&m, Colorspace::Rgb, false, true).unwrap();
        let at_zero = diff(&pa, &pb, 0, 0, 40);
        for (dx, dy) in [(-2, 0), (2, 0), (0, -2), (0, 2), (-1, -1), (1, 1)] {
            let shifted = diff(&pa, &pb, dx, dy, 40);
            assert!(
                at_zero <= shifted + 1e-9,
                "{name}: desplazado ({dx},{dy}) coincide mejor ({shifted:.4}) que sin desplazar ({at_zero:.4})"
            );
        }
        assert!(at_zero < 0.03, "{name}: demasiada diferencia con MuPDF ({at_zero:.4})");
    }
}

#[test]
fn region_matches_full_render() {
    if !pdfium() {
        return;
    }
    let b = open("basic/hola.pdf", RenderBackend::Pdfium).load_page(0).unwrap();
    let m = Matrix::new_scale(2.0, 2.0);
    let full = b.to_pixmap(&m, Colorspace::Rgb, false, true).unwrap();
    let (x0, y0, x1, y1) = (40, 50, 200, 160);
    let part = b
        .render_region(&m, pliego_pdf::IRect::new(x0, y0, x1, y1), Colorspace::Rgb, true)
        .unwrap();
    assert_eq!((part.width() as i32, part.height() as i32), (x1 - x0, y1 - y0));
    let mut bad = 0;
    for y in 0..part.height() as usize {
        for x in 0..part.width() as usize {
            let ip = (y * part.width() as usize + x) * 3;
            let jf = ((y + y0 as usize) * full.width() as usize + x + x0 as usize) * 3;
            if (0..3).any(|i| part.samples()[ip + i].abs_diff(full.samples()[jf + i]) > 8) {
                bad += 1;
            }
        }
    }
    assert!(
        bad < 20,
        "la región difiere del recorte del render completo en {bad} píxeles"
    );
}

#[test]
fn search_finds_same_words_as_mupdf() {
    if !pdfium() {
        return;
    }
    let a = open("basic/hola.pdf", RenderBackend::Mupdf).load_page(0).unwrap();
    let b = open("basic/hola.pdf", RenderBackend::Pdfium).load_page(0).unwrap();
    let words: Vec<String> = a.words().unwrap().into_iter().map(|w| w.text).collect();
    assert!(!words.is_empty());
    for w in words.iter().take(5) {
        let (qa, qb) = (a.search(w, 20).unwrap(), b.search(w, 20).unwrap());
        assert_eq!(qa.len(), qb.len(), "«{w}»");
        for (x, y) in qa.iter().zip(&qb) {
            let (rx, ry) = (pliego_pdf::Rect::from(*x), pliego_pdf::Rect::from(*y));
            assert!(
                (rx.x0 - ry.x0).abs() < 3.0 && (rx.x1 - ry.x1).abs() < 3.0,
                "«{w}»: {rx:?} {ry:?}"
            );
            // MuPDF da la caja de línea (ascendente a descendente); PDFium, la del texto: se comparan los centros.
            let (cx, cy) = ((rx.y0 + rx.y1) / 2.0, (ry.y0 + ry.y1) / 2.0);
            assert!((cx - cy).abs() < 8.0, "«{w}»: {rx:?} {ry:?}");
        }
    }
}

#[test]
fn structured_text_has_lines_chars_fonts_and_words() {
    if !pdfium() {
        return;
    }
    let p = open("basic/hola.pdf", RenderBackend::Pdfium).load_page(0).unwrap();
    let tp = p.to_text_page(TextFlags::PRESERVE_LIGATURES).unwrap();
    let blocks = tp.blocks();
    assert!(!blocks.is_empty());
    let line = &blocks[0].lines[0];
    assert!(!line.text().is_empty());
    let c = line
        .chars
        .iter()
        .find(|c| c.ch.is_some_and(char::is_alphabetic))
        .unwrap();
    assert!(c.size > 4.0 && c.size < 100.0);
    assert!(c.font.as_ref().is_some_and(|f| !f.name.is_empty()));
    assert_eq!(c.argb >> 24, 0xFF);
    let words = tp.words();
    assert!(words.iter().all(|w| !w.text.is_empty() && !w.bounds.is_empty()));
}

#[test]
fn modified_document_is_rendered_from_memory_copy() {
    if !pdfium() {
        return;
    }
    // Un documento con cambios sin guardar se serializa y PDFium dibuja el cambio.
    let mut doc = PdfDocument::open(corpus("basic/hola.pdf")).unwrap();
    doc.set_render_backend(RenderBackend::Pdfium).unwrap();
    let before = doc
        .load_page(0)
        .unwrap()
        .to_pixmap(&Matrix::IDENTITY, Colorspace::Rgb, false, true)
        .unwrap();
    let size = doc.load_page(0).unwrap().bounds().unwrap();
    let mut page = doc.load_pdf_page(0).unwrap();
    let contents = b"q 1 0 0 rg 0 0 200 200 re f Q";
    page.insert_contents(&mut doc, contents, true).unwrap();
    let after = doc
        .load_page(0)
        .unwrap()
        .to_pixmap(&Matrix::IDENTITY, Colorspace::Rgb, false, true)
        .unwrap();
    assert_ne!(before.samples(), after.samples(), "el cambio en memoria debe verse");
    // Rojo abajo a la izquierda: píxel (10, alto-10).
    let (w, h) = (after.width() as usize, after.height() as usize);
    let i = ((h - 10) * w + 10) * 3;
    assert!(
        after.samples()[i] > 200 && after.samples()[i + 1] < 60,
        "esperaba rojo, hay {:?}",
        &after.samples()[i..i + 3]
    );
    assert_eq!((w as f32, h as f32), (size.x1.ceil(), size.y1.ceil()));
}

#[test]
fn pixmap_formats() {
    if !pdfium() {
        return;
    }
    let b = open("basic/hola.pdf", RenderBackend::Pdfium).load_page(0).unwrap();
    let m = Matrix::new_scale(1.0, 1.0);
    let rgb = b.to_pixmap(&m, Colorspace::Rgb, false, true).unwrap();
    let gray = b.to_pixmap(&m, Colorspace::Gray, false, true).unwrap();
    let rgba = b.to_pixmap(&m, Colorspace::Rgb, true, true).unwrap();
    assert_eq!(gray.n(), 1);
    assert_eq!(rgba.n(), 4);
    assert_eq!(rgb.samples().len(), rgb.width() as usize * rgb.height() as usize * 3);
    // Fondo transparente en la esquina; blanco opaco sin alfa.
    assert_eq!(rgba.samples()[3], 0);
    assert_eq!(&rgb.samples()[..3], &[255, 255, 255]);
}

#[test]
fn needs_absolute_path() {
    let err = pliego_pdf::pdfium_init(std::path::Path::new("pdfium.dll"));
    // Con la característica `pdfium` una ruta relativa se rechaza (nunca se busca en PATH ni en el directorio actual).
    // Si ya hay otra biblioteca cargada por otra prueba, `init` es idempotente y no comprueba la ruta.
    if !pdfium_available() {
        assert!(err.is_err());
    }
}

#[test]
fn encrypted_documents_open_with_the_password_checked_by_mupdf() {
    if !pdfium() {
        return;
    }
    let mut plain = PdfDocument::open(corpus("basic/hola.pdf")).unwrap();
    plain.set_render_backend(RenderBackend::Mupdf).unwrap();
    let mut opts = WriteOptions::default();
    opts.set_encryption(Encryption::Aes256)
        .set_user_password("secreto")
        .set_owner_password("dueno");
    let bytes = plain.to_bytes(&opts).unwrap();
    // Sin contraseña no se abre; con ella, PDFium dibuja y extrae el texto del documento cifrado.
    assert!(matches!(
        Document::from_bytes_with_password(&bytes, "application/pdf", None),
        Err(pliego_core::Error::Password)
    ));
    let mut doc = Document::from_bytes_with_password(&bytes, "application/pdf", Some("secreto")).unwrap();
    doc.set_render_backend(RenderBackend::Pdfium).unwrap();
    let page = doc.load_page(0).unwrap();
    assert_eq!(page.render_backend(), RenderBackend::Pdfium);
    assert!(page.text().unwrap().contains("Hola Pliego"));
    let pix = page.to_pixmap(&Matrix::IDENTITY, Colorspace::Rgb, false, true).unwrap();
    assert!(
        pix.samples().iter().any(|&v| v < 128),
        "la página cifrada debe dibujarse"
    );
}

#[test]
fn documents_from_bytes_and_pdf_documents_use_pdfium_too() {
    if !pdfium() {
        return;
    }
    let bytes = std::fs::read(corpus("basic/hola.pdf")).unwrap();
    let mut d = PdfDocument::from_bytes(&bytes).unwrap();
    d.set_render_backend(RenderBackend::Pdfium).unwrap();
    let p = d.load_pdf_page(0).unwrap();
    assert_eq!(p.render_backend(), RenderBackend::Pdfium);
    assert!(p.text().unwrap().contains("Hola Pliego"));
    // `into_pdf` conserva el motor y el origen.
    let mut doc = Document::from_bytes(&bytes, "application/pdf").unwrap();
    doc.set_render_backend(RenderBackend::Pdfium).unwrap();
    let pdf = doc.into_pdf().unwrap();
    assert_eq!(pdf.render_backend(), RenderBackend::Pdfium);
    assert_eq!(pdf.load_page(1).unwrap().render_backend(), RenderBackend::Pdfium);
    // Una página fuera de rango da error, no pánico.
    assert!(pdf.load_page(9).is_err());
}

#[test]
fn text_with_images_requested_uses_mupdf() {
    if !pdfium() {
        return;
    }
    let p = open("edit/corpus/con-imagen-y-vectores.pdf", RenderBackend::Pdfium)
        .load_page(0)
        .unwrap();
    let tp = p
        .to_text_page(TextFlags::PRESERVE_IMAGES | TextFlags::PRESERVE_LIGATURES)
        .unwrap();
    assert!(
        tp.blocks()
            .iter()
            .any(|b| matches!(b.kind, pliego_pdf::TextBlockKind::Image)),
        "con PRESERVE_IMAGES el texto viene de MuPDF y trae los bloques de imagen"
    );
}

#[test]
fn user_unit_scales_the_page_like_mupdf() {
    if !pdfium() {
        return;
    }
    // `/UserUnit` 3: MuPDF multiplica el tamaño de la página por 3, PDFium no; la capa los reconcilia.
    let doc = PdfDocument::open(corpus("basic/hola.pdf")).unwrap();
    let mut page_obj = doc.find_page(0).unwrap();
    page_obj.dict_put_real("UserUnit", 3.0).unwrap();
    let bytes = doc.to_bytes(&WriteOptions::default()).unwrap();

    let a = {
        let d = PdfDocument::from_bytes(&bytes).unwrap();
        d.load_page(0).unwrap()
    };
    let mut d = PdfDocument::from_bytes(&bytes).unwrap();
    d.set_render_backend(RenderBackend::Pdfium).unwrap();
    let b = d.load_page(0).unwrap();
    let (ra, rb) = (a.bounds().unwrap(), b.bounds().unwrap());
    assert!(ra.x1 > 1500.0, "MuPDF debe aplicar UserUnit: {ra:?}");
    assert!(
        (ra.x1 - rb.x1).abs() < 0.1 && (ra.y1 - rb.y1).abs() < 0.1,
        "{ra:?} {rb:?}"
    );
    let m = Matrix::new_scale(0.5, 0.5);
    let pa = a.to_pixmap(&m, Colorspace::Rgb, false, true).unwrap();
    let pb = b.to_pixmap(&m, Colorspace::Rgb, false, true).unwrap();
    assert_eq!((pa.width(), pa.height()), (pb.width(), pb.height()));
    let ink = |p: &Pixmap| p.samples().iter().filter(|&&v| v < 100).count();
    let (ia, ib) = (ink(&pa), ink(&pb));
    assert!(
        ia > 100 && ib > 100 && (ia as f64 / ib as f64 - 1.0).abs() < 0.35,
        "tinta {ia} / {ib}"
    );
}
