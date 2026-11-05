//! Imágenes → PDF con el modelo propio (W40, bloque 8): fidelidad de píxeles, transparencia, formatos y validez.
//! Cada PDF se dibuja con MuPDF (motor independiente) a 72 ppp y se compara con la imagen de origen.

use std::path::{Path, PathBuf};
use std::process::Command;

use image::{DynamicImage, GrayAlphaImage, GrayImage, ImageFormat, Luma, LumaA, Rgb, RgbImage, Rgba, RgbaImage};
use pliego_convert::{ImagesToPdfOptions, images_to_pdf};
use pliego_core::JobContext;
use pliego_pdf::{Colorspace, Document, Matrix};

fn opts() -> ImagesToPdfOptions {
    ImagesToPdfOptions {
        dpi: 72.0,
        ..Default::default()
    }
}

fn make_pdf(dir: &Path, inputs: &[PathBuf]) -> PathBuf {
    let pdf = dir.join("out.pdf");
    images_to_pdf(inputs, &pdf, &opts(), &JobContext::silent()).unwrap();
    pdf
}

/// Dibuja la página `n` a 72 ppp en RGB sobre fondo blanco: (ancho, alto, bytes RGB).
fn render(pdf: &Path, n: i32) -> (u32, u32, Vec<u8>) {
    let doc = Document::open(pdf).unwrap();
    let pix = doc
        .load_page(n)
        .unwrap()
        .to_pixmap(&Matrix::IDENTITY, Colorspace::Rgb, false, true)
        .unwrap();
    let (w, h, stride) = (pix.width(), pix.height(), pix.stride());
    let mut rgb = Vec::new();
    for y in 0..h as usize {
        rgb.extend_from_slice(&pix.samples()[y * stride..y * stride + w as usize * 3]);
    }
    (w, h, rgb)
}

/// Diferencia media absoluta por canal.
fn mean_diff(a: &[u8], b: &[u8]) -> f64 {
    assert_eq!(a.len(), b.len());
    a.iter().zip(b).map(|(x, y)| f64::from(x.abs_diff(*y))).sum::<f64>() / a.len() as f64
}

fn rgb_sample(w: u32, h: u32) -> RgbImage {
    RgbImage::from_fn(w, h, |x, y| Rgb([(x * 5) as u8, (y * 7) as u8, ((x + y) * 3) as u8]))
}

fn save(dir: &Path, name: &str, img: DynamicImage, fmt: ImageFormat) -> PathBuf {
    let p = dir.join(name);
    img.save_with_format(&p, fmt).unwrap();
    p
}

