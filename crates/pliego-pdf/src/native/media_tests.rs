//! Pruebas de fuentes e imágenes nativas. PDFium abre y dibuja lo que escribe `native`.

use super::PdfDocument as Native;
use super::tests::qpdf_check;
use super::{Font, Image, ImageSource, InsertImageOptions};
use crate::{Colorspace, Document, Matrix, Rect, Size, WriteOptions};

fn png_rgba(w: u32, h: u32, px: [u8; 4]) -> Vec<u8> {
    let mut out = Vec::new();
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(w, h, image::Rgba(px)))
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .unwrap();
    out
}

fn jpeg_rgb(w: u32, h: u32, px: [u8; 3]) -> Vec<u8> {
    let mut out = Vec::new();
    image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(w, h, image::Rgb(px)))
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Jpeg)
        .unwrap();
    out
}

struct Shot {
    px: Vec<u8>,
    stride: usize,
    height: u32,
}

impl Shot {
    fn new(bytes: &[u8]) -> Shot {
        let doc = Document::from_bytes(bytes, "application/pdf").unwrap();
        let page = doc.load_page(0).unwrap();
        let pix = page
            .to_pixmap(&Matrix::IDENTITY, Colorspace::device_rgb(), false, true)
            .unwrap();
        Shot {
            px: pix.samples().to_vec(),
            stride: pix.stride(),
            height: pix.height(),
        }
    }
    /// Color en el punto (x, y) del espacio de usuario (y hacia arriba) de una página sin giro.
    fn at(&self, x: u32, y: u32) -> (u8, u8, u8) {
        let row = (self.height - 1 - y) as usize;
        let i = row * self.stride + x as usize * 3;
        (self.px[i], self.px[i + 1], self.px[i + 2])
    }
    /// Píxeles oscuros dentro de un rectángulo del espacio de usuario.
    fn ink(&self, r: Rect) -> usize {
        let mut n = 0;
        for y in r.y0 as u32..r.y1 as u32 {
            for x in r.x0 as u32..r.x1 as u32 {
                let (a, b, c) = self.at(x, y);
                if u32::from(a) + u32::from(b) + u32::from(c) < 300 {
                    n += 1;
                }
            }
        }
        n
    }
}

fn close(a: (u8, u8, u8), b: (u8, u8, u8), tol: i32) -> bool {
    [(a.0, b.0), (a.1, b.1), (a.2, b.2)]
        .iter()
        .all(|(p, q)| (i32::from(*p) - i32::from(*q)).abs() <= tol)
}

fn save(doc: &Native) -> Vec<u8> {
    let bytes = doc.to_bytes(&WriteOptions::default()).unwrap();
    if let Some((code, text)) = qpdf_check(&bytes, None) {
        assert_eq!(code, 0, "qpdf --check:\n{text}");
    }
    bytes
}

