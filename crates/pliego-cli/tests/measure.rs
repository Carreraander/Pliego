use std::path::PathBuf;
use std::process::Command;

fn pliego() -> Command {
    Command::new(env!("CARGO_BIN_EXE_pliego"))
}

#[test]
fn scale_add_and_list() {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/organize/a.pdf");
    let dir = tempfile::tempdir().unwrap();
    let (a, b) = (dir.path().join("a.pdf"), dir.path().join("b.pdf"));

    // Sin escala no se puede medir.
    let r = pliego()
        .args(["measure", "add"])
        .arg(&src)
        .arg("-o")
        .arg(&a)
        .args(["--points", "0,0 100,0"])
        .output()
        .unwrap();
    assert!(!r.status.success());
    assert!(String::from_utf8_lossy(&r.stderr).contains("no tiene escala"));

    let r = pliego()
        .args(["measure", "scale"])
        .arg(&src)
        .arg("-o")
        .arg(&a)
        .args(["--paper", "1cm", "--real", "5m"])
        .output()
        .unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));

    // 2 cm de papel = 10 m.
    let r = pliego()
        .args(["measure", "add"])
        .arg(&a)
        .arg("-o")
        .arg(&b)
        .args(["--points", "0,0 56.692913,0"])
        .output()
        .unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    assert_eq!(String::from_utf8_lossy(&r.stdout).trim(), "10,00 m");

    let r = pliego().args(["--json", "measure", "list"]).arg(&b).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&r.stdout).unwrap();
    assert_eq!(v["scales"].as_array().unwrap().len(), 3);
    assert_eq!(v["measurements"].as_array().unwrap().len(), 1);
}
