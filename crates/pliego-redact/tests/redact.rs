use std::path::{Path, PathBuf};

use pliego_core::JobContext;
use pliego_pdf::{Colorspace, Matrix, PdfDocument, TextFlags};
use pliego_redact::{Area, Fill, ImageMode, Pattern, RedactJob, RedactOptions, apply, find_patterns, verify};

fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/redact/datos.pdf")
}

fn open(p: &Path) -> PdfDocument {
    PdfDocument::open(p).unwrap()
}

fn text_of(p: &Path) -> String {
    let doc = open(p);
    let mut all = String::new();
    for i in 0..doc.page_count().unwrap() {
        let page = doc.load_page(i).unwrap();
        all.push_str(&page.to_text_page(TextFlags::empty()).unwrap().to_text().unwrap());
    }
    all
}

/// Color (r, g, b) del píxel (x, y) de la página 1 renderizada a 72 ppp.
fn pixel(p: &Path, x: usize, y: usize) -> (u8, u8, u8) {
    let doc = open(p);
    let page = doc.load_page(0).unwrap();
    let pix = page
        .to_pixmap(&Matrix::IDENTITY, Colorspace::Rgb, false, false)
        .unwrap();
    let n = pix.n() as usize;
    let i = y * pix.stride() + x * n;
    let s = pix.samples();
    (s[i], s[i + 1], s[i + 2])
}

fn run(job: &RedactJob) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("redactado.pdf");
    apply(&corpus(), &out, None, job, &JobContext::silent()).unwrap();
    (dir, out)
}

fn job(patterns: Vec<Pattern>) -> RedactJob {
    RedactJob {
        patterns,
        ..Default::default()
    }
}

#[test]
fn original_contains_everything() {
    let t = text_of(&corpus());
    for s in [
        "12345678Z",
        "X1234567L",
        "A58818501",
        "2100 0418",
        "ana.lopez@ejemplo.es",
        "612 345 678",
        "12/03/1985",
    ] {
        assert!(t.contains(s), "el corpus debería contener {s}");
    }
}

#[test]
fn each_pattern_removes_its_text_and_keeps_the_rest() {
    let cases: [(Pattern, &str); 7] = [
        (Pattern::Dni, "12345678Z"),
        (Pattern::Nie, "X1234567L"),
        (Pattern::Cif, "A58818501"),
        (Pattern::Iban, "ES91 2100 0418 4502 0005 1332"),
        (Pattern::Email, "ana.lopez@ejemplo.es"),
        (Pattern::Phone, "+34 612 345 678"),
        (Pattern::Date, "12/03/1985"),
    ];
    for (pattern, needle) in &cases {
        let (_d, out) = run(&job(vec![pattern.clone()]));
        let t = text_of(&out);
        assert!(
            !t.replace(' ', "").contains(&needle.replace(' ', "")),
            "{pattern:?}: sigue «{needle}»"
        );
        // Las demás líneas siguen ahí (la redacción no daña los vecinos).
        assert!(
            t.contains("texto normal conservado"),
            "{pattern:?}: se perdió texto vecino"
        );
        assert!(
            t.contains("Fecha de nacimiento") && t.contains("Correo"),
            "{pattern:?}: se perdieron etiquetas"
        );
        // Y verify() lo confirma sobre el archivo resultante.
        assert!(verify(&open(&out), &[needle.to_string()]).unwrap().is_empty());
    }
}

#[test]
fn dni_pattern_hits_both_pages_and_skips_invalid() {
    let doc = open(&corpus());
    let m = find_patterns(&doc, &[Pattern::Dni]).unwrap();
    let texts: Vec<_> = m.iter().map(|m| (m.page, m.text.as_str())).collect();
    assert_eq!(texts, [(0, "12345678Z"), (1, "12345678Z")]);
    let (_d, out) = run(&job(vec![Pattern::Dni]));
    let t = text_of(&out);
    assert!(!t.contains("12345678Z"));
    assert!(t.contains("12345678A"), "el DNI inválido no debe redactarse");
    // Tampoco queda en los bytes del archivo (ni en objetos viejos sin referencia).
    let raw = |p: &Path| std::fs::read(p).unwrap().windows(9).any(|w| w == b"12345678Z");
    assert!(raw(&corpus()), "el original lo tiene en claro");
    assert!(!raw(&out), "el archivo redactado no debe contenerlo");
}

