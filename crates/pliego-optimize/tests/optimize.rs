use std::io::Write;
use std::path::{Path, PathBuf};

use flate2::Compression;
use flate2::write::ZlibEncoder;
use jpeg_encoder::{ColorType, Encoder};
use pliego_core::JobContext;
use pliego_optimize::{OptimizeOptions, Preset, analyze, optimize, optimize_detailed};
use pliego_pdf::{Colorspace, Document, Matrix, TextFlags};

/// Escribe un PDF de una página A4 con una imagen a página completa y, opcionalmente, texto invisible.
fn pdf_with_image(
    width: usize,
    height: usize,
    rgb_or_gray: u8,
    filter: &str,
    data: &[u8],
    text: Option<&str>,
) -> Vec<u8> {
    let cs = if rgb_or_gray == 3 { "/DeviceRGB" } else { "/DeviceGray" };
    let mut objs: Vec<Vec<u8>> = Vec::new();
    objs.push(b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
    objs.push(b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec());
    objs.push(
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 4 0 R \
          /Resources << /XObject << /Im0 5 0 R >> /Font << /F1 6 0 R >> >> >>"
            .to_vec(),
    );
    let mut content = String::from("q 595 0 0 842 0 0 cm /Im0 Do Q\n");
    if let Some(t) = text {
        content.push_str(&format!("BT 3 Tr /F1 12 Tf 72 700 Td ({t}) Tj ET\n"));
    }
    objs.push(format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()).into_bytes());
    let mut im = format!(
        "<< /Type /XObject /Subtype /Image /Width {width} /Height {height} /ColorSpace {cs} /BitsPerComponent 8 \
         /Filter /{filter} /Length {} >>\nstream\n",
        data.len()
    )
    .into_bytes();
    im.extend_from_slice(data);
    im.extend_from_slice(b"\nendstream");
    objs.push(im);
    objs.push(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec());

    let mut out = b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n".to_vec();
    let mut offs = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offs.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend_from_slice(o);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
    for o in offs {
        out.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objs.len() + 1
        )
        .as_bytes(),
    );
    out
}

/// "Escaneo" sintético en gris: papel con ruido y bloques oscuros que imitan líneas de texto.
fn scan_pixels(w: usize, h: usize) -> Vec<u8> {
    let mut seed = 12345u32;
    let mut rnd = move || {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        (seed >> 24) as u8
    };
    let mut px = vec![0u8; w * h];
    for y in 0..h {
        let line = (y / 60) % 2 == 0 && y > 300 && y < h - 300;
        for x in 0..w {
            let paper = 225 + rnd() % 20;
            px[y * w + x] = if line && x > 300 && x < w - 300 && (x / 25 + y / 60) % 3 != 0 && y % 60 < 30 {
                30 + rnd() % 40
            } else {
                paper
            };
        }
    }
    px
}

fn scan_pdf(path: &Path) {
    let (w, h) = (2480, 3508); // A4 a 300 ppp
    let mut jpeg = Vec::new();
    Encoder::new(&mut jpeg, 92)
        .encode(&scan_pixels(w, h), w as u16, h as u16, ColorType::Luma)
        .unwrap();
    std::fs::write(
        path,
        pdf_with_image(w, h, 1, "DCTDecode", &jpeg, Some("Texto invisible de prueba")),
    )
    .unwrap();
}

fn render(path: &Path, dpi: f32) -> Vec<u8> {
    let doc = Document::open(path.to_str().unwrap()).unwrap();
    let page = doc.load_page(0).unwrap();
    let s = dpi / 72.0;
    let pix = page
        .to_pixmap(&Matrix::new_scale(s, s), Colorspace::Rgb, false, true)
        .unwrap();
    pix.samples().to_vec()
}

fn text(path: &Path) -> String {
    let doc = Document::open(path.to_str().unwrap()).unwrap();
    doc.load_page(0)
        .unwrap()
        .to_text_page(TextFlags::empty())
        .unwrap()
        .to_text()
        .unwrap()
}

fn opts(preset: Preset) -> OptimizeOptions {
    OptimizeOptions {
        preset,
        ..OptimizeOptions::default()
    }
}

#[test]
fn ebook_halves_a_300ppp_scan_and_keeps_the_text_layer() {
    let dir = tempfile::tempdir().unwrap();
    let (input, out) = (dir.path().join("scan.pdf"), dir.path().join("scan-ebook.pdf"));
    scan_pdf(&input);
    let (report, stats) = optimize_detailed(&input, &out, &opts(Preset::Ebook), &JobContext::silent()).unwrap();
    let (a, b) = (
        std::fs::metadata(&input).unwrap().len(),
        std::fs::metadata(&out).unwrap().len(),
    );
    assert!(b * 2 <= a, "ebook solo redujo de {a} a {b}");
    assert_eq!(stats.images.recompressed, 1);
    assert_eq!(stats.images.downsampled, 1);
    assert!(stats.after.images_bytes * 2 < stats.before.images_bytes);
    assert!(report.warnings.is_empty());
    assert!(text(&out).contains("Texto invisible de prueba"));
    // El resultado se parece al original (diferencia media pequeña a 50 ppp).
    let (ra, rb) = (render(&input, 50.0), render(&out, 50.0));
    assert_eq!(ra.len(), rb.len());
    let mad = ra.iter().zip(&rb).map(|(x, y)| x.abs_diff(*y) as u64).sum::<u64>() as f64 / ra.len() as f64;
    assert!(mad < 6.0, "diferencia media {mad}");
    // La imagen quedó a ~150 ppp.
    let u = analyze(&out).unwrap();
    assert!(
        u.largest_images[0].width <= 1250 && u.largest_images[0].width >= 1200,
        "{:?}",
        u.largest_images[0]
    );
}

