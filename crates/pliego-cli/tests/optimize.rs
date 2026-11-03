use std::path::PathBuf;
use std::process::Command;

fn corpus(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/compare")
        .join(name)
}

fn pliego() -> Command {
    Command::new(env!("CARGO_BIN_EXE_pliego"))
}

#[test]
fn analyze_json_lists_categories() {
    let out = pliego()
        .args(["--json", "optimize", "--analyze"])
        .arg(corpus("imagen_a.pdf"))
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["image_count"], 1);
    assert!(v["total_bytes"].as_u64().unwrap() > 0);
}

#[test]
fn optimize_writes_output_and_never_touches_input() {
    let dir = tempfile::tempdir().unwrap();
    let out_pdf = dir.path().join("o.pdf");
    let before = std::fs::read(corpus("imagen_a.pdf")).unwrap();
    let out = pliego()
        .args(["optimize", "--preset", "sin-perdida", "-o"])
        .arg(&out_pdf)
        .arg(corpus("imagen_a.pdf"))
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Antes") && stdout.contains("Después"), "{stdout}");
    assert!(std::fs::read(&out_pdf).unwrap().starts_with(b"%PDF-"));
    assert_eq!(std::fs::read(corpus("imagen_a.pdf")).unwrap(), before);
}

#[test]
fn optimize_needs_an_output_and_rejects_unknown_presets() {
    let out = pliego().arg("optimize").arg(corpus("base.pdf")).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("falta el archivo de salida"));
    let out = pliego()
        .args(["optimize", "--preset", "otro", "-o", "x.pdf"])
        .arg(corpus("base.pdf"))
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("preajuste desconocido"));
}
