//! Detección sobre el corpus de formularios planos (tools/gen_formdesign_corpus.py) y sobre un escaneo derivado.

mod common;

use std::path::PathBuf;

use mupdf::pdf::PdfDocument;
use mupdf::{Colorspace, Matrix};
use pliego_core::JobContext;
use pliego_formdesign::*;
use serde_json::Value;

fn corpus(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/corpus/formdesign")
        .join(name)
}

struct Expected {
    kind: FieldType,
    rect: [f32; 4],
}

fn expected(n: usize) -> Vec<Expected> {
    let v: Value = serde_json::from_slice(&std::fs::read(corpus(&format!("flat-{n}.json"))).unwrap()).unwrap();
    v["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| Expected {
            kind: if f["kind"] == "checkbox" {
                FieldType::CheckBox
            } else {
                FieldType::Text
            },
            rect: [0, 1, 2, 3].map(|i| f["rect"][i].as_f64().unwrap() as f32),
        })
        .collect()
}

fn iou(a: [f32; 4], b: [f32; 4]) -> f32 {
    let w = (a[2].min(b[2]) - a[0].max(b[0])).max(0.0);
    let h = (a[3].min(b[3]) - a[1].max(b[1])).max(0.0);
    let i = w * h;
    i / ((a[2] - a[0]) * (a[3] - a[1]) + (b[2] - b[0]) * (b[3] - b[1]) - i)
}

/// (esperados encontrados, esperados, detectados que coinciden con alguno, detectados)
fn score(found: &[FieldSpec], want: &[Expected]) -> (usize, usize, usize, usize) {
    let hit = |f: &FieldSpec, e: &Expected| f.kind == e.kind && iou(f.rect, e.rect) >= 0.3;
    let recall = want.iter().filter(|e| found.iter().any(|f| hit(f, e))).count();
    let precise = found.iter().filter(|f| want.iter().any(|e| hit(f, e))).count();
    (recall, want.len(), precise, found.len())
}

fn detect(doc: &PdfDocument) -> Vec<FieldSpec> {
    detect_fields(doc, 0, &JobContext::silent()).unwrap()
}

#[test]
fn flat_forms_reach_80_percent() {
    let (mut r, mut n, mut p, mut d) = (0, 0, 0, 0);
    for i in 1..=9 {
        let doc = PdfDocument::open(corpus(&format!("flat-{i}.pdf")).to_str().unwrap()).unwrap();
        let found = detect(&doc);
        let s = score(&found, &expected(i));
        eprintln!("flat-{i}: recall {}/{}  precisión {}/{}", s.0, s.1, s.2, s.3);
        assert!(s.0 as f32 >= 0.7 * s.1 as f32, "flat-{i}: recall {}/{}", s.0, s.1);
        r += s.0;
        n += s.1;
        p += s.2;
        d += s.3;
    }
    eprintln!(
        "total: recall {r}/{n} ({:.0} %), precisión {p}/{d} ({:.0} %)",
        100.0 * r as f32 / n as f32,
        100.0 * p as f32 / d as f32
    );
    assert!(r as f32 >= 0.8 * n as f32, "recall global {r}/{n}");
    assert!(p as f32 >= 0.7 * d as f32, "precisión global {p}/{d}");
}

/// Convierte un PDF en un «escaneo»: solo una imagen en grises, sin capa de texto.
fn scan_of(doc: &PdfDocument, noise_per_mille: u64) -> PdfDocument {
    let page = doc.load_pdf_page(0).unwrap();
    let pm = page
        .to_pixmap(&Matrix::new_scale(2.0, 2.0), &Colorspace::device_gray(), false, false)
        .unwrap();
    let (w, h, n, stride) = (
        pm.width() as usize,
        pm.height() as usize,
        pm.n() as usize,
        pm.stride() as usize,
    );
    let mut raw = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            raw.push(pm.samples()[y * stride + x * n]);
        }
    }
    // Ruido determinista de sal y pimienta.
    let mut seed = 0x2545_F491_4F6C_DD1Du64;
    for px in raw.iter_mut() {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        if (seed >> 33) % 1000 < noise_per_mille {
            *px = if (seed >> 20) & 1 == 0 { 0 } else { 255 };
        }
    }
    let content = format!("q {} 0 0 {} 0 0 cm /Im Do Q", w as f32 / 2.0, h as f32 / 2.0);
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offs = vec![];
    let mut obj = |out: &mut Vec<u8>, num: usize, body: Vec<u8>| {
        offs.push(out.len());
        out.extend(format!("{num} 0 obj\n").into_bytes());
        out.extend(body);
        out.extend(b"\nendobj\n");
    };
    obj(&mut out, 1, b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
    obj(&mut out, 2, b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec());
    obj(
        &mut out,
        3,
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Resources << /XObject << /Im 4 0 R >> >> /Contents 5 0 R >>",
            w / 2,
            h / 2
        )
        .into_bytes(),
    );
    let mut img = format!(
        "<< /Type /XObject /Subtype /Image /Width {w} /Height {h} /ColorSpace /DeviceGray /BitsPerComponent 8 /Length {} >>\nstream\n",
        raw.len()
    )
    .into_bytes();
    img.extend(&raw);
    img.extend(b"\nendstream");
    obj(&mut out, 4, img);
    obj(
        &mut out,
        5,
        format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()).into_bytes(),
    );
    let x = out.len();
    out.extend(b"xref\n0 6\n0000000000 65535 f \n");
    for o in offs {
        out.extend(format!("{o:010} 00000 n \n").into_bytes());
    }
    out.extend(format!("trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{x}\n%%EOF\n").into_bytes());
    PdfDocument::from_bytes(&out).unwrap()
}