#[test]
fn print_preset_keeps_300ppp_resolution() {
    let dir = tempfile::tempdir().unwrap();
    let (input, out) = (dir.path().join("scan.pdf"), dir.path().join("o.pdf"));
    scan_pdf(&input);
    let (_, stats) = optimize_detailed(&input, &out, &opts(Preset::Print), &JobContext::silent()).unwrap();
    assert_eq!(stats.images.downsampled, 0);
}

#[test]
fn lossless_never_changes_the_render() {
    let dir = tempfile::tempdir().unwrap();
    // Imagen RGB comprimida con Flate, con datos repetidos que dan margen a la limpieza.
    let (w, h) = (300, 400);
    let raw: Vec<u8> = (0..w * h * 3)
        .map(|i| ((i / 3 % w) * 255 / w) as u8 ^ (i % 3 * 40) as u8)
        .collect();
    let mut z = ZlibEncoder::new(Vec::new(), Compression::fast());
    z.write_all(&raw).unwrap();
    let input = dir.path().join("rgb.pdf");
    std::fs::write(
        &input,
        pdf_with_image(w, h, 3, "FlateDecode", &z.finish().unwrap(), Some("Hola")),
    )
    .unwrap();
    let out = dir.path().join("rgb-lossless.pdf");
    let (_, stats) = optimize_detailed(&input, &out, &opts(Preset::Lossless), &JobContext::silent()).unwrap();
    assert_eq!(stats.images.recompressed, 0);
    assert_eq!(render(&input, 96.0), render(&out, 96.0));
    assert!(text(&out).contains("Hola"));

    // También con el PDF del corpus (imagen movida).
    let corpus: PathBuf = [env!("CARGO_MANIFEST_DIR"), "../../tests/corpus/compare/imagen_a.pdf"]
        .iter()
        .collect();
    let out2 = dir.path().join("corpus-lossless.pdf");
    optimize(&corpus, &out2, &opts(Preset::Lossless), &JobContext::silent()).unwrap();
    assert_eq!(render(&corpus, 96.0), render(&out2, 96.0));
}

#[test]
fn linearize_is_reported_as_unavailable_but_output_is_valid() {
    let dir = tempfile::tempdir().unwrap();
    let corpus: PathBuf = [env!("CARGO_MANIFEST_DIR"), "../../tests/corpus/compare/pagina.pdf"]
        .iter()
        .collect();
    let out = dir.path().join("lin.pdf");
    let o = OptimizeOptions {
        preset: Preset::Lossless,
        linearize: true,
        ..OptimizeOptions::default()
    };
    let report = optimize(&corpus, &out, &o, &JobContext::silent()).unwrap();
    assert!(report.warnings.iter().any(|w| w.contains("linealización")));
    let doc = Document::open(out.to_str().unwrap()).unwrap();
    assert_eq!(doc.page_count().unwrap(), 3);
}

#[test]
fn analyze_splits_space_by_category() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("scan.pdf");
    scan_pdf(&input);
    let u = analyze(&input).unwrap();
    assert_eq!(u.image_count, 1);
    assert_eq!(u.total_bytes, std::fs::metadata(&input).unwrap().len());
    assert!(u.images_bytes > u.total_bytes * 9 / 10);
    let sum = u.images_bytes + u.fonts_bytes + u.content_bytes + u.other_streams_bytes + u.structure_bytes;
    assert_eq!(sum, u.total_bytes);
}

#[test]
fn refuses_to_overwrite_the_input_and_reports_missing_files() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("a.pdf");
    let corpus: PathBuf = [env!("CARGO_MANIFEST_DIR"), "../../tests/corpus/compare/base.pdf"]
        .iter()
        .collect();
    std::fs::copy(corpus, &input).unwrap();
    assert!(optimize(&input, &input, &opts(Preset::Ebook), &JobContext::silent()).is_err());
    assert!(
        optimize(
            &dir.path().join("no.pdf"),
            &dir.path().join("o.pdf"),
            &opts(Preset::Ebook),
            &JobContext::silent()
        )
        .is_err()
    );
}

#[test]
fn preset_names_parse() {
    assert_eq!("pantalla".parse::<Preset>().unwrap(), Preset::Screen);
    assert_eq!("sin-perdida".parse::<Preset>().unwrap(), Preset::Lossless);
    assert!("otro".parse::<Preset>().is_err());
    assert_eq!(Preset::Print.to_string(), "impresion");
}
