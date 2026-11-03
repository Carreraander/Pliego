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

#[test]
fn hairlines_then_preflight_is_clean() {
    let dir = tempfile::tempdir().unwrap();
    let fixed = dir.path().join("fino.pdf");
    let out = pliego()
        .args(["print", "hairlines"])
        .arg(corpus("defectos.pdf"))
        .arg("-o")
        .arg(&fixed)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stdout).contains("1 línea"));
    let pf = pliego()
        .args(["--json", "print", "preflight", "--profile", "basic"])
        .arg(&fixed)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&pf.stdout).unwrap();
    assert!(
        !v["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["code"] == "hair_line")
    );
}

#[test]
fn marks_command_and_bad_pages() {
    let dir = tempfile::tempdir().unwrap();
    let out_pdf = dir.path().join("marcas.pdf");
    let out = pliego()
        .args(["print", "marks"])
        .arg(corpus("limpio.pdf"))
        .arg("-o")
        .arg(&out_pdf)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(out_pdf.exists());
    let bad = pliego()
        .args(["print", "marks", "--pages", "4"])
        .arg(corpus("limpio.pdf"))
        .arg("-o")
        .arg(dir.path().join("x.pdf"))
        .output()
        .unwrap();
    assert!(!bad.status.success());
}

#[test]
fn pdfx_without_valid_profile_fails_clearly() {
    let dir = tempfile::tempdir().unwrap();
    let fake = dir.path().join("no.icc");
    std::fs::write(&fake, b"no es un perfil").unwrap();
    let out = pliego()
        .args(["print", "pdfx"])
        .arg(corpus("limpio.pdf"))
        .arg("-o")
        .arg(dir.path().join("x.pdf"))
        .arg("--icc")
        .arg(&fake)
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("perfil ICC"));
}
