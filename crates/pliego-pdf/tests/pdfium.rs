//! Pruebas de dibujo, texto y búsqueda sobre PDFium (W37, W41). Necesitan la biblioteca de PDFium:
//! `PLIEGO_PDFIUM_DLL=<ruta absoluta a pdfium.dll / libpdfium.so>`. Sin ella se omiten con un aviso, para que
//! `cargo test --workspace` siga en verde en máquinas sin la DLL (la CI la descarga; véase `ci.yml`).

use std::path::PathBuf;

use pliego_pdf::{
    Colorspace, Document, Encryption, Matrix, PdfDocument, TextFlags, WriteOptions, pdfium_available,
    pdfium_init_from_environment,
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

fn open(name: &str) -> Document {
    Document::open(corpus(name)).unwrap()
}

fn norm(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn simple_pages_have_text() {
    if !pdfium() {
        return;
    }
    for name in [
        "basic/hola.pdf",
        "compare/base.pdf",
        "edit/corpus/dos-columnas.pdf",
        "edit/corpus/girada-90.pdf",
    ] {
        if !corpus(name).exists() {
            continue;
        }
        let t = open(name).load_page(0).unwrap().text().unwrap();
        assert!(!norm(&t).is_empty(), "{name}: sin texto");
    }
    let t = open("basic/hola.pdf").load_page(0).unwrap().text().unwrap();
    assert!(norm(&t).contains("Hola Pliego"), "{t:?}");
}

#[test]
fn pixmap_size_follows_bounds_and_scale() {
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
        let p = open(name).load_page(0).unwrap();
        let r = p.bounds().unwrap();
        for s in [1.0_f32, 2.0, 150.0 / 72.0] {
            let pix = p
                .to_pixmap(&Matrix::new_scale(s, s), Colorspace::Rgb, false, true)
                .unwrap();
            let (ew, eh) = (r.width() * s, r.height() * s);
            assert!(
                (pix.width() as f32 - ew).abs() <= 1.0 && (pix.height() as f32 - eh).abs() <= 1.0,
                "{name} @{s}: {}x{} frente a {ew}x{eh}",
                pix.width(),
                pix.height()
            );
        }
    }
}

/// Las tintas de los giros de página caen donde dice el texto: la caja de la primera palabra encontrada contiene
/// píxeles oscuros en el dibujo.
#[test]
fn ink_is_where_the_text_boxes_say() {
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
        if !corpus(name).exists() {
            continue;
        }
        let p = open(name).load_page(0).unwrap();
        let m = Matrix::new_scale(2.0, 2.0);
        let pix = p.to_pixmap(&m, Colorspace::Rgb, false, true).unwrap();
        let Some(w) = p.words().unwrap().into_iter().next() else {
            continue;
        };
        let (x0, y0) = (
            (w.bounds.x0 * 2.0).floor().max(0.0) as usize,
            (w.bounds.y0 * 2.0).floor().max(0.0) as usize,
        );
        let (x1, y1) = (
            ((w.bounds.x1 * 2.0).ceil() as usize).min(pix.width() as usize),
            ((w.bounds.y1 * 2.0).ceil() as usize).min(pix.height() as usize),
        );
        let n = pix.n() as usize;
        let dark = (y0..y1)
            .flat_map(|y| (x0..x1).map(move |x| (x, y)))
            .filter(|&(x, y)| pix.samples()[(y * pix.width() as usize + x) * n] < 128)
            .count();
        assert!(
            dark > 5,
            "{name}: la palabra «{}» no tiene tinta en {:?}",
            w.text,
            w.bounds
        );
    }
}