#[test]
fn scan_without_text_layer() {
    let src = PdfDocument::open(corpus("flat-8.pdf").to_str().unwrap()).unwrap();
    let scan = scan_of(&src, 0);
    let found = detect(&scan);
    let s = score(&found, &expected(8));
    eprintln!("escaneo flat-8: recall {}/{}  precisión {}/{}", s.0, s.1, s.2, s.3);
    assert!(s.0 as f32 >= 0.6 * s.1 as f32, "recall {}/{}", s.0, s.1);
    assert!(
        found.iter().all(|f| f.name.starts_with("campo_")),
        "sin texto no hay etiquetas"
    );
}

#[test]
fn names_come_from_labels_and_are_unique() {
    let doc = PdfDocument::open(corpus("flat-1.pdf").to_str().unwrap()).unwrap();
    let found = detect(&doc);
    let names: Vec<&str> = found.iter().map(|f| f.name.as_str()).collect();
    for n in ["Nombre", "Apellidos", "Domicilio", "Fecha"] {
        assert!(names.contains(&n), "{n} en {names:?}");
    }
    let mut sorted = names.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), names.len());
    // Orden de lectura: de arriba abajo.
    assert!(found.windows(2).all(|w| w[0].rect[1] >= w[1].rect[1] - 1.0));
}

#[test]
fn comb_boxes_and_checkboxes() {
    let doc = PdfDocument::open(corpus("flat-5.pdf").to_str().unwrap()).unwrap();
    let found = detect(&doc);
    let lens: Vec<u32> = found.iter().filter_map(|f| f.max_len).collect();
    assert!(lens.contains(&9) && lens.contains(&5) && lens.contains(&24), "{lens:?}");
    let doc = PdfDocument::open(corpus("flat-4.pdf").to_str().unwrap()).unwrap();
    let found = detect(&doc);
    assert!(found.iter().filter(|f| f.kind == FieldType::CheckBox).count() >= 7);
}

#[test]
fn detected_fields_can_be_added_and_not_proposed_again() {
    let mut doc = PdfDocument::open(corpus("flat-2.pdf").to_str().unwrap()).unwrap();
    let found = detect(&doc);
    assert!(found.len() >= 6);
    for f in &found {
        add_field(&mut doc, f).unwrap();
    }
    assert_eq!(list_fields(&doc).unwrap().len(), found.len());
    // Con los campos ya creados no se proponen de nuevo.
    let again = detect(&common::reopen(&doc));
    assert!(again.len() <= found.len() / 4, "{} nuevos", again.len());
}

#[test]
fn rotated_page_maps_to_user_space() {
    let doc = PdfDocument::open(corpus("flat-1.pdf").to_str().unwrap()).unwrap();
    let mut page = doc.find_page(0).unwrap();
    page.dict_put("Rotate", doc.new_int(90).unwrap()).unwrap();
    let found = detect(&doc);
    let want = expected(1);
    let s = score(&found, &want);
    assert!(s.0 as f32 >= 0.7 * s.1 as f32, "página girada: {}/{}", s.0, s.1);
}

#[test]
fn bad_page_and_cancel() {
    let doc = common::open_blank();
    assert!(detect_fields(&doc, 9, &JobContext::silent()).is_err());
    let ctx = JobContext::silent();
    ctx.cancel.cancel();
    assert!(detect_fields(&doc, 0, &ctx).is_err());
    assert!(detect(&doc).is_empty(), "una página en blanco no tiene campos");
}

#[test]
fn noisy_scan_still_finds_most_fields() {
    let src = PdfDocument::open(corpus("flat-8.pdf").to_str().unwrap()).unwrap();
    let found = detect(&scan_of(&src, 2));
    let s = score(&found, &expected(8));
    eprintln!("escaneo con ruido: recall {}/{}  precisión {}/{}", s.0, s.1, s.2, s.3);
    assert!(s.0 as f32 >= 0.6 * s.1 as f32, "recall {}/{}", s.0, s.1);
}
