use std::path::PathBuf;
use std::process::Command;

fn corpus(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus")
        .join(rel)
}

fn pliego() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_pliego"));
    c.env("PLIEGO_LANG", "es");
    c
}

#[test]
fn plain_pdf_is_not_pdfa() {
    let out = pliego()
        .args(["pdfa", "validate"])
        .arg(corpus("basic/hola.pdf"))
        .output()
        .unwrap();
    assert!(!out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("no cumple PDF/A-2b") && text.contains("XMP"), "{text}");
}

#[test]
fn json_output_and_levels() {
    let out = pliego()
        .args(["--json", "pdfa", "validate", "--level", "1b"])
        .arg(corpus("basic/hola.pdf"))
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["level"], "1b");
    assert!(
        v["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["rule"] == "no_output_intent")
    );
    let bad = pliego()
        .args(["pdfa", "validate", "--level", "9z"])
        .arg(corpus("basic/hola.pdf"))
        .output()
        .unwrap();
    assert!(!bad.status.success());
}

#[test]
fn convert_then_validate() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("a.pdf");
    let conv = pliego()
        .args(["pdfa", "convert"])
        .arg(corpus("basic/hola.pdf"))
        .arg("-o")
        .arg(&out)
        .args(["--level", "3b"])
        .output()
        .unwrap();
    assert!(conv.status.success(), "{}", String::from_utf8_lossy(&conv.stderr));
    assert!(String::from_utf8_lossy(&conv.stdout).contains("Convertido a PDF/A-3b"));
    let ok = pliego()
        .args(["pdfa", "validate", "--level", "3b"])
        .arg(&out)
        .output()
        .unwrap();
    assert!(ok.status.success(), "{}", String::from_utf8_lossy(&ok.stdout));
    // el texto se conserva
    let text = pliego().arg("text").arg(&out).output().unwrap();
    assert!(String::from_utf8_lossy(&text.stdout).contains("Hola Pliego"));
    // y no sirve para otro nivel
    let other = pliego()
        .args(["pdfa", "validate", "--level", "1b"])
        .arg(&out)
        .output()
        .unwrap();
    assert!(!other.status.success());
}

#[test]
fn convert_does_not_overwrite_input() {
    let p = corpus("basic/hola.pdf");
    let out = pliego()
        .args(["pdfa", "convert"])
        .arg(&p)
        .arg("-o")
        .arg(&p)
        .output()
        .unwrap();
    assert!(!out.status.success());
}
