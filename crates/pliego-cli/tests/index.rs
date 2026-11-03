use std::path::PathBuf;
use std::process::Command;

fn corpus(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus")
        .join(name)
}

fn pliego() -> Command {
    Command::new(env!("CARGO_BIN_EXE_pliego"))
}

#[test]
fn index_add_and_search() {
    let dir = tempfile::tempdir().unwrap();
    let docs = dir.path().join("docs");
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::copy(corpus("compare/base.pdf"), docs.join("contrato.pdf")).unwrap();
    let idx = dir.path().join("idx");

    let r = pliego().args(["index", "add"]).arg(&idx).arg(&docs).output().unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    assert!(String::from_utf8_lossy(&r.stdout).contains("1 indexados"));

    let r = pliego()
        .args(["--json", "index", "search"])
        .arg(&idx)
        .arg("testigos")
        .output()
        .unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let v: serde_json::Value = serde_json::from_slice(&r.stdout).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 1);
    assert_eq!(v[0]["page"], 0);
    assert!(v[0]["path"].as_str().unwrap().ends_with("contrato.pdf"));
}
