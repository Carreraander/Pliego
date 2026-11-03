use std::path::PathBuf;
use std::process::Command;

fn corpus() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/basic/hola.pdf")
}

fn pliego() -> Command {
    Command::new(env!("CARGO_BIN_EXE_pliego"))
}

#[test]
fn add_list_delete_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let spec = dir.path().join("spec.json");
    std::fs::write(
        &spec,
        r#"{"page":0,"geometry":{"type":"square","rect":[50,50,150,120]},
            "style":{"color":[1,0,0],"opacity":0.5},"author":"Ana","contents":"Revisar"}"#,
    )
    .unwrap();
    let out = dir.path().join("anotado.pdf");

    let r = pliego()
        .args(["--json", "annot", "add"])
        .arg(corpus())
        .args(["--spec"])
        .arg(&spec)
        .args(["-p", "1", "-o"])
        .arg(&out)
        .output()
        .unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let v: serde_json::Value = serde_json::from_slice(&r.stdout).unwrap();
    let xref = v["id"]["xref"].as_i64().unwrap();

    let r = pliego().args(["--json", "annot", "list"]).arg(&out).output().unwrap();
    assert!(r.status.success());
    let v: serde_json::Value = serde_json::from_slice(&r.stdout).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 1);
    assert_eq!(v[0]["kind"], "square");
    assert_eq!(v[0]["author"], "Ana");

    let out2 = dir.path().join("limpio.pdf");
    let r = pliego()
        .args(["annot", "delete"])
        .arg(&out)
        .args(["-p", "1", "--id", &xref.to_string(), "-o"])
        .arg(&out2)
        .output()
        .unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let r = pliego().args(["--json", "annot", "list"]).arg(&out2).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&r.stdout).unwrap();
    assert!(v.as_array().unwrap().is_empty());
}

#[test]
fn refuses_to_overwrite_input_by_default() {
    let dir = tempfile::tempdir().unwrap();
    let spec = dir.path().join("s.json");
    std::fs::write(&spec, r#"{"page":0,"geometry":{"type":"note","at":[10,10]}}"#).unwrap();
    let r = pliego()
        .args(["annot", "add"])
        .arg(corpus())
        .arg("--spec")
        .arg(&spec)
        .output()
        .unwrap();
    assert!(!r.status.success());
}
