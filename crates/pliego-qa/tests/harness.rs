//! Arnés de comparación y oráculo de redacción: línea base (MuPDF contra MuPDF) y control negativo del oráculo.

use std::path::{Path, PathBuf};
use std::process::Command;

use pliego_core::JobContext;
use pliego_qa::engines;
use pliego_qa::oracle::{OracleConfig, check_redacted};
use pliego_redact::{Pattern, RedactEngine, RedactJob, apply, apply_with_engine};

fn corpus(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus")
        .join(rel)
}

#[test]
fn baseline_mupdf_contra_mupdf_es_identico() {
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
    let engine = engines::by_name("mupdf").unwrap();
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

/// El oráculo es la verificación obligatoria del motor propio de redacción (W38): mismos datos, los dos motores, y
/// los dos extractores de texto (MuPDF y PDFium si hay DLL) más los bytes del archivo.
#[test]
fn oraculo_acepta_lo_redactado_con_el_motor_propio() {
    let engine = engines::by_name("mupdf").unwrap();
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
    apply_with_engine(
        &corpus("redact/datos.pdf"),
        &out,
        None,
        &job,
        &JobContext::silent(),
        RedactEngine::Native,
    )
    .unwrap();
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
