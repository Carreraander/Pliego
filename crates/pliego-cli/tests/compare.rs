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
fn compare_json_reports_changed_word() {
    let out = pliego()
        .args(["--json", "compare"])
        .arg(corpus("base.pdf"))
        .arg(corpus("palabra.pdf"))
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["summary"]["changed_pages"], 1);
    assert_eq!(v["pages"][0]["text_changes"][0]["text_b"], "sesenta");
}

#[test]
fn compare_writes_pdf_report_and_json_file() {
    let dir = tempfile::tempdir().unwrap();
    let pdf = dir.path().join("informe.pdf");
    let out = pliego()
        .arg("compare")
        .arg(corpus("base.pdf"))
        .arg(corpus("pagina.pdf"))
        .arg("-o")
        .arg(&pdf)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stdout).contains("página insertada"));
    assert!(std::fs::read(&pdf).unwrap().starts_with(b"%PDF-"));

    let json = dir.path().join("informe.json");
    let out = pliego()
        .arg("compare")
        .arg(corpus("base.pdf"))
        .arg(corpus("base.pdf"))
        .arg("-o")
        .arg(&json)
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("Sin diferencias"));
    let v: serde_json::Value = serde_json::from_slice(&std::fs::read(&json).unwrap()).unwrap();
    assert_eq!(v["summary"]["changed_pages"], 0);
}

#[test]
fn compare_missing_file_fails_cleanly() {
    let out = pliego()
        .arg("compare")
        .arg(corpus("nada.pdf"))
        .arg(corpus("base.pdf"))
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("no se pudieron comparar"));
}
