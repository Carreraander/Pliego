use std::path::PathBuf;
use std::process::Command;

fn corpus() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/edit/parrafos.pdf")
}

fn pliego() -> Command {
    Command::new(env!("CARGO_BIN_EXE_pliego"))
}

#[test]
fn blocks_replace_y_text() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("editado.pdf");

    let r = pliego()
        .args(["--json", "edit", "blocks"])
        .arg(corpus())
        .output()
        .unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let v: serde_json::Value = serde_json::from_slice(&r.stdout).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 2);
    let id = v[0]["id"].as_u64().unwrap().to_string();

    let r = pliego()
        .args(["--json", "edit", "replace"])
        .arg(corpus())
        .args([
            "--block",
            &id,
            "--text",
            "Hola desde la CLI",
            "--color",
            "#0000ff",
            "-o",
        ])
        .arg(&out)
        .output()
        .unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let v: serde_json::Value = serde_json::from_slice(&r.stdout).unwrap();
    assert!(!v["warnings"].as_array().unwrap().is_empty());

    let r = pliego().args(["text"]).arg(&out).output().unwrap();
    let t = String::from_utf8_lossy(&r.stdout);
    assert!(t.contains("Hola desde la CLI"), "{t}");
    assert!(!t.contains("Primer parrafo"), "{t}");
}

#[test]
fn sin_salida_no_sobrescribe() {
    let r = pliego()
        .args(["edit", "insert-text"])
        .arg(corpus())
        .args(["--rect", "10,10,100,50", "--text", "x"])
        .output()
        .unwrap();
    assert!(!r.status.success());
    assert!(String::from_utf8_lossy(&r.stderr).contains("-o"));
}

#[test]
fn enlace_uri() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("enlace.pdf");
    let r = pliego()
        .args(["edit", "link"])
        .arg(corpus())
        .args(["--rect", "72,100,200,120", "--uri", "https://example.org", "-o"])
        .arg(&out)
        .output()
        .unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    assert!(out.exists());
}