#[test]
fn fonts_write_text_that_pdfium_reads_and_draws() {
    let mut doc = Native::new();
    let mut page = doc
        .new_page(Size {
            width: 300.0,
            height: 200.0,
        })
        .unwrap();
    let (helv, _) = page.insert_font(&mut doc, "Helvetica").unwrap();

    // Fuente simple incrustada (TrueType, WinAnsi): «Año» con la ñ en el byte 0xF1.
    let serif = Font::from_bytes(
        "Serif Cursiva",
        include_bytes!("../../fonts/liberation/LiberationSerif-Italic.ttf"),
    )
    .unwrap();
    let simple = doc.add_simple_font(&serif, crate::SimpleFontEncoding::Latin).unwrap();
    // Fuente Unicode (Type0): «Ñandú» por números de glifo.
    let uni = doc.add_font(&Font::new("Courier-Bold").unwrap()).unwrap();
    let gids: String = "Ñandú"
        .chars()
        .map(|c| {
            format!(
                "{:04X}",
                Font::new("Courier-Bold").unwrap().encode_character(c as i32).unwrap()
            )
        })
        .collect();

    let mut res = page.resources().unwrap();
    let mut fonts = res.get_dict("Font").unwrap().unwrap();
    fonts.dict_put("FS", simple).unwrap();
    fonts.dict_put("FU", uni).unwrap();
    let content = format!(
        "BT {helv} 24 Tf 20 150 Td (Hola mundo) Tj ET\n\
         BT /FS 24 Tf 20 100 Td <41F16F> Tj ET\n\
         BT /FU 24 Tf 20 50 Td <{gids}> Tj ET\n"
    );
    page.insert_contents(&mut doc, content.as_bytes(), true).unwrap();
    let _ = &mut res;

    let bytes = save(&doc);
    let shot = Shot::new(&bytes);
    for (label, y) in [("Helvetica", 150.0), ("Serif incrustada", 100.0), ("Type0", 50.0)] {
        let ink = shot.ink(Rect::new(15.0, y - 6.0, 200.0, y + 24.0));
        assert!(ink > 60, "{label} no dibuja texto ({ink} píxeles)");
    }
    let text = Document::from_bytes(&bytes, "application/pdf")
        .unwrap()
        .load_page(0)
        .unwrap()
        .text()
        .unwrap();
    assert!(text.contains("Hola mundo"), "{text:?}");
    assert!(text.contains("Año"), "{text:?}");
    assert!(text.contains("Ñandú"), "{text:?}");
}

#[test]
fn inserted_images_draw_with_alpha_and_opacity() {
    let mut doc = Native::new();
    let mut page = doc
        .new_page(Size {
            width: 200.0,
            height: 200.0,
        })
        .unwrap();
    let jpeg = jpeg_rgb(16, 16, [0, 128, 255]);
    let png = png_rgba(4, 4, [255, 0, 0, 128]);
    page.insert_image(
        &mut doc,
        Rect::new(10.0, 100.0, 90.0, 180.0),
        ImageSource::Bytes {
            data: &jpeg,
            format_hint: None,
        },
        InsertImageOptions::default(),
    )
    .unwrap();
    page.insert_image(
        &mut doc,
        Rect::new(110.0, 100.0, 190.0, 180.0),
        ImageSource::Image(&Image::from_bytes(&png).unwrap()),
        InsertImageOptions::default(),
    )
    .unwrap();
    page.insert_image(
        &mut doc,
        Rect::new(10.0, 10.0, 90.0, 90.0),
        ImageSource::Bytes {
            data: &jpeg,
            format_hint: None,
        },
        InsertImageOptions {
            opacity: Some(0.5),
            ..Default::default()
        },
    )
    .unwrap();

    let bytes = save(&doc);
    let shot = Shot::new(&bytes);
    assert!(
        close(shot.at(50, 140), (0, 128, 255), 6),
        "JPEG: {:?}",
        shot.at(50, 140)
    );
    // Rojo con alfa 128 sobre blanco.
    assert!(
        close(shot.at(150, 140), (255, 127, 127), 4),
        "PNG alfa: {:?}",
        shot.at(150, 140)
    );
    // JPEG al 50 %.
    assert!(
        close(shot.at(50, 50), (127, 191, 255), 6),
        "opacidad: {:?}",
        shot.at(50, 50)
    );
    assert!(close(shot.at(150, 50), (255, 255, 255), 0));
}

#[test]
fn replaced_images_survive_save_and_reopen() {
    let mut doc = Native::new();
    let mut page = doc
        .new_page(Size {
            width: 100.0,
            height: 100.0,
        })
        .unwrap();
    let placed = page
        .insert_image(
            &mut doc,
            Rect::new(0.0, 0.0, 100.0, 100.0),
            ImageSource::Bytes {
                data: &jpeg_rgb(8, 8, [255, 0, 0]),
                format_hint: None,
            },
            InsertImageOptions::default(),
        )
        .unwrap();
    page.replace_image(
        &mut doc,
        placed.xref,
        ImageSource::Bytes {
            data: &png_rgba(2, 2, [0, 0, 255, 255]),
            format_hint: None,
        },
    )
    .unwrap();
    let bytes = save(&doc);
    assert!(close(Shot::new(&bytes).at(50, 50), (0, 0, 255), 2));

    let reopened = Native::from_bytes(&bytes).unwrap();
    let page = reopened.load_pdf_page(0).unwrap();
    let imgs = page.images().unwrap();
    assert_eq!(imgs.len(), 1);
    assert_eq!((imgs[0].width, imgs[0].height), (2, 2));
}

