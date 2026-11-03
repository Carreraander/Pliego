use std::path::PathBuf;
use std::process::Command;

fn corpus(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus")
        .join(rel)
}

fn pliego() -> Command {
    Command::new(env!("CARGO_BIN_EXE_pliego"))
}

#[test]
fn check_good_passes_and_bad_fails() {
    let ok = pliego()
        .args(["a11y", "check"])
        .arg(corpus("a11y/bueno.pdf"))
        .output()
        .unwrap();
    assert!(ok.status.success(), "{}", String::from_utf8_lossy(&ok.stderr));
    assert!(String::from_utf8_lossy(&ok.stdout).contains("0 error(es)"));

    let bad = pliego()
        .args(["a11y", "check"])
        .arg(corpus("a11y/malo.pdf"))
        .output()
        .unwrap();
    assert!(!bad.status.success());
    let text = String::from_utf8_lossy(&bad.stdout);
    assert!(text.contains("no declara su idioma") && text.contains("Cómo arreglarlo"));
}

#[test]
fn check_json_and_fix() {
    let out = pliego()
        .args(["--json", "a11y", "check"])
        .arg(corpus("a11y/malo_etiquetado.pdf"))
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let fig = v["issues"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["code"] == "figure_without_alt")
        .unwrap();
    let xref = fig["xref"].as_i64().unwrap();

    let dir = tempfile::tempdir().unwrap();
    let fixed = dir.path().join("f.pdf");
    let res = pliego()
        .args(["a11y", "fix"])
        .arg(corpus("a11y/malo_etiquetado.pdf"))
        .arg("-o")
        .arg(&fixed)
        .args(["--lang", "es-ES", "--title", "Informe", "--tab-order", "--alt"])
        .arg(format!("{xref}=Un cuadrado"))
        .output()
        .unwrap();
    assert!(res.status.success(), "{}", String::from_utf8_lossy(&res.stderr));

    let out = pliego().args(["--json", "a11y", "check"]).arg(&fixed).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let codes: Vec<&str> = v["issues"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["code"].as_str().unwrap())
        .collect();
    assert!(
        !codes.contains(&"figure_without_alt") && !codes.contains(&"no_language"),
        "{codes:?}"
    );
}

#[test]
fn fix_rejects_bad_alt_format() {
    let dir = tempfile::tempdir().unwrap();
    let res = pliego()
        .args(["a11y", "fix"])
        .arg(corpus("a11y/bueno.pdf"))
        .arg("-o")
        .arg(dir.path().join("x.pdf"))
        .args(["--alt", "sin-igual"])
        .output()
        .unwrap();
    assert!(!res.status.success());
}

#[test]
fn autotag_makes_a_plain_document_tagged() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("t.pdf");
    let res = pliego()
        .args(["a11y", "autotag"])
        .arg(corpus("basic/hola.pdf"))
        .arg("-o")
        .arg(&out)
        .args(["--lang", "es", "--title", "Hola"])
        .output()
        .unwrap();
    assert!(res.status.success(), "{}", String::from_utf8_lossy(&res.stderr));
    assert!(String::from_utf8_lossy(&res.stdout).contains("Etiquetado:"));
    let chk = pliego().args(["--json", "a11y", "check"]).arg(&out).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&chk.stdout).unwrap();
    assert_eq!(v["tagged"], true);
    assert_eq!(v["language"], "es");
    // ya etiquetado: no se vuelve a etiquetar
    let again = pliego()
        .args(["a11y", "autotag"])
        .arg(&out)
        .arg("-o")
        .arg(dir.path().join("t2.pdf"))
        .output()
        .unwrap();
    assert!(!again.status.success());
}
