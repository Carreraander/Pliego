use std::path::PathBuf;
use std::process::Command;

fn corpus(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/print")
        .join(name)
}

fn pliego() -> Command {
    Command::new(env!("CARGO_BIN_EXE_pliego"))
}

#[test]
fn inks_lists_spot_colors() {
    let out = pliego()
        .args(["print", "inks"])
        .arg(corpus("directo.pdf"))
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("Barniz") && text.contains("directo") && text.contains("Black"),
        "{text}"
    );
}

#[test]
fn preflight_fails_on_defective_file_and_reports_json() {
    let out = pliego()
        .args(["--json", "print", "preflight"])
        .arg(corpus("defectos.pdf"))
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "un archivo con errores debe devolver código distinto de 0"
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        v["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["code"] == "font_not_embedded")
    );

    let ok = pliego()
        .args(["print", "preflight"])
        .arg(corpus("limpio.pdf"))
        .output()
        .unwrap();
    assert!(ok.status.success(), "{}", String::from_utf8_lossy(&ok.stderr));
}

#[test]
fn separations_write_four_plates() {
    let dir = tempfile::tempdir().unwrap();
    let out = pliego()
        .args(["print", "separations"])
        .arg(corpus("cmyk-cian.pdf"))
        .arg("-o")
        .arg(dir.path())
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    for ch in ["cyan", "magenta", "yellow", "black"] {
        let f = dir.path().join(format!("cmyk-cian-p1-{ch}.png"));
        let bytes = std::fs::read(&f).unwrap();
        assert_eq!(&bytes[1..4], b"PNG", "{ch}");
    }
    assert!(String::from_utf8_lossy(&out.stdout).contains("Cyan"));
}
