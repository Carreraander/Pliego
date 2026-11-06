use std::path::PathBuf;
use std::process::Command;

fn pliego() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_pliego"));
    c.env("PLIEGO_LANG", "es");
    c
}

#[test]
fn create_list_extract() {
    let corpus = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus");
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("c.pdf");
    let r = pliego()
        .args(["portfolio", "create", "-o"])
        .arg(&out)
        .arg(corpus.join("organize/a.pdf"))
        .arg(corpus.join("compare/base.pdf"))
        .output()
        .unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));

    let r = pliego()
        .args(["--json", "portfolio", "list"])
        .arg(&out)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&r.stdout).unwrap();
    assert_eq!(v["is_portfolio"], true);
    assert_eq!(v["entries"].as_array().unwrap().len(), 2);

    let dest = dir.path().join("fuera");
    let r = pliego()
        .args(["portfolio", "extract"])
        .arg(&out)
        .arg("-o")
        .arg(&dest)
        .arg("base.pdf")
        .output()
        .unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    assert!(dest.join("base.pdf").exists() && !dest.join("a.pdf").exists());
}
