//! Pruebas de fuentes e imágenes nativas. MuPDF es el motor de referencia: abre y dibuja lo que escribe `native`.

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
fn fonts_write_text_that_mupdf_reads_and_draws() {
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

    // La misma entrada dibuja igual con el motor de la raíz (MuPDF): paridad de píxeles.
    let mut reference = crate::PdfDocument::new();
    let mut rpage = reference
        .new_page(Size {
            width: 200.0,
            height: 200.0,
        })
        .unwrap();
    rpage
        .insert_image(
            &mut reference,
            Rect::new(110.0, 100.0, 190.0, 180.0),
            crate::ImageSource::Bytes {
                data: &png,
                format_hint: None,
            },
            crate::InsertImageOptions::default(),
        )
        .unwrap();
    let mut buf = Vec::new();
    reference.write_to(&mut buf).unwrap();
    let theirs = Shot::new(&buf);
    assert!(
        close(theirs.at(150, 140), shot.at(150, 140), 3),
        "{:?} vs {:?}",
        theirs.at(150, 140),
        shot.at(150, 140)
    );
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
