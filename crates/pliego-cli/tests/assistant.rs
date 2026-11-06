use std::path::PathBuf;
use std::process::Command;

fn corpus() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/basic/hola.pdf")
}

fn pliego() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_pliego"));
    c.env("PLIEGO_LANG", "es");
    c
}

#[test]
fn models_lists_only_open_licenses() {
    let out = pliego().args(["--json", "assistant", "models"]).output().unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let models = v.as_array().unwrap();
    assert!(!models.is_empty());
    for m in models {
        assert!(matches!(m["license"].as_str().unwrap(), "Apache-2.0" | "MIT"));
    }
}

#[test]
fn passages_finds_text_with_its_page() {
    let out = pliego()
        .args(["--json", "assistant", "passages"])
        .arg(corpus())
        .arg("Hola Pliego")
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v[0]["page"], 1);
    assert!(v[0]["text"].as_str().unwrap().contains("Hola Pliego"));
}

#[test]
fn ask_without_engine_fails_cleanly() {
    let out = pliego()
        .args(["assistant", "ask"])
        .arg(corpus())
        .arg("¿qué dice?")
        .args(["--model", "no-existe.gguf"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("modelo"));
}
