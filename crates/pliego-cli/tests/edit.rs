use std::path::PathBuf;
use std::process::Command;

fn corpus() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/edit/parrafos.pdf")
}

fn pliego() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_pliego"));
    c.env("PLIEGO_LANG", "es");
    c
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

#[test]
fn imagenes_listar_mover_borrar() {
    let dir = tempfile::tempdir().unwrap();
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/edit/imagenes.pdf");
    let out = dir.path().join("m.pdf");
    let r = pliego()
        .args(["edit", "image-move"])
        .arg(&base)
        .args(["--id", "0", "--dx", "-10", "--rotate", "90", "-o"])
        .arg(&out)
        .output()
        .unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let out2 = dir.path().join("b.pdf");
    let r = pliego()
        .args(["edit", "image-delete"])
        .arg(&out)
        .args(["--id", "1", "-o"])
        .arg(&out2)
        .output()
        .unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let r = pliego().args(["--json", "edit", "images"]).arg(&out2).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&r.stdout).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 1);
}

#[test]
fn vectores_listar_y_borrar() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("v.pdf");
    let r = pliego()
        .args(["--json", "edit", "vectors"])
        .arg(corpus())
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&r.stdout).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 1);
    let r = pliego()
        .args(["edit", "vector-delete"])
        .arg(corpus())
        .args(["--id", "0", "-o"])
        .arg(&out)
        .output()
        .unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let r = pliego().args(["--json", "edit", "vectors"]).arg(&out).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&r.stdout).unwrap();
    assert!(v.as_array().unwrap().is_empty());
}
