use std::path::PathBuf;
use std::process::Command;

fn corpus(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/compare")
        .join(name)
}

fn pliego() -> Command {
    Command::new(env!("CARGO_BIN_EXE_pliego"))
}

const ACTION: &str = r#"
name = "Archivar"

[[steps]]
kind = "optimize"
preset = "sin-perdida"

[[steps]]
kind = "pages.rotate"
degrees = 90

[[steps]]
kind = "protect.encrypt"
user_password = "clave"
"#;

#[test]
fn run_applies_three_steps_to_a_folder_and_resumes() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("entrada");
    std::fs::create_dir_all(src.join("sub")).unwrap();
    std::fs::copy(corpus("imagen_a.pdf"), src.join("a.pdf")).unwrap();
    std::fs::copy(corpus("imagen_a.pdf"), src.join("sub/b.pdf")).unwrap();
    let action = dir.path().join("accion.toml");
    std::fs::write(&action, ACTION).unwrap();
    let out = dir.path().join("salida");

    let run = |extra: &[&str]| {
        pliego()
            .args(["--json", "run"])
            .arg(&action)
            .arg(&src)
            .arg("-o")
            .arg(&out)
            .args(extra)
            .output()
            .unwrap()
    };
    let r = run(&[]);
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let v: serde_json::Value = serde_json::from_slice(&r.stdout).unwrap();
    assert_eq!(v["processed"], 2);
    assert!(out.join("a.pdf").exists() && out.join("sub/b.pdf").exists());

    // El resultado está cifrado: sin contraseña no se abre.
    let info = pliego().args(["info"]).arg(out.join("a.pdf")).output().unwrap();
    assert!(!info.status.success() || String::from_utf8_lossy(&info.stdout).contains("AES"));

    let r = run(&["--resume"]);
    let v: serde_json::Value = serde_json::from_slice(&r.stdout).unwrap();
    assert_eq!((v["processed"].as_u64(), v["resumed"].as_u64()), (Some(0), Some(2)));
}

#[test]
fn run_rejects_unknown_step_and_output_over_input() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("entrada");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::copy(corpus("imagen_a.pdf"), src.join("a.pdf")).unwrap();
    let action = dir.path().join("a.toml");

    std::fs::write(&action, "[[steps]]\nkind = \"inventado\"\n").unwrap();
    let r = pliego()
        .arg("run")
        .arg(&action)
        .arg(&src)
        .arg("-o")
        .arg(dir.path().join("o"))
        .output()
        .unwrap();
    assert!(!r.status.success());
    assert!(String::from_utf8_lossy(&r.stderr).contains("paso desconocido"));

    std::fs::write(&action, "[[steps]]\nkind = \"pages.rotate\"\ndegrees = 90\n").unwrap();
    let before = std::fs::read(src.join("a.pdf")).unwrap();
    let r = pliego()
        .arg("run")
        .arg(&action)
        .arg(&src)
        .arg("-o")
        .arg(&src)
        .output()
        .unwrap();
    assert!(!r.status.success());
    assert_eq!(std::fs::read(src.join("a.pdf")).unwrap(), before);
}

#[test]
fn list_steps_names_the_builtin_steps() {
    let r = pliego().args(["run", "--list-steps"]).output().unwrap();
    let s = String::from_utf8_lossy(&r.stdout);
    for k in ["optimize", "protect.encrypt", "stamp.watermark", "pages.rotate"] {
        assert!(s.contains(k), "{s}");
    }
}

#[test]
fn bundled_example_runs() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("a.pdf");
    std::fs::copy(corpus("imagen_a.pdf"), &src).unwrap();
    let example = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../pliego-actions/examples/archivar.toml");
    let r = pliego()
        .arg("run")
        .arg(example)
        .arg(&src)
        .arg("-o")
        .arg(dir.path().join("o"))
        .output()
        .unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    assert!(dir.path().join("o/a.pdf").exists());
}