// ---------------------------------------------------------------------------------------------------------------
// image_placements: el recorrido del contenido,
// ---------------------------------------------------------------------------------------------------------------

type Placements = Vec<(u32, u32, [f32; 6])>;

fn flat(p: Vec<(Image, Matrix)>) -> Placements {
    p.into_iter()
        .map(|(i, m)| (i.width(), i.height(), [m.a, m.b, m.c, m.d, m.e, m.f]))
        .collect()
}

#[test]
fn placements_are_found_on_plain_and_rotated_pages() {
    for rotate in [0, 90, 270] {
        let mut doc = Native::new();
        let mut page = doc
            .new_page(Size {
                width: 200.0,
                height: 300.0,
            })
            .unwrap();
        page.set_rotation(rotate).unwrap();
        for (rect, bytes) in [
            (Rect::new(10.0, 20.0, 110.0, 90.0), jpeg_rgb(16, 8, [10, 20, 30])),
            (Rect::new(50.0, 150.0, 70.0, 250.0), png_rgba(3, 5, [1, 2, 3, 255])),
        ] {
            page.insert_image(
                &mut doc,
                rect,
                ImageSource::Bytes {
                    data: &bytes,
                    format_hint: None,
                },
                InsertImageOptions::default(),
            )
            .unwrap();
        }
        let bytes = save(&doc);
        let ours = flat(
            Native::from_bytes(&bytes)
                .unwrap()
                .load_pdf_page(0)
                .unwrap()
                .image_placements()
                .unwrap(),
        );
        assert_eq!(ours.len(), 2);
        assert_eq!((ours[0].0, ours[0].1), (16, 8));
        assert_eq!((ours[1].0, ours[1].1), (3, 5));
        assert!(
            ours.iter()
                .all(|p| p.2.iter().all(|v| v.is_finite()) && p.2[0] * p.2[3] != p.2[1] * p.2[2])
        );
    }
}

#[test]
fn placements_walk_forms_inline_images_and_decode_pdf_images() {
    let mut doc = Native::new();
    let mut page = doc
        .new_page(Size {
            width: 200.0,
            height: 200.0,
        })
        .unwrap();

    // Imagen 2×2 de 4 bits indexada (paleta rojo/verde/azul/blanco) con Flate.
    let palette = doc
        .new_object_from_str("[/Indexed /DeviceRGB 3 <FF0000 00FF00 0000FF FFFFFF>]")
        .unwrap();
    let mut d = doc
        .new_object_from_str("<< /Type /XObject /Subtype /Image /Width 2 /Height 2 /BitsPerComponent 4 >>")
        .unwrap();
    d.dict_put("ColorSpace", palette).unwrap();
    // Fila 0: índices 0,1 → 0x01; fila 1: 2,3 → 0x23 (cada fila ocupa 1 byte).
    let indexed = doc.add_stream(&[0x01, 0x23], Some(&d), false).unwrap();
    // Imagen de 2×1 en DeviceGray de 8 bits con /Decode invertido.
    let d2 = doc
        .new_object_from_str(
            "<< /Type /XObject /Subtype /Image /Width 2 /Height 1 /BitsPerComponent 8 /ColorSpace /DeviceGray /Decode [1 0] >>",
        )
        .unwrap();
    let gray = doc.add_stream(&[0, 255], Some(&d2), false).unwrap();
    // Formulario que dibuja la imagen indexada con su propia matriz y recursos.
    let mut form = doc
        .new_object_from_str("<< /Type /XObject /Subtype /Form /BBox [0 0 100 100] /Matrix [2 0 0 2 5 5] >>")
        .unwrap();
    let mut xo = doc.new_dict().unwrap();
    xo.dict_put("Im", indexed.clone()).unwrap();
    let mut fres = doc.new_dict().unwrap();
    fres.dict_put("XObject", xo).unwrap();
    form.dict_put("Resources", fres).unwrap();
    let form = doc
        .add_stream(b"q 10 0 0 10 1 2 cm /Im Do Q", Some(&form), false)
        .unwrap();

    let mut res = page.resources().unwrap();
    let mut xo = doc.new_dict().unwrap();
    xo.dict_put("G", gray).unwrap();
    xo.dict_put("F", form).unwrap();
    res.dict_put("XObject", xo).unwrap();
    // Imagen en línea RGB de 1×1 y un formulario que se llama a sí mismo no debe colgar el recorrido.
    page.insert_contents(
        &mut doc,
        b"q 40 0 0 20 100 100 cm /G Do Q\nq 1 0 0 1 30 40 cm /F Do Q\nq 8 0 0 8 150 150 cm BI /W 1 /H 1 /CS /RGB /BPC 8 ID \xff\x00\x00 EI Q\n",
        true,
    )
    .unwrap();

    let bytes = save(&doc);
    let native = Native::from_bytes(&bytes).unwrap();
    let ours = flat(native.load_pdf_page(0).unwrap().image_placements().unwrap());
    assert_eq!(ours.len(), 3, "{ours:?}");
    // Los píxeles decodificados son los de la entrada.
    let mine = native.load_pdf_page(0).unwrap().image_placements().unwrap();
    for (img, _) in &mine {
        let px = img.to_pixmap().unwrap();
        assert_eq!((px.width(), px.height()), (img.width(), img.height()));
        assert!(px.samples().iter().any(|&b| b != 0), "{}x{}", img.width(), img.height());
    }
}

