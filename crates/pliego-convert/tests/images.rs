//! Pruebas de ida y vuelta: imágenes → PDF → imágenes con las mismas dimensiones.

use std::path::{Path, PathBuf};

use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use pliego_convert::{
    ExportImageFormat, ImagesToPdfOptions, PageSizeSpec, PdfToImagesOptions, TextExportOptions, images_to_pdf,
    pdf_to_images, pdf_to_text,
};
use pliego_core::{JobContext, PageRange};

fn sample(w: u32, h: u32, seed: u8) -> RgbImage {
    RgbImage::from_fn(w, h, |x, y| {
        Rgb([(x as u8).wrapping_add(seed), (y as u8).wrapping_mul(2), seed])
    })
}

fn save(dir: &Path, name: &str, img: &RgbImage, fmt: ImageFormat) -> PathBuf {
    let path = dir.join(name);
    DynamicImage::ImageRgb8(img.clone())
        .save_with_format(&path, fmt)
        .unwrap();
    path
}

fn dims(path: &Path) -> (u32, u32) {
    image::image_dimensions(path).unwrap()
}

fn opts_72() -> ImagesToPdfOptions {
    ImagesToPdfOptions {
        dpi: 72.0,
        ..Default::default()
    }
}

#[test]
fn roundtrip_png_jpeg_webp_keep_dimensions() {
    let dir = tempfile::tempdir().unwrap();
    let inputs = vec![
        save(dir.path(), "a.png", &sample(200, 100, 1), ImageFormat::Png),
        save(dir.path(), "b.jpg", &sample(120, 160, 2), ImageFormat::Jpeg),
        save(dir.path(), "c.webp", &sample(90, 90, 3), ImageFormat::WebP),
    ];
    let pdf = dir.path().join("out.pdf");
    let ctx = JobContext::silent();
    let report = images_to_pdf(&inputs, &pdf, &opts_72(), &ctx).unwrap();
    assert_eq!(report.outputs, vec![pdf.clone()]);

    let out = dir.path().join("back");
    let r = pdf_to_images(
        &pdf,
        None,
        &PageRange::all(),
        &out,
        &PdfToImagesOptions {
            dpi: 72.0,
            ..Default::default()
        },
        &ctx,
    )
    .unwrap();
    assert_eq!(r.outputs.len(), 3);
    assert_eq!(dims(&r.outputs[0]), (200, 100));
    assert_eq!(dims(&r.outputs[1]), (120, 160));
    assert_eq!(dims(&r.outputs[2]), (90, 90));
}

#[test]
fn multipage_tiff_gives_one_page_per_frame() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("m.tif");
    {
        let file = std::fs::File::create(&path).unwrap();
        let mut enc = tiff::encoder::TiffEncoder::new(file).unwrap();
        for (w, h) in [(64u32, 48u32), (80, 40)] {
            let data = sample(w, h, 9).into_raw();
            enc.write_image::<tiff::encoder::colortype::RGB8>(w, h, &data).unwrap();
        }
    }
    let pdf = dir.path().join("t.pdf");
    let ctx = JobContext::silent();
    images_to_pdf(&[path], &pdf, &opts_72(), &ctx).unwrap();
    let r = pdf_to_images(
        &pdf,
        None,
        &PageRange::all(),
        &dir.path().join("o"),
        &PdfToImagesOptions {
            dpi: 72.0,
            ..Default::default()
        },
        &ctx,
    )
    .unwrap();
    assert_eq!(r.outputs.len(), 2);
    assert_eq!(dims(&r.outputs[0]), (64, 48));
    assert_eq!(dims(&r.outputs[1]), (80, 40));
}

#[test]
fn fixed_page_size_orientation_and_margins() {
    let dir = tempfile::tempdir().unwrap();
    let wide = save(dir.path(), "w.png", &sample(400, 100, 4), ImageFormat::Png);
    let tall = save(dir.path(), "t.png", &sample(100, 400, 5), ImageFormat::Png);
    let pdf = dir.path().join("a4.pdf");
    let o = ImagesToPdfOptions {
        page_size: PageSizeSpec::A4,
        margin_pt: 36.0,
        dpi: 72.0,
        ..Default::default()
    };
    images_to_pdf(&[wide, tall], &pdf, &o, &JobContext::silent()).unwrap();
    let doc = pliego_pdf::Document::open(&pdf).unwrap();
    let b0 = doc.load_page(0).unwrap().bounds().unwrap();
    let b1 = doc.load_page(1).unwrap().bounds().unwrap();
    assert!(
        b0.width() > b0.height(),
        "la imagen apaisada debe dar página horizontal"
    );
    assert!(b1.height() > b1.width());
    assert!((b1.width() - 595.28).abs() < 0.5);
}

#[test]
fn jpeg_is_embedded_without_recompression() {
    let dir = tempfile::tempdir().unwrap();
    let jpg = save(dir.path(), "p.jpg", &sample(64, 64, 7), ImageFormat::Jpeg);
    let original = std::fs::read(&jpg).unwrap();
    let pdf = dir.path().join("j.pdf");
    images_to_pdf(&[jpg], &pdf, &opts_72(), &JobContext::silent()).unwrap();
    let bytes = std::fs::read(&pdf).unwrap();
    assert!(bytes.windows(original.len()).any(|w| w == original.as_slice()));
}

#[test]
fn jpeg_and_tiff_export() {
    let dir = tempfile::tempdir().unwrap();
    let png = save(dir.path(), "a.png", &sample(50, 30, 1), ImageFormat::Png);
    let pdf = dir.path().join("a.pdf");
    let ctx = JobContext::silent();
    images_to_pdf(&[png], &pdf, &opts_72(), &ctx).unwrap();
    for (fmt, ext) in [(ExportImageFormat::Jpeg, "jpg"), (ExportImageFormat::Tiff, "tiff")] {
        let o = PdfToImagesOptions {
            format: fmt,
            dpi: 72.0,
            ..Default::default()
        };
        let r = pdf_to_images(&pdf, None, &PageRange::all(), &dir.path().join(ext), &o, &ctx).unwrap();
        assert!(r.outputs[0].to_str().unwrap().ends_with(ext));
        assert_eq!(dims(&r.outputs[0]), (50, 30));
    }
}

#[test]
fn text_export_contains_original_text() {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/basic/hola.pdf");
    let dir = tempfile::tempdir().unwrap();
    let txt = dir.path().join("t.txt");
    let r = pdf_to_text(
        &corpus,
        None,
        &PageRange::all(),
        &txt,
        &TextExportOptions::default(),
        &JobContext::silent(),
    )
    .unwrap();
    assert!(r.warnings.is_empty());
    let s = std::fs::read_to_string(&txt).unwrap();
    assert!(s.contains("Hola Pliego pagina uno"));
    assert!(s.contains("Segunda pagina de prueba Pliego"));
    assert!(s.contains('\u{c}'));
}

#[test]
fn errors_are_clear() {
    let dir = tempfile::tempdir().unwrap();
    let bad = dir.path().join("x.bin");
    std::fs::write(&bad, b"no soy una imagen").unwrap();
    let ctx = JobContext::silent();
    let e = images_to_pdf(&[bad], &dir.path().join("o.pdf"), &opts_72(), &ctx).unwrap_err();
    assert!(e.to_string().contains("no reconocido"));
    assert!(images_to_pdf(&[], &dir.path().join("o.pdf"), &opts_72(), &ctx).is_err());
}