#[test]
fn custom_regex() {
    let (_d, out) = run(&job(vec![Pattern::Regex(r"Ana\s+Lopez".into())]));
    let t = text_of(&out);
    assert!(!t.contains("Ana") && !t.contains("Lopez"));
    assert!(t.contains("12345678Z"));
}

// Coordenadas de página (y hacia abajo): imagen en x 72..172, y 442..542; vector rojo en x 300..400.
fn area(x0: f32, x1: f32) -> Area {
    Area {
        page: 0,
        x0,
        y0: 442.0,
        x1,
        y1: 542.0,
    }
}

#[test]
fn covered_image_is_modified() {
    assert_eq!(pixel(&corpus(), 120, 490), (40, 40, 40));
    let j = RedactJob {
        areas: vec![area(100.0, 140.0)],
        options: RedactOptions {
            fill: Fill::None,
            images: ImageMode::Pixels,
            remove_line_art: true,
        },
        ..Default::default()
    };
    let (_d, out) = run(&j);
    assert_eq!(
        pixel(&out, 120, 490),
        (255, 255, 255),
        "los píxeles cubiertos deben borrarse"
    );
    // La imagen es de 4x4 píxeles (25 pt cada uno): se borran las columnas tocadas, no las otras.
    assert_eq!(pixel(&out, 80, 490), (40, 40, 40), "la parte no cubierta se conserva");
    assert_eq!(pixel(&out, 160, 490), (40, 40, 40), "la parte no cubierta se conserva");
}

#[test]
fn black_fill_and_vector_removal() {
    assert_eq!(pixel(&corpus(), 350, 490), (255, 0, 0));
    let j = RedactJob {
        areas: vec![area(290.0, 410.0)],
        ..Default::default()
    };
    let (_d, out) = run(&j);
    assert_eq!(pixel(&out, 350, 490), (0, 0, 0), "recuadro negro");

    let j = RedactJob {
        areas: vec![area(290.0, 410.0)],
        options: RedactOptions {
            fill: Fill::None,
            images: ImageMode::Pixels,
            remove_line_art: true,
        },
        ..Default::default()
    };
    let (_d, out) = run(&j);
    assert_eq!(pixel(&out, 350, 490), (255, 255, 255), "el vector cubierto desaparece");

    let j = RedactJob {
        areas: vec![area(290.0, 410.0)],
        options: RedactOptions {
            fill: Fill::None,
            images: ImageMode::Pixels,
            remove_line_art: false,
        },
        ..Default::default()
    };
    let (_d, out) = run(&j);
    assert_eq!(pixel(&out, 350, 490), (255, 0, 0), "sin quitar vectores se conserva");
}

#[test]
fn nothing_found_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("x.pdf");
    let rep = apply(
        &corpus(),
        &out,
        None,
        &job(vec![Pattern::Regex("zzzz".into())]),
        &JobContext::silent(),
    )
    .unwrap();
    assert!(rep.outputs.is_empty() && !out.exists());
}

#[test]
fn never_overwrites_input() {
    let dir = tempfile::tempdir().unwrap();
    let copy = dir.path().join("a.pdf");
    std::fs::copy(corpus(), &copy).unwrap();
    assert!(apply(&copy, &copy, None, &job(vec![Pattern::Dni]), &JobContext::silent()).is_err());
    assert!(text_of(&copy).contains("12345678Z"));
}

#[test]
fn bad_area_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("x.pdf");
    let j = RedactJob {
        areas: vec![Area {
            page: 9,
            x0: 0.0,
            y0: 0.0,
            x1: 10.0,
            y1: 10.0,
        }],
        ..Default::default()
    };
    assert!(apply(&corpus(), &out, None, &j, &JobContext::silent()).is_err());
    let j = RedactJob {
        areas: vec![Area {
            page: 0,
            x0: 10.0,
            y0: 0.0,
            x1: 5.0,
            y1: 10.0,
        }],
        ..Default::default()
    };
    assert!(apply(&corpus(), &out, None, &j, &JobContext::silent()).is_err());
}