#[test]
fn self_referencing_forms_do_not_loop() {
    let mut doc = Native::new();
    let mut page = doc
        .new_page(Size {
            width: 100.0,
            height: 100.0,
        })
        .unwrap();
    let mut form = doc
        .new_object_from_str("<< /Type /XObject /Subtype /Form /BBox [0 0 100 100] >>")
        .unwrap();
    let form_ref = doc.add_stream(b"/F Do", Some(&form), false).unwrap();
    // El formulario se llama a sí mismo a través de sus propios recursos.
    let mut xo = doc.new_dict().unwrap();
    xo.dict_put("F", form_ref.clone()).unwrap();
    let mut fres = doc.new_dict().unwrap();
    fres.dict_put("XObject", xo).unwrap();
    let mut target = doc.new_indirect(form_ref.as_indirect().unwrap(), 0).unwrap();
    target.dict_put("Resources", fres).unwrap();
    let _ = &mut form;
    let mut xo = doc.new_dict().unwrap();
    xo.dict_put("F", form_ref).unwrap();
    page.resources().unwrap().dict_put("XObject", xo).unwrap();
    page.insert_contents(&mut doc, b"/F Do", true).unwrap();
    assert!(page.image_placements().unwrap().is_empty());
}

#[test]
fn corpus_placements_are_consistent() {
    let (mut files, mut pages_with_images) = (0, 0);
    for path in super::tests::corpus() {
        if path.ends_with("tiling-hang.pdf") {
            continue;
        }
        let bytes = std::fs::read(&path).unwrap();
        let Ok(ours) = Native::from_bytes(&bytes) else { continue };
        if ours.needs_password().unwrap_or(true) {
            continue;
        }
        files += 1;
        for p in 0..ours.page_count().unwrap().min(5) {
            let Ok(page) = ours.load_pdf_page(p) else { continue };
            let Ok(found) = page.image_placements() else { continue };
            let a = flat(found);
            pages_with_images += usize::from(!a.is_empty());
            for (w, h, m) in &a {
                assert!(
                    *w > 0 && *h > 0 && m.iter().all(|v| v.is_finite()),
                    "{} p{p}: {a:?}",
                    path.display()
                );
            }
        }
    }
    assert!(files > 3, "el corpus no se ha leído ({files} archivos)");
    eprintln!("corpus: {files} archivos, {pages_with_images} páginas con imágenes");
}
