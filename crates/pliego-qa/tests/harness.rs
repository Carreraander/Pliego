//! Arnés de comparación y oráculo de redacción: línea base (PDFium contra PDFium) y control negativo del oráculo.

use std::path::{Path, PathBuf};
use std::process::Command;

use pliego_core::JobContext;
use pliego_pdf::PdfDocument;
use pliego_qa::engines;
use pliego_qa::oracle::{OracleConfig, check_against_original, check_redacted};
use pliego_redact::{Area, Pattern, RedactJob, apply, find_patterns};

fn corpus(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus")
        .join(rel)
}

#[test]
fn baseline_pdfium_contra_pdfium_es_identico() {
    let out = tempfile::tempdir().unwrap();
    let json = out.path().join("r.json");
    let res = Command::new(env!("CARGO_BIN_EXE_engine-compare"))
        .arg(corpus(""))
        .args(["--max-pages", "1", "--timeout", "5", "--known"])
        .arg(corpus("engine-diff-known.txt"))
        .arg("--json")
        .arg(&json)
        .arg("--md")
        .arg(out.path().join("r.md"))
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&res.stdout);
    assert!(res.status.success(), "{text}");
    assert!(text.contains("inesperados: 0"), "{text}");
    // Es determinista: ningún archivo con diferencia de píxeles.
    let reports: Vec<serde_json::Value> = serde_json::from_slice(&std::fs::read(&json).unwrap()).unwrap();
    assert!(
        reports.len() >= 80,
        "el corpus debería tener decenas de PDF, hay {}",
        reports.len()
    );
    for r in &reports {
        for p in r["pages"].as_array().unwrap() {
            for d in p["diffs"].as_array().unwrap() {
                if let Some(d) = d[1].as_object() {
                    assert_eq!(d["diff_fraction"], 0.0, "{}", r["file"]);
                }
            }
        }
    }
}

#[test]
fn humo_de_robustez_no_cae() {
    let res = Command::new(env!("CARGO_BIN_EXE_engine-compare"))
        .arg(corpus("basic"))
        .args(["--mutate", "4", "--json"])
        .arg(tempfile::tempdir().unwrap().path().join("r.json"))
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&res.stdout);
    assert!(res.status.success(), "{text}");
    assert!(text.contains("0 caídas/bloqueos"), "{text}");
}

#[test]
fn oraculo_detecta_el_original_y_acepta_lo_redactado() {
    let engine = engines::by_name("pdfium").unwrap();
    let cfg = OracleConfig {
        needles: vec!["12345678Z".into(), "IBAN ES91 2100 0418".into()],
        regions: vec![],
    };

    // Control negativo: el original conserva los datos y el oráculo tiene que verlos por varias vías.
    let original = check_redacted(engine.as_ref(), &corpus("redact/datos.pdf"), &cfg).unwrap();
    assert!(original.iter().any(|f| f.check == "texto extraíble"), "{original:?}");
    assert!(
        original
            .iter()
            .any(|f| f.check == "literales de texto" || f.check == "bytes del archivo"),
        "{original:?}"
    );

    // Redactado de verdad: sin hallazgos.
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("redactado.pdf");
    let job = RedactJob {
        patterns: vec![Pattern::Dni, Pattern::Iban],
        ..Default::default()
    };
    apply(&corpus("redact/datos.pdf"), &out, None, &job, &JobContext::silent()).unwrap();
    let left = check_redacted(engine.as_ref(), &out, &cfg).unwrap();
    assert!(left.is_empty(), "{left:?}");
}