#[test]
fn region_matches_full_render() {
    if !pdfium() {
        return;
    }
    let b = open("basic/hola.pdf").load_page(0).unwrap();
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
fn search_finds_every_word_inside_its_own_box() {
    if !pdfium() {
        return;
    }
    let p = open("basic/hola.pdf").load_page(0).unwrap();
    let words = p.words().unwrap();
    assert!(!words.is_empty());
    for w in words.iter().take(5) {
        let hits = p.search(&w.text, 20).unwrap();
        assert!(!hits.is_empty(), "«{}» no se encuentra", w.text);
        let near = hits.iter().any(|q| {
            let r = pliego_pdf::Rect::from(*q);
            (r.x0 - w.bounds.x0).abs() < 3.0
                && (r.x1 - w.bounds.x1).abs() < 3.0
                && ((r.y0 + r.y1) - (w.bounds.y0 + w.bounds.y1)).abs() < 16.0
        });
        assert!(near, "«{}»: {:?} lejos de {:?}", w.text, hits, w.bounds);
    }
}

#[test]
fn structured_text_has_lines_chars_fonts_and_words() {
    if !pdfium() {
        return;
    }
    let p = open("basic/hola.pdf").load_page(0).unwrap();
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
    let b = open("basic/hola.pdf").load_page(0).unwrap();
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
    // Una ruta relativa se rechaza (nunca se busca en PATH ni en el directorio actual).
    // Si ya hay otra biblioteca cargada por otra prueba, `init` es idempotente y no comprueba la ruta.
    if !pdfium_available() {
        assert!(err.is_err());
    }
}

#[test]
fn encrypted_documents_open_with_the_password() {
    if !pdfium() {
        return;
    }
    let plain = PdfDocument::open(corpus("basic/hola.pdf")).unwrap();
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
    let doc = Document::from_bytes_with_password(&bytes, "application/pdf", Some("secreto")).unwrap();
    let page = doc.load_page(0).unwrap();
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
    let d = PdfDocument::from_bytes(&bytes).unwrap();
    let p = d.load_pdf_page(0).unwrap();
    assert!(p.text().unwrap().contains("Hola Pliego"));
    // `into_pdf` conserva el contenido.
    let doc = Document::from_bytes(&bytes, "application/pdf").unwrap();
    let pdf = doc.into_pdf().unwrap();
    assert!(pdf.load_page(1).unwrap().text().is_ok());
    // Una página fuera de rango da error, no pánico.
    assert!(pdf.load_page(9).is_err());
}

#[test]
fn text_with_images_requested_adds_image_blocks() {
    if !pdfium() {
        return;
    }
    let p = open("edit/corpus/con-imagen-y-vectores.pdf").load_page(0).unwrap();
    let tp = p
        .to_text_page(TextFlags::PRESERVE_IMAGES | TextFlags::PRESERVE_LIGATURES)
        .unwrap();
    assert!(
        tp.blocks()
            .iter()
            .any(|b| matches!(b.kind, pliego_pdf::TextBlockKind::Image)),
        "con PRESERVE_IMAGES el texto trae los bloques de imagen"
    );
}

#[test]
fn user_unit_does_not_change_the_page_size() {
    if !pdfium() {
        return;
    }
    // Ni el modelo propio ni PDFium aplican `/UserUnit` al tamaño de la página (MuPDF lo multiplicaba): el tamaño
    // y el dibujo coinciden.
    let doc = PdfDocument::open(corpus("basic/hola.pdf")).unwrap();
    let before = doc.load_page(0).unwrap().bounds().unwrap();
    let mut page_obj = doc.find_page(0).unwrap();
    page_obj.dict_put_real("UserUnit", 3.0).unwrap();
    let bytes = doc.to_bytes(&WriteOptions::default()).unwrap();

    let d = PdfDocument::from_bytes(&bytes).unwrap();
    let p = d.load_page(0).unwrap();
    let r = p.bounds().unwrap();
    assert!(
        (r.x1 - before.x1).abs() < 0.1 && (r.y1 - before.y1).abs() < 0.1,
        "{r:?} frente a {before:?}"
    );
    let pix = p
        .to_pixmap(&Matrix::new_scale(0.5, 0.5), Colorspace::Rgb, false, true)
        .unwrap();
    assert!(((pix.width() as f32) - r.x1 * 0.5).abs() <= 1.0);
    assert!(pix.samples().iter().filter(|&&v| v < 100).count() > 100);
    let w = p.words().unwrap().into_iter().next().unwrap();
    assert!(w.bounds.x1 < r.x1, "{:?}", w.bounds);
}
