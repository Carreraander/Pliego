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

fn ok(cmd: &mut Command) -> String {
    let out = cmd.output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn encrypt_requires_password_to_read_then_decrypt() {
    let dir = tempfile::tempdir().unwrap();
    let enc = dir.path().join("c.pdf");
    ok(pliego()
        .args(["protect", "encrypt"])
        .arg(corpus("basic/hola.pdf"))
        .arg("-o")
        .arg(&enc)
        .args([
            "--password",
            "abre",
            "--owner-password",
            "manda",
            "--deny",
            "copy,modify",
        ]));

    // Sin contraseña no se puede leer.
    let out = pliego().arg("text").arg(&enc).output().unwrap();
    assert!(!out.status.success());
    // Con ella sí.
    let text = ok(pliego().arg("text").arg(&enc).args(["--password", "abre"]));
    assert!(text.contains("Hola Pliego"));

    let shown = ok(pliego()
        .args(["protect", "permissions"])
        .arg(&enc)
        .args(["--current-password", "abre"]));
    assert!(
        shown.contains("copiar texto e imágenes") && shown.contains("denegado"),
        "{shown}"
    );

    let plain = dir.path().join("p.pdf");
    ok(pliego()
        .args(["protect", "decrypt"])
        .arg(&enc)
        .arg("-o")
        .arg(&plain)
        .args(["--password", "abre"]));
    assert!(ok(pliego().arg("text").arg(&plain)).contains("Hola Pliego"));
}

#[test]
fn deny_without_owner_password_fails() {
    let dir = tempfile::tempdir().unwrap();
    let out = pliego()
        .args(["protect", "encrypt"])
        .arg(corpus("basic/hola.pdf"))
        .arg("-o")
        .arg(dir.path().join("c.pdf"))
        .args(["--password", "x", "--deny", "print"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("contraseña de permisos"));
}

#[test]
fn sanitize_reports_what_was_removed() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("limpio.pdf");
    let json = ok(pliego()
        .args(["--json", "protect", "sanitize"])
        .arg(corpus("protect/sucio.pdf"))
        .arg("-o")
        .arg(&out));
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(v["summary"].as_str().unwrap().contains("Metadatos XMP"));
    let bytes = std::fs::read(&out).unwrap();
    assert!(!bytes.windows(14).any(|w| w == b"Autora Privada"));
}