/// El oráculo es la verificación obligatoria del motor de redacción (W38): muchos tipos de dato, el extractor de texto
/// de PDFium, los literales de los flujos y los bytes del archivo.
#[test]
fn oraculo_acepta_lo_redactado_con_varios_tipos_de_dato() {
    let engine = engines::by_name("pdfium").unwrap();
    let datos = [
        "12345678Z",
        "X1234567L",
        "A58818501",
        "ES91 2100 0418 4502 0005 1332",
        "ana.lopez@ejemplo.es",
        "612 345 678",
        "12/03/1985",
    ];
    let cfg = OracleConfig {
        needles: datos.iter().map(|s| (*s).into()).collect(),
        regions: vec![],
    };
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("propio.pdf");
    let job = RedactJob {
        patterns: vec![
            Pattern::Dni,
            Pattern::Nie,
            Pattern::Cif,
            Pattern::Iban,
            Pattern::Email,
            Pattern::Phone,
            Pattern::Date,
        ],
        ..Default::default()
    };
    apply(&corpus("redact/datos.pdf"), &out, None, &job, &JobContext::silent()).unwrap();
    let left = check_redacted(engine.as_ref(), &out, &cfg).unwrap();
    assert!(left.is_empty(), "{left:?}");
    // Y el original, que sigue teniéndolo todo, no pasa (control negativo).
    let original = check_redacted(engine.as_ref(), &corpus("redact/datos.pdf"), &cfg).unwrap();
    assert!(!original.is_empty());
}

#[test]
fn oraculo_por_linea_de_ordenes() {
    let exe = env!("CARGO_BIN_EXE_redaction-oracle");
    let fail = Command::new(exe)
        .arg(corpus("redact/datos.pdf"))
        .args(["--needle", "12345678Z"])
        .output()
        .unwrap();
    assert_eq!(fail.status.code(), Some(1), "{}", String::from_utf8_lossy(&fail.stdout));
    let ok = Command::new(exe)
        .arg(corpus("redact/datos.pdf"))
        .args(["--needle", "dato-que-no-existe-9"])
        .output()
        .unwrap();
    assert!(ok.status.success());
}

fn rotated(dir: &Path, angle: i32) -> PathBuf {
    let doc = PdfDocument::open(corpus("redact/datos.pdf")).unwrap();
    for i in 0..doc.page_count().unwrap() {
        let mut page = doc.load_pdf_page(i).unwrap();
        page.set_rotation(angle).unwrap();
        page.update().unwrap();
    }
    let path = dir.join(format!("giro{angle}.pdf"));
    doc.save(&path).unwrap();
    path
}

const TODOS: [Pattern; 7] = [
    Pattern::Dni,
    Pattern::Nie,
    Pattern::Cif,
    Pattern::Iban,
    Pattern::Email,
    Pattern::Phone,
    Pattern::Date,
];

/// La redacción de todos los tipos de dato, en las cuatro orientaciones de página, deja exactamente el original menos
/// los datos: ni trozos de ellos ni vecinos perdidos (fallo real en 90°/270° que el oráculo no veía).
#[test]
fn oraculo_con_original_acepta_la_redaccion_en_paginas_giradas() {
    let engine = engines::by_name("pdfium").unwrap();
    for angle in [0, 90, 180, 270] {
        let dir = tempfile::tempdir().unwrap();
        let input = rotated(dir.path(), angle);
        let found = find_patterns(&PdfDocument::open(&input).unwrap(), &TODOS).unwrap();
        let needles: Vec<String> = found.iter().map(|m| m.text.clone()).collect();
        assert!(needles.len() >= 7, "giro {angle}: solo {needles:?}");
        let out = dir.path().join("redactado.pdf");
        let job = RedactJob {
            patterns: TODOS.to_vec(),
            ..Default::default()
        };
        apply(&input, &out, None, &job, &JobContext::silent()).unwrap();
        let cfg = OracleConfig {
            needles: needles.clone(),
            regions: vec![],
        };
        let left = check_redacted(engine.as_ref(), &out, &cfg).unwrap();
        assert!(left.is_empty(), "giro {angle}: {left:?}");
        let diff = check_against_original(engine.as_ref(), &input, &out, &needles).unwrap();
        assert!(diff.is_empty(), "giro {angle}: {diff:?}");
    }
}

