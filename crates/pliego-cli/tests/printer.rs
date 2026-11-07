use std::path::PathBuf;
use std::process::Command;

fn pliego() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_pliego"));
    c.env_remove("RUST_BACKTRACE");
    c.env("PLIEGO_LANG", "es");
    c
}

fn hola() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/basic/hola.pdf")
}

fn stderr(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn rejects_invalid_options_before_touching_any_printer() {
    let out = pliego()
        .args(["printer", "send"])
        .arg(hola())
        .args(["--copies", "0"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(stderr(&out).contains("las copias van de 1 a 999"), "{}", stderr(&out));
    let out = pliego()
        .args(["printer", "send"])
        .arg(hola())
        .args(["--scale", "custom", "--scale-percent", "5"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(stderr(&out).contains("la escala va del 10"), "{}", stderr(&out));
    let out = pliego()
        .args(["printer", "send"])
        .arg(hola())
        .args(["--pages", "x-y"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(stderr(&out).contains("páginas no válidas"), "{}", stderr(&out));
}

#[test]
fn rejects_incompatible_sheet_layouts() {
    let run = |extra: &[&str]| {
        let out = pliego()
            .args(["printer", "send"])
            .arg(hola())
            .args(extra)
            .output()
            .unwrap();
        assert!(!out.status.success());
        stderr(&out)
    };
    assert!(run(&["--per-sheet", "3"]).contains("las páginas por hoja pueden ser"));
    assert!(run(&["--booklet", "both", "--per-sheet", "2"]).contains("el folleto ya pone dos páginas"));
    assert!(run(&["--booklet", "front", "--subset", "odd"]).contains("anversos o los reversos"));
}

#[test]
fn list_prints_json_on_windows_and_explains_elsewhere() {
    let out = pliego().args(["--json", "printer", "list"]).output().unwrap();
    if cfg!(windows) {
        assert!(out.status.success(), "{}", stderr(&out));
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert!(v.is_array());
    } else {
        assert!(!out.status.success());
        assert!(
            stderr(&out).contains("solo está disponible en Windows"),
            "{}",
            stderr(&out)
        );
    }
}
