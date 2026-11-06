use std::path::PathBuf;
use std::process::Command;

fn corpus(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../../tests/corpus/office/{name}.pdf"))
}

fn pliego() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_pliego"));
    c.env("PLIEGO_LANG", "es");
    c
}

#[test]
fn office_exports_all_three_formats() {
    let dir = tempfile::tempdir().unwrap();
    for (fmt, ext) in [("docx", "docx"), ("xlsx", "xlsx"), ("pptx", "pptx")] {
        let out = dir.path().join(format!("factura.{ext}"));
        let r = pliego()
            .args(["--json", "office", fmt])
            .arg(corpus("factura-1"))
            .arg("-o")
            .arg(&out)
            .output()
            .unwrap();
        assert!(r.status.success(), "{fmt}: {}", String::from_utf8_lossy(&r.stderr));
        let v: serde_json::Value = serde_json::from_slice(&r.stdout).unwrap();
        assert_eq!(v["outputs"].as_array().unwrap().len(), 1);
        let bytes = std::fs::read(&out).unwrap();
        assert_eq!(&bytes[..2], b"PK", "{fmt} debe ser un ZIP");
    }
}

#[test]
fn scanned_pdf_warns_or_fails() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("e.docx");
    let r = pliego()
        .args(["office", "docx"])
        .arg(corpus("escaneado"))
        .arg("-o")
        .arg(&out)
        .output()
        .unwrap();
    assert!(r.status.success());
    assert!(String::from_utf8_lossy(&r.stderr).contains("OCR"));
    let out2 = dir.path().join("e2.docx");
    let r = pliego()
        .args(["office", "docx", "--fail-on-scanned"])
        .arg(corpus("escaneado"))
        .arg("-o")
        .arg(&out2)
        .output()
        .unwrap();
    assert!(!r.status.success());
    assert!(!out2.exists());
}
