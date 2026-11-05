use std::path::Path;

use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use pliego_convert::{
    Block, ExtractImagesOptions, ImagesToPdfOptions, extract_images, images_to_pdf, pdf_to_html, pdf_to_svg,
    structured_text_pages,
};
use pliego_core::{JobContext, PageRange};

fn corpus() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/basic/hola.pdf")
}

fn sample(w: u32, h: u32) -> DynamicImage {
    DynamicImage::ImageRgb8(RgbImage::from_fn(w, h, |x, y| Rgb([x as u8, y as u8, 99])))
}

#[test]
fn structured_text_has_fonts_sizes_and_boxes() {
    let pages = structured_text_pages(&corpus(), None, &PageRange::all()).unwrap();
    assert_eq!(pages.len(), 2);
    let p = &pages[0];
    assert_eq!((p.width.round(), p.height.round()), (595.0, 842.0));
    let Block::Text { lines, bbox } = &p.blocks[0] else {
        panic!("se esperaba texto")
    };
    assert!(bbox.x1 > bbox.x0 && bbox.y1 > bbox.y0);
    let line = &lines[0];
    assert_eq!(line.text(), "Hola Pliego pagina uno");
    let span = &line.spans[0];
    assert!(span.font.contains("Helvetica"), "fuente: {}", span.font);
    assert!((span.size - 24.0).abs() < 0.01);
    assert_eq!(span.color, "#000000");
    // Serializable
    let json = serde_json::to_string(p).unwrap();
    assert!(json.contains("\"kind\":\"text\""));
}

#[test]
fn html_and_svg_export() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = JobContext::silent();
    let html = dir.path().join("o.html");
    pdf_to_html(&corpus(), None, &PageRange::all(), &html, &ctx).unwrap();
    let s = std::fs::read_to_string(&html).unwrap();
    assert!(s.starts_with("<!DOCTYPE html>"));
    assert!(s.contains("Hola") && s.contains("Segunda"));
    let r = pdf_to_svg(&corpus(), None, &"1".parse().unwrap(), dir.path(), &ctx).unwrap();
    let svg = std::fs::read_to_string(&r.outputs[0]).unwrap();
    assert!(
        svg.contains("<svg") && svg.contains("data:image/png;base64,iVBORw0KGgo"),
        "{}",
        &svg[..svg.len().min(300)]
    );
    // Capa de texto invisible y seleccionable.
    assert!(
        svg.contains("fill-opacity=\"0\"") && svg.contains(">Hola"),
        "falta la capa de texto"
    );
    // El XML está bien formado: toda etiqueta de apertura de `text` se cierra y no hay `&` sueltos.
    assert_eq!(svg.matches("<text ").count(), svg.matches("</text>").count());
    assert!(
        !svg.replace("&amp;", "")
            .replace("&lt;", "")
            .replace("&gt;", "")
            .replace("&quot;", "")
            .contains('&')
    );
}

#[test]
fn svg_embeds_a_decodable_png() {
    // El PNG incrustado (base64) debe poder decodificarse.
    let dir = tempfile::tempdir().unwrap();
    let r = pdf_to_svg(
        &corpus(),
        None,
        &"1".parse().unwrap(),
        dir.path(),
        &JobContext::silent(),
    )
    .unwrap();
    let svg = std::fs::read_to_string(&r.outputs[0]).unwrap();
    let start = svg.find("base64,").unwrap() + 7;
    let end = start + svg[start..].find('"').unwrap();
    let b64 = &svg[start..end];
    assert_eq!(b64.len() % 4, 0);
    let png = decode_b64(b64);
    let img = image::load_from_memory_with_format(&png, image::ImageFormat::Png).unwrap();
    assert!(img.width() > 100 && img.height() > 100);
}

fn decode_b64(s: &str) -> Vec<u8> {
    let val = |c: u8| match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' => 62,
        b'/' => 63,
        _ => 0,
    };
    let mut out = Vec::new();
    for ch in s.as_bytes().chunks(4) {
        let n = (u32::from(val(ch[0])) << 18)
            | (u32::from(val(ch[1])) << 12)
            | (u32::from(val(ch[2])) << 6)
            | u32::from(val(ch[3]));
        out.push((n >> 16) as u8);
        if ch[2] != b'=' {
            out.push((n >> 8) as u8);
        }
        if ch[3] != b'=' {
            out.push(n as u8);
        }
    }
    out
}

#[test]
fn extract_keeps_jpeg_and_decodes_png() {
    let dir = tempfile::tempdir().unwrap();
    let jpg = dir.path().join("a.jpg");
    sample(80, 60).save_with_format(&jpg, ImageFormat::Jpeg).unwrap();
    let png = dir.path().join("b.png");
    sample(40, 30).save_with_format(&png, ImageFormat::Png).unwrap();
    let pdf = dir.path().join("i.pdf");
    let ctx = JobContext::silent();
    images_to_pdf(&[jpg.clone(), png], &pdf, &ImagesToPdfOptions::default(), &ctx).unwrap();

    let out = dir.path().join("ex");
    let r = extract_images(
        &pdf,
        None,
        &PageRange::all(),
        &out,
        &ExtractImagesOptions::default(),
        &ctx,
    )
    .unwrap();
    assert_eq!(r.outputs.len(), 2, "{:?}", r.warnings);
    assert!(r.outputs[0].extension().unwrap() == "jpg");
    assert_eq!(std::fs::read(&r.outputs[0]).unwrap(), std::fs::read(&jpg).unwrap());
    assert!(r.outputs[1].extension().unwrap() == "png");
    assert_eq!(image::image_dimensions(&r.outputs[1]).unwrap(), (40, 30));
}
