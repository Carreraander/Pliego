use std::path::PathBuf;
use std::process::Command;

use pliego_ocr::available_languages;

fn pliego() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_pliego"));
    c.env("PLIEGO_LANG", "es");
    c
}

fn hola() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/basic/hola.pdf")
}

#[test]
fn list_languages_json() {
    let out = pliego().args(["--json", "ocr", "--list-languages"]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(v.is_array());
}

#[test]
fn skip_and_force_conflict() {
    let out = pliego()
        .arg("ocr")
        .arg(hola())
        .args(["--skip-text", "--force"])
        .output()
        .unwrap();
    assert!(!out.status.success());
}

#[test]
fn skips_pages_with_text_and_writes_output() {
    if !available_languages().iter().any(|l| l == "eng") {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("o.pdf");
    let out = pliego()
        .arg("--json")
        .arg("ocr")
        .arg(hola())
        .args(["--lang", "eng", "-o"])
        .arg(&dest)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(v["summary"].as_str().unwrap().contains("0 página"), "{v}");
    assert!(dest.exists());
}

#[test]
fn refuses_to_overwrite_input() {
    let out = pliego().arg("ocr").arg(hola()).arg("-o").arg(hola()).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("no puede ser el de entrada"));
}

#[test]
fn missing_model_is_explained() {
    let dir = tempfile::tempdir().unwrap();
    let out = pliego()
        .arg("ocr")
        .arg(hola())
        .args(["--force", "--lang", "zzz", "--tessdata"])
        .arg(dir.path())
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("Tesseract no pudo cargar"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn download_of_unverified_language_is_refused() {
    let out = pliego().args(["ocr", "--download", "zzz"]).output().unwrap();
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("catálogo"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn list_downloadable_json() {
    let out = pliego()
        .args(["--json", "ocr", "--list-downloadable"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let v: Vec<String> = serde_json::from_slice(&out.stdout).unwrap();
    assert!(v.iter().any(|l| l == "spa"), "{v:?}");
}

/// Cifra `hola.pdf` con la CLI y devuelve la ruta (contraseña de apertura `abre`).
fn encrypted(dir: &std::path::Path) -> PathBuf {
    let enc = dir.join("cifrado.pdf");
    let out = pliego()
        .args(["protect", "encrypt"])
        .arg(hola())
        .arg("-o")
        .arg(&enc)
        .args(["--password", "abre", "--owner-password", "dueno"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    enc
}

#[test]
fn ocr_of_a_protected_pdf_needs_the_password_and_keeps_it_protected() {
    if !available_languages().iter().any(|l| l == "eng") {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let enc = encrypted(dir.path());
    let run = |extra: &[&str], env: Option<&str>| {
        let mut c = pliego();
        c.env_remove("PLIEGO_PASSWORD").env_remove("RUST_BACKTRACE");
        if let Some(pw) = env {
            c.env("PLIEGO_PASSWORD", pw);
        }
        let out = dir.path().join(format!("o{}.pdf", extra.len() + env.map_or(0, |_| 10)));
        let o = c
            .args(["--json", "ocr"])
            .arg(&enc)
            .args(["--lang", "eng", "--force", "-o"])
            .arg(&out)
            .args(extra)
            .output()
            .unwrap();
        (o, out)
    };

    // Sin contraseña o con una incorrecta: error claro y sin archivo de salida.
    for (extra, env) in [
        (vec![], None),
        (vec!["--password", "mala"], None),
        (vec![], Some("mala")),
    ] {
        let (o, out) = run(&extra, env);
        assert!(!o.status.success());
        assert!(
            String::from_utf8_lossy(&o.stderr).contains("contraseña"),
            "{extra:?} {env:?}"
        );
        assert!(!out.exists(), "sin salida a medias");
    }
    // Con la contraseña en la opción o en el entorno (así la manda la app): funciona.
    for (extra, env) in [(vec!["--password", "abre"], None), (vec![], Some("abre"))] {
        let (o, out) = run(&extra, env);
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        // La copia sigue protegida: sin contraseña, el propio OCR lo rechaza.
        let again = pliego()
            .arg("ocr")
            .arg(&out)
            .args(["--lang", "eng", "--force"])
            .env_remove("PLIEGO_PASSWORD")
            .output()
            .unwrap();
        assert!(!again.status.success(), "la copia debe seguir cifrada");
    }
}
