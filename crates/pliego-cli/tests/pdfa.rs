use std::path::PathBuf;
use std::process::Command;

fn corpus(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus")
        .join(rel)
}

fn pliego() -> Command {
    Command::new(env!("CARGO_BIN_EXE_pliego"))
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
