use std::path::PathBuf;
use std::process::Command;

use pliego_ocr::available_languages;

fn pliego() -> Command {
    Command::new(env!("CARGO_BIN_EXE_pliego"))
}

fn hola() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/basic/hola.pdf")
}

#[test]
fn list_languages_json() {
    let out = pliego().args(["--json", "ocr", "--list-languages"]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(v.is_array());
}

#[test]
fn skip_and_force_conflict() {
    let out = pliego()
        .arg("ocr")
        .arg(hola())
        .args(["--skip-text", "--force"])
        .output()
        .unwrap();
    assert!(!out.status.success());
}

#[test]
fn skips_pages_with_text_and_writes_output() {
    if !available_languages().iter().any(|l| l == "eng") {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("o.pdf");
    let out = pliego()
        .arg("--json")
        .arg("ocr")
        .arg(hola())
        .args(["--lang", "eng", "-o"])
        .arg(&dest)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(v["summary"].as_str().unwrap().contains("0 página"), "{v}");
    assert!(dest.exists());
}

#[test]
fn refuses_to_overwrite_input() {
    let out = pliego().arg("ocr").arg(hola()).arg("-o").arg(hola()).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("no puede ser el de entrada"));
}

#[test]
fn missing_model_is_explained() {
    let dir = tempfile::tempdir().unwrap();
    let out = pliego()
        .arg("ocr")
        .arg(hola())
        .args(["--force", "--lang", "zzz", "--tessdata"])
        .arg(dir.path())
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("Tesseract no pudo cargar"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
