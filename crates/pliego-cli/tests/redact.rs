use std::path::PathBuf;
use std::process::Command;

fn corpus() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/redact/datos.pdf")
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
fn preview_lists_matches_without_writing() {
    let out = ok(pliego()
        .arg("redact")
        .arg(corpus())
        .args(["--pattern", "dni,iban,email"]));
    assert!(
        out.contains("12345678Z")
            && out.contains("ES91 2100 0418 4502 0005 1332")
            && out.contains("ana.lopez@ejemplo.es")
    );
    assert!(!out.contains("12345678A"), "el DNI inválido no debe aparecer");
}

#[test]
fn apply_then_text_and_search_find_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let red = dir.path().join("r.pdf");
    ok(pliego()
        .arg("redact")
        .arg(corpus())
        .args(["--pattern", "dni,nie,cif,iban,email,telefono,fecha", "--apply", "-o"])
        .arg(&red));
    let text = ok(pliego().arg("text").arg(&red));
    for s in [
        "12345678Z",
        "X1234567L",
        "A58818501",
        "ana.lopez",
        "612 345 678",
        "12/03/1985",
    ] {
        assert!(!text.contains(s), "`pliego text` aún muestra {s}");
        let found = ok(pliego().arg("--json").arg("search").arg(&red).arg(s));
        assert!(
            !found.contains("\"page\""),
            "`pliego search` aún encuentra {s}: {found}"
        );
    }
    assert!(text.contains("texto normal conservado"));
}

#[test]
fn apply_requires_output_and_something_to_redact() {
    let out = pliego()
        .arg("redact")
        .arg(corpus())
        .args(["--pattern", "dni", "--apply"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    let out = pliego().arg("redact").arg(corpus()).output().unwrap();
    assert!(!out.status.success());
}
