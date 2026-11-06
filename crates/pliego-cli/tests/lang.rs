//! Idioma de la CLI: `PLIEGO_LANG` (o el idioma del sistema) elige el de la ayuda y los mensajes.
use std::path::PathBuf;
use std::process::{Command, Output};

fn run(lang: &str, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pliego"))
        .env("PLIEGO_LANG", lang)
        .args(args)
        .output()
        .unwrap()
}

fn text(o: &[u8]) -> String {
    String::from_utf8_lossy(o).into_owned()
}

fn sample() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/organize/a.pdf")
}

#[test]
fn help_follows_the_language() {
    let en = text(&run("en", &["pages", "--help"]).stdout);
    assert!(en.contains("Operations on pages: extract, insert"), "{en}");
    assert!(en.contains("Create a PDF with the given pages"), "{en}");
    assert!(!en.contains("Crea un PDF"), "{en}");
    let es = text(&run("es", &["pages", "--help"]).stdout);
    assert!(es.contains("Crea un PDF con las páginas indicadas"), "{es}");
    // Una etiqueta regional (`en-GB`) también es inglés.
    assert!(text(&run("en-GB", &["--help"]).stdout).contains("Pliego: free PDF tools"));
}

#[test]
fn errors_and_messages_follow_the_language() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("m.pdf");
    let args = |extra: &[&str]| -> Vec<String> {
        let mut v = vec![
            "measure".to_string(),
            "add".into(),
            sample().display().to_string(),
            "-o".into(),
            out.display().to_string(),
        ];
        v.extend(extra.iter().map(|s| s.to_string()));
        v
    };
    let a = args(&["--points", "0,0 100,0"]);
    let a: Vec<&str> = a.iter().map(String::as_str).collect();
    let en = run("en", &a);
    assert!(!en.status.success());
    assert!(
        text(&en.stderr).contains("has no scale: calibrate it first"),
        "{}",
        text(&en.stderr)
    );
    assert!(text(&run("es", &a).stderr).contains("no tiene escala"));

    // El resumen de un trabajo también: «3 páginas extraídas».
    let o = tmp.path().join("e.pdf");
    let r = run(
        "en",
        &[
            "pages",
            "extract",
            &sample().display().to_string(),
            "-o",
            &o.display().to_string(),
            "--pages",
            "1",
        ],
    );
    assert!(r.status.success(), "{}", text(&r.stderr));
    assert!(text(&r.stdout).contains("page"), "{}", text(&r.stdout));
    assert!(!text(&r.stdout).contains("extraíd"), "{}", text(&r.stdout));
}

#[test]
fn json_output_is_left_as_written() {
    let r = run("en", &["--json", "info", &sample().display().to_string()]);
    assert!(r.status.success());
    assert!(serde_json::from_str::<serde_json::Value>(&text(&r.stdout)).is_ok());
}
