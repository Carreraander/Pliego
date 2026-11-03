use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn corpus() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/organize/a.pdf")
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pliego")).args(args).output().unwrap()
}

fn ok(o: &Output) {
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}

fn text(path: &Path) -> String {
    let o = run(&["text", "--pages", "all", path.to_str().unwrap()]);
    ok(&o);
    String::from_utf8_lossy(&o.stdout)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn header_bates_watermark_chain() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("s.pdf");
    let (a, s) = (corpus(), out.to_str().unwrap().to_string());
    let a = a.to_str().unwrap();
    ok(&run(&[
        "stamp",
        "header",
        a,
        "-o",
        &s,
        "--footer-center",
        "Pág. {page}/{pages}",
        "--header-left",
        "{file}",
    ]));
    ok(&run(&[
        "stamp",
        "bates",
        &s,
        "--in-place",
        "--prefix",
        "EXP-",
        "--digits",
        "4",
        "--start",
        "7",
    ]));
    ok(&run(&[
        "stamp",
        "watermark",
        &s,
        "--in-place",
        "--text",
        "COPIA",
        "--behind",
        "--pages",
        "2",
    ]));
    let t = text(&out);
    for want in ["a.pdf", "Pág. 1/3", "Pág. 3/3", "EXP-0007", "EXP-0009", "COPIA", "A p1"] {
        assert!(t.contains(want), "falta {want:?} en {t}");
    }
    assert_eq!(t.matches("COPIA").count(), 1);
}

#[test]
fn background_and_errors() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("b.pdf");
    let a = corpus();
    ok(&run(&[
        "stamp",
        "background",
        a.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--color",
        "#ffeecc",
    ]));
    assert!(out.exists());
    // Sin destino no se sobrescribe, y los colores y opciones mal escritos fallan con mensaje.
    assert!(!run(&["stamp", "bates", a.to_str().unwrap()]).status.success());
    assert!(
        !run(&[
            "stamp",
            "background",
            a.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
            "--color",
            "rojo"
        ])
        .status
        .success()
    );
    assert!(
        !run(&["stamp", "watermark", a.to_str().unwrap(), "-o", out.to_str().unwrap()])
            .status
            .success()
    );
}
