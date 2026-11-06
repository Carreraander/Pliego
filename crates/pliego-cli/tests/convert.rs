use std::path::PathBuf;
use std::process::Command;

fn corpus() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/basic/hola.pdf")
}

fn pliego() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_pliego"));
    c.env("PLIEGO_LANG", "es");
    c
}

#[test]
fn export_to_png_then_create_pdf_from_them() {
    let dir = tempfile::tempdir().unwrap();
    let imgs = dir.path().join("imgs");
    let out = pliego()
        .arg("export")
        .arg(corpus())
        .args(["--to", "png", "--dpi", "36", "-o"])
        .arg(&imgs)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let (p1, p2) = (imgs.join("hola-001.png"), imgs.join("hola-002.png"));
    assert!(p1.exists() && p2.exists());

    let pdf = dir.path().join("nuevo.pdf");
    let out = pliego()
        .args(["--json", "create"])
        .args([&p1, &p2])
        .args(["--page-size", "a4", "-o"])
        .arg(&pdf)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["outputs"].as_array().unwrap().len(), 1);

    let out = pliego().args(["--json", "info"]).arg(&pdf).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["page_count"], 2);
}

#[test]
fn export_to_text() {
    let dir = tempfile::tempdir().unwrap();
    let txt = dir.path().join("hola.txt");
    let out = pliego()
        .arg("export")
        .arg(corpus())
        .args(["--to", "txt", "-o"])
        .arg(&txt)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(std::fs::read_to_string(txt).unwrap().contains("Hola Pliego"));
}

#[test]
fn create_with_missing_file_fails_cleanly() {
    let dir = tempfile::tempdir().unwrap();
    let out = pliego()
        .args(["create", "no-existe.png", "-o"])
        .arg(dir.path().join("x.pdf"))
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(!dir.path().join("x.pdf").exists());
}

#[test]
fn export_html_and_json() {
    let dir = tempfile::tempdir().unwrap();
    let html = dir.path().join("o.html");
    let out = pliego()
        .arg("export")
        .arg(corpus())
        .args(["--to", "html", "-o"])
        .arg(&html)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(std::fs::read_to_string(&html).unwrap().contains("Hola"));

    let json = dir.path().join("o.json");
    let out = pliego()
        .arg("export")
        .arg(corpus())
        .args(["--to", "json", "-o"])
        .arg(&json)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let v: serde_json::Value = serde_json::from_slice(&std::fs::read(&json).unwrap()).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 2);
}

#[test]
fn create_from_markdown() {
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("n.md");
    std::fs::write(&md, "# Hola\n\nTexto de prueba.\n").unwrap();
    let pdf = dir.path().join("n.pdf");
    let out = pliego().arg("create").arg(&md).arg("-o").arg(&pdf).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let out = pliego().arg("text").arg(&pdf).output().unwrap();
    assert!(String::from_utf8_lossy(&out.stdout).contains("Texto de prueba"));
}

#[test]
fn create_from_office_without_backend_or_with_it() {
    let dir = tempfile::tempdir().unwrap();
    let csv = dir.path().join("d.csv");
    std::fs::write(&csv, "a,b\n1,2\n").unwrap();
    let pdf = dir.path().join("d.pdf");
    let out = pliego().arg("create").arg(&csv).arg("-o").arg(&pdf).output().unwrap();
    // Con cualquier backend (Word/Excel, LibreOffice) debe funcionar; sin ninguno, el error lo explica.
    if pliego_convert::office_backend().is_some() {
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        assert!(pdf.exists());
    } else {
        assert!(!out.status.success());
        assert!(String::from_utf8_lossy(&out.stderr).contains("LibreOffice"));
    }
}

fn package(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/packages")
        .join(name)
}

#[test]
fn create_pdf_from_cbz_epub_and_xps() {
    let dir = tempfile::tempdir().unwrap();
    for (file, pages) in [("mini.cbz", 3), ("mini.epub", 2), ("mini.xps", 1)] {
        let pdf = dir.path().join(format!("{file}.pdf"));
        let out = pliego()
            .args(["--json", "create"])
            .arg(package(file))
            .arg("-o")
            .arg(&pdf)
            .output()
            .unwrap();
        assert!(out.status.success(), "{file}: {}", String::from_utf8_lossy(&out.stderr));
        let out = pliego().args(["--json", "info"]).arg(&pdf).output().unwrap();
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(v["page_count"], pages, "{file}");
    }
}

#[test]
fn packaged_formats_are_converted_one_at_a_time() {
    let dir = tempfile::tempdir().unwrap();
    let out = pliego()
        .arg("create")
        .arg(package("mini.cbz"))
        .arg(package("mini.cbz"))
        .arg("-o")
        .arg(dir.path().join("x.pdf"))
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("de uno en uno"));
}
