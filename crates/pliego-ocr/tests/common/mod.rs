//! Generador de «escaneos» sintéticos: texto conocido → imagen a 300 ppp → giro, ruido → PDF de solo imagen.
#![allow(dead_code)]

use std::path::Path;

use mupdf::pdf::PdfDocument;
use mupdf::{Colorspace, Image, Matrix, Pixmap, Size};
use pliego_ocr::preprocess::{rotate, rotate_quarter};
use pliego_ocr::textlayer::win_ansi;
use pliego_ocr::{GrayImage, available_languages};

pub struct ScanSpec<'a> {
    pub lines: &'a [&'a str],
    /// Inclinación del escaneo (grados, a favor del reloj).
    pub skew: f32,
    /// Probabilidad (0-1) de píxel de ruido «sal y pimienta».
    pub noise: f32,
    /// Giro de página completo (0, 90, 180, 270) a favor del reloj.
    pub quarter: u32,
}

pub fn has_models(langs: &[&str]) -> bool {
    let avail = available_languages();
    langs.iter().all(|l| avail.iter().any(|a| a == l))
}

fn lcg(state: &mut u64) -> u32 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    (*state >> 33) as u32
}

/// PDF vectorial con las líneas de texto en Helvetica 13 pt.
fn text_pdf(lines: &[&str]) -> PdfDocument {
    let mut doc = PdfDocument::new();
    let mut page = doc.new_page(Size::new(612.0, 792.0)).unwrap();
    let font = doc
        .new_object_from_str("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>")
        .unwrap();
    let font = doc.add_object(&font).unwrap();
    let mut res = page.resources().unwrap();
    let mut fonts = doc.new_dict().unwrap();
    fonts.dict_put("F1", font).unwrap();
    res.dict_put("Font", fonts).unwrap();
    let mut content: Vec<u8> = b"BT /F1 13 Tf 16 TL 56 730 Td\n".to_vec();
    for l in lines {
        content.push(b'(');
        for c in l.chars() {
            let b = win_ansi(c).unwrap_or(b'?');
            if matches!(b, b'(' | b')' | b'\\') {
                content.push(b'\\');
            }
            content.push(b);
        }
        content.extend_from_slice(b") Tj T*\n");
    }
    content.extend_from_slice(b"ET\n");
    page.insert_contents(&mut doc, &content, true).unwrap();
    doc
}

pub fn render_gray(doc: &PdfDocument, dpi: f32) -> GrayImage {
    let page = doc.load_pdf_page(0).unwrap();
    let s = dpi / 72.0;
    let pix = page
        .to_pixmap(&Matrix::new_scale(s, s), &Colorspace::device_gray(), false, false)
        .unwrap();
    GrayImage::from_interleaved(pix.width(), pix.height(), pix.n() as usize, pix.samples()).unwrap()
}

/// Escribe en `path` un PDF de una página que solo contiene una imagen.
pub fn image_pdf(img: &GrayImage, path: &Path) {
    let mut doc = PdfDocument::new();
    let (w_pt, h_pt) = (img.width as f32 * 72.0 / 300.0, img.height as f32 * 72.0 / 300.0);
    let mut page = doc.new_page(Size::new(w_pt, h_pt)).unwrap();
    let mut pix = Pixmap::new_with_w_h(&Colorspace::device_gray(), img.width as i32, img.height as i32, false).unwrap();
    pix.samples_mut().copy_from_slice(&img.data);
    let image = Image::from_pixmap(&pix).unwrap();
    let image_obj = doc.add_image(&image).unwrap();
    let mut res = page.resources().unwrap();
    let mut xobj = doc.new_dict().unwrap();
    xobj.dict_put("Im0", image_obj).unwrap();
    res.dict_put("XObject", xobj).unwrap();
    let content = format!("q {w_pt} 0 0 {h_pt} 0 0 cm /Im0 Do Q\n");
    page.insert_contents(&mut doc, content.as_bytes(), true).unwrap();
    doc.save(path.to_str().unwrap()).unwrap();
}

pub fn make_scan(path: &Path, spec: &ScanSpec) {
    let mut img = render_gray(&text_pdf(spec.lines), 300.0);
    if spec.skew != 0.0 {
        img = rotate(&img, spec.skew);
    }
    if spec.noise > 0.0 {
        let mut st = 0x9e3779b97f4a7c15u64;
        let thr = (spec.noise * u32::MAX as f32 / 2.0) as u32; // mitad sal, mitad pimienta
        for p in img.data.iter_mut() {
            let r = lcg(&mut st) as u64 * 2 % u32::MAX as u64;
            if (r as u32) < thr {
                *p = if r.is_multiple_of(2) { 0 } else { 255 };
            }
        }
    }
    if spec.quarter != 0 {
        img = rotate_quarter(&img, spec.quarter);
    }
    image_pdf(&img, path);
}

/// Texto de todas las páginas, con espacios normalizados.
pub fn extract_text(path: &Path) -> String {
    let doc = mupdf::Document::open(path.to_str().unwrap()).unwrap();
    let mut out = String::new();
    for i in 0..doc.page_count().unwrap() {
        let tp = doc
            .load_page(i)
            .unwrap()
            .to_text_page(mupdf::TextPageFlags::empty())
            .unwrap();
        out.push_str(&tp.to_text().unwrap());
        out.push(' ');
    }
    normalize(&out)
}

pub fn normalize(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Tasa de error por carácter (Levenshtein / longitud de la referencia).
pub fn cer(reference: &str, hypothesis: &str) -> f32 {
    let (a, b): (Vec<char>, Vec<char>) = (reference.chars().collect(), hypothesis.chars().collect());
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i; b.len() + 1];
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        prev = cur;
    }
    prev[b.len()] as f32 / a.len().max(1) as f32
}

pub const SPANISH: &[&str] = &[
    "La rápida zorra marrón salta sobre el perro perezoso.",
    "Pliego es una alternativa libre para trabajar con documentos PDF.",
    "El corazón de la ciudad late con más fuerza cada mañana de otoño.",
    "Mañana habrá reunión a las diez en la sala número cinco del edificio.",
    "Los niños jugaban en el jardín mientras llovía suavemente sobre el tejado.",
];
