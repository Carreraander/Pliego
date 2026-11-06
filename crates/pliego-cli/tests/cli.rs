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
fn info_json() {
    let out = pliego().args(["--json", "info"]).arg(corpus()).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["page_count"], 2);
}

#[test]
fn render_writes_pngs() {
    let dir = tempfile::tempdir().unwrap();
    let out = pliego()
        .arg("render")
        .arg(corpus())
        .args(["--pages", "all", "--dpi", "36", "-o"])
        .arg(dir.path())
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(dir.path().join("hola-001.png").exists());
    assert!(dir.path().join("hola-002.png").exists());
}

#[test]
fn bad_page_fails_cleanly() {
    let out = pliego()
        .arg("text")
        .arg(corpus())
        .args(["--pages", "9"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("no existe"));
}