fn qpdf_check(pdf: &Path) {
    if Command::new("qpdf").arg("--version").output().is_ok() {
        let out = Command::new("qpdf").arg("--check").arg(pdf).output().unwrap();
        assert!(
            out.status.success(),
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

#[test]
fn lossless_formats_keep_their_pixels() {
    let dir = tempfile::tempdir().unwrap();
    let src = rgb_sample(40, 30);
    for (name, fmt) in [
        ("a.png", ImageFormat::Png),
        ("a.bmp", ImageFormat::Bmp),
        ("a.gif", ImageFormat::Gif),
        ("a.tif", ImageFormat::Tiff),
        ("a.webp", ImageFormat::WebP),
    ] {
        let p = save(dir.path(), name, DynamicImage::ImageRgb8(src.clone()), fmt);
        let pdf = make_pdf(dir.path(), &[p]);
        qpdf_check(&pdf);
        let (w, h, got) = render(&pdf, 0);
        assert_eq!((w, h), (40, 30), "{name}");
        // GIF cuantiza a 256 colores: tolerancia mayor.
        let tol = if name.ends_with("gif") { 25.0 } else { 1.0 };
        let d = mean_diff(&got, src.as_raw());
        assert!(d <= tol, "{name}: diferencia media {d}");
    }
}

#[test]
fn gray_images_use_device_gray_and_stay_small() {
    let dir = tempfile::tempdir().unwrap();
    let g = GrayImage::from_fn(64, 64, |x, y| Luma([(x * 4) as u8 ^ y as u8]));
    let p = save(
        dir.path(),
        "g.png",
        DynamicImage::ImageLuma8(g.clone()),
        ImageFormat::Png,
    );
    let pdf = make_pdf(dir.path(), &[p]);
    let bytes = std::fs::read(&pdf).unwrap();
    assert!(bytes.windows(10).any(|w| w == b"DeviceGray"), "debe ser DeviceGray");
    assert!(!bytes.windows(9).any(|w| w == b"DeviceRGB"));
    let (_, _, got) = render(&pdf, 0);
    let expect: Vec<u8> = g.as_raw().iter().flat_map(|&v| [v, v, v]).collect();
    assert!(mean_diff(&got, &expect) <= 1.0);
    // Comprimido (Flate): bastante menos que los 4096 bytes de muestras + estructura.
    assert!(bytes.windows(11).any(|w| w == b"FlateDecode"));
}

#[test]
fn transparency_becomes_a_soft_mask() {
    let dir = tempfile::tempdir().unwrap();
    // Cuadrante izquierdo opaco rojo, derecho rojo al 50 %, abajo transparente.
    let rgba = RgbaImage::from_fn(20, 20, |x, y| {
        let a = if y >= 10 {
            0
        } else if x < 10 {
            255
        } else {
            128
        };
        Rgba([255, 0, 0, a])
    });
    let p = save(
        dir.path(),
        "t.png",
        DynamicImage::ImageRgba8(rgba.clone()),
        ImageFormat::Png,
    );
    let pdf = make_pdf(dir.path(), &[p]);
    qpdf_check(&pdf);
    assert!(std::fs::read(&pdf).unwrap().windows(5).any(|w| w == b"SMask"));
    let (w, _, got) = render(&pdf, 0);
    let px = |x: usize, y: usize| {
        let i = (y * w as usize + x) * 3;
        [got[i], got[i + 1], got[i + 2]]
    };
    assert_eq!(px(2, 2), [255, 0, 0], "opaco");
    let half = px(15, 2);
    assert!(half[0] >= 254 && (120..=135).contains(&half[1]), "50 %: {half:?}");
    assert_eq!(px(2, 15), [255, 255, 255], "transparente sobre blanco");

    // Gris con alfa.
    let la = GrayAlphaImage::from_fn(10, 10, |x, _| LumaA([0, if x < 5 { 255 } else { 0 }]));
    let p = save(dir.path(), "la.png", DynamicImage::ImageLumaA8(la), ImageFormat::Png);
    let pdf = make_pdf(dir.path(), &[p]);
    let (w, _, got) = render(&pdf, 0);
    assert_eq!(got[0], 0);
    assert_eq!(got[(8 * 3) as usize % (w as usize * 3)], 255);
}

#[test]
fn opaque_alpha_channel_is_dropped_and_16_bit_is_reduced() {
    let dir = tempfile::tempdir().unwrap();
    let rgba = RgbaImage::from_fn(8, 8, |x, y| Rgba([x as u8 * 30, y as u8 * 30, 7, 255]));
    let p = save(dir.path(), "o.png", DynamicImage::ImageRgba8(rgba), ImageFormat::Png);
    let pdf = make_pdf(dir.path(), &[p]);
    assert!(!std::fs::read(&pdf).unwrap().windows(5).any(|w| w == b"SMask"));

    let img16 =
        image::ImageBuffer::<Rgb<u16>, Vec<u16>>::from_fn(8, 8, |x, y| Rgb([x as u16 * 8000, y as u16 * 8000, 65535]));
    let p = save(dir.path(), "s.png", DynamicImage::ImageRgb16(img16), ImageFormat::Png);
    let pdf = make_pdf(dir.path(), &[p]);
    let (_, _, got) = render(&pdf, 0);
    assert_eq!(&got[..3], &[0, 0, 255]);
}

#[test]
fn many_images_pages_sizes_and_validity() {
    let dir = tempfile::tempdir().unwrap();
    let a = save(
        dir.path(),
        "a.png",
        DynamicImage::ImageRgb8(rgb_sample(30, 20)),
        ImageFormat::Png,
    );
    let b = save(
        dir.path(),
        "b.jpg",
        DynamicImage::ImageRgb8(rgb_sample(20, 30)),
        ImageFormat::Jpeg,
    );
    let c = save(
        dir.path(),
        "c.png",
        DynamicImage::ImageLuma8(GrayImage::new(10, 10)),
        ImageFormat::Png,
    );
    let pdf = make_pdf(dir.path(), &[a, b, c]);
    qpdf_check(&pdf);
    let doc = Document::open(&pdf).unwrap();
    assert_eq!(doc.page_count().unwrap(), 3);
    let sizes: Vec<(u32, u32)> = (0..3)
        .map(|i| {
            let (w, h, _) = render(&pdf, i);
            (w, h)
        })
        .collect();
    assert_eq!(sizes, vec![(30, 20), (20, 30), (10, 10)]);
}

#[test]
fn tiff_variants_decode() {
    let dir = tempfile::tempdir().unwrap();
    use tiff::encoder::{TiffEncoder, colortype};
    // Gris 8, RGBA 8 (con transparencia), CMYK 8 y RGB 16 como páginas de un mismo TIFF.
    let path = dir.path().join("v.tif");
    {
        let mut enc = TiffEncoder::new(std::fs::File::create(&path).unwrap()).unwrap();
        enc.write_image::<colortype::Gray8>(4, 4, &[90u8; 16]).unwrap();
        let mut rgba = Vec::new();
        for i in 0..16u8 {
            rgba.extend_from_slice(&[0, 0, 255, if i < 8 { 255 } else { 0 }]);
        }
        enc.write_image::<colortype::RGBA8>(4, 4, &rgba).unwrap();
        enc.write_image::<colortype::CMYK8>(4, 4, &[0u8, 0, 0, 255].repeat(16))
            .unwrap();
        enc.write_image::<colortype::RGB16>(4, 4, &[65535u16, 0, 0].repeat(16))
            .unwrap();
    }
    let pdf = make_pdf(dir.path(), &[path]);
    qpdf_check(&pdf);
    assert_eq!(Document::open(&pdf).unwrap().page_count().unwrap(), 4);
    assert_eq!(&render(&pdf, 0).2[..3], &[90, 90, 90]);
    let rgba = render(&pdf, 1).2;
    assert_eq!(&rgba[..3], &[0, 0, 255]);
    assert_eq!(&rgba[rgba.len() - 3..], &[255, 255, 255]);
    let cmyk = render(&pdf, 2).2;
    assert!(
        cmyk[0] < 60 && cmyk[1] < 60 && cmyk[2] < 60,
        "negro CMYK: {:?}",
        &cmyk[..3]
    );
    assert_eq!(&render(&pdf, 3).2[..3], &[255, 0, 0]);
}

#[test]
fn broken_and_oversized_images_fail_cleanly() {
    let dir = tempfile::tempdir().unwrap();
    let ok = save(
        dir.path(),
        "a.png",
        DynamicImage::ImageRgb8(rgb_sample(30, 20)),
        ImageFormat::Png,
    );
    let bytes = std::fs::read(&ok).unwrap();
    for (name, data) in [
        ("cortada.png", bytes[..bytes.len() / 2].to_vec()),
        ("jpg-roto.jpg", vec![0xFF, 0xD8, 0xFF, 0xE0, 0, 4, 1, 2]),
        ("tif-roto.tif", b"II*\0\x08\0\0\0\xff\xff".to_vec()),
        ("bmp-roto.bmp", b"BM\x00\x00".to_vec()),
    ] {
        let p = dir.path().join(name);
        std::fs::write(&p, data).unwrap();
        let e = images_to_pdf(&[p], &dir.path().join("o.pdf"), &opts(), &JobContext::silent()).unwrap_err();
        assert!(!e.to_string().is_empty(), "{name}");
    }
}

#[test]
fn grayscale_jpeg_is_embedded_untouched_as_device_gray() {
    let dir = tempfile::tempdir().unwrap();
    let g = GrayImage::from_fn(48, 32, |x, y| Luma([(x * 5 + y) as u8]));
    let p = save(dir.path(), "g.jpg", DynamicImage::ImageLuma8(g), ImageFormat::Jpeg);
    let original = std::fs::read(&p).unwrap();
    let pdf = make_pdf(dir.path(), &[p]);
    qpdf_check(&pdf);
    let bytes = std::fs::read(&pdf).unwrap();
    assert!(bytes.windows(original.len()).any(|w| w == original.as_slice()));
    assert!(bytes.windows(9).any(|w| w == b"DCTDecode") && bytes.windows(10).any(|w| w == b"DeviceGray"));
    let (w, h, _) = render(&pdf, 0);
    assert_eq!((w, h), (48, 32));
}