/// Control negativo: una redacción corta (solo la mitad del DNI) pasa la búsqueda de la cadena entera, pero la
/// comparación con el original tiene que verla como resto parcial.
#[test]
fn oraculo_con_original_detecta_restos_parciales() {
    let engine = engines::by_name("pdfium").unwrap();
    let input = corpus("redact/datos.pdf");
    let found = find_patterns(&PdfDocument::open(&input).unwrap(), &[Pattern::Dni]).unwrap();
    let a = found[0].areas[0];
    let half = Area {
        x1: a.x0 + (a.x1 - a.x0) / 2.0,
        y0: a.y0 + (a.y1 - a.y0) * 0.25,
        y1: a.y1 - (a.y1 - a.y0) * 0.25,
        ..a
    };
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("corto.pdf");
    let job = RedactJob {
        areas: vec![half],
        ..Default::default()
    };
    apply(&input, &out, None, &job, &JobContext::silent()).unwrap();
    let needles = vec!["12345678Z".to_string()];
    // En la página 1 la cadena entera ya no está (el DNI también está, sin tocar, en la página 2), pero queda la
    // mitad del dato: solo la comparación palabra a palabra con el original lo ve.
    let page1 = engines::by_name("pdfium")
        .unwrap()
        .open(&out)
        .unwrap()
        .page_text(0)
        .unwrap();
    assert!(!page1.replace(' ', "").contains("12345678Z"), "{page1}");
    let diff = check_against_original(engine.as_ref(), &input, &out, &needles).unwrap();
    assert!(
        diff.iter()
            .any(|f| f.check == "resto parcial" && f.detail.starts_with("página 1:")),
        "{diff:?}"
    );
}

/// PDF de una página con un campo de texto relleno, con borde en relieve (marco claro/oscuro) en su apariencia.
fn formulario_con_relieve() -> Vec<u8> {
    let ap = "0.75 g 0 0 200 20 re f 1 g 0 0 200 2 re f 0 0 2 20 re f 0.4 g 0 18 200 2 re f 198 0 2 20 re f BT /Helv 12 Tf 0 g 4 6 Td (SECRETO-123) Tj ET";
    let objs: Vec<String> = vec![
        "<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [5 0 R] /DR << /Font << /Helv 4 0 R >> >> >> >>".into(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".into(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /Font << /Helv 4 0 R >> >> /Annots [5 0 R] >>"
            .into(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".into(),
        "<< /Type /Annot /Subtype /Widget /FT /Tx /T (nombre) /V (SECRETO-123) /Rect [50 700 250 720] /F 4 /P 3 0 R /DA (/Helv 12 Tf 0 g) /AP << /N 6 0 R >> >>".into(),
        format!(
            "<< /Type /XObject /Subtype /Form /BBox [0 0 200 20] /Resources << /Font << /Helv 4 0 R >> >> /Length {} >>\nstream\n{ap}\nendstream",
            ap.len()
        ),
    ];
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offs = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offs.push(out.len());
        out.extend(format!("{} 0 obj\n{o}\nendobj\n", i + 1).bytes());
    }
    let xref = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).bytes());
    for o in offs {
        out.extend(format!("{o:010} 00000 n \n").bytes());
    }
    out.extend(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objs.len() + 1
        )
        .bytes(),
    );
    out
}

#[test]
fn oraculo_acepta_un_campo_de_formulario_con_relieve_redactado() {
    let engine = engines::by_name("pdfium").unwrap();
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("form.pdf");
    std::fs::write(&input, formulario_con_relieve()).unwrap();
    // El campo en coordenadas de página (y hacia abajo): x 50..250, y 122..142.
    let area = Area {
        page: 0,
        x0: 50.0,
        y0: 122.0,
        x1: 250.0,
        y1: 142.0,
    };
    let out = dir.path().join("salida.pdf");
    let job = RedactJob {
        areas: vec![area],
        ..Default::default()
    };
    apply(&input, &out, None, &job, &JobContext::silent()).unwrap();
    let cfg = OracleConfig {
        needles: vec!["SECRETO-123".into()],
        regions: vec![pliego_qa::oracle::Region {
            page: 0,
            x0: 50.0,
            y0: 122.0,
            x1: 250.0,
            y1: 142.0,
        }],
    };
    let left = check_redacted(engine.as_ref(), &out, &cfg).unwrap();
    assert!(left.is_empty(), "{left:?}");
}
