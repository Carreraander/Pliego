use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn corpus(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/organize")
        .join(name)
}

fn pliego(args: &[&str], extra: &[&Path]) -> Output {
    let mut c = Command::new(env!("CARGO_BIN_EXE_pliego"));
    c.env("PLIEGO_LANG", "es");
    c.args(args);
    c.args(extra);
    c.output().unwrap()
}

fn ok(o: &Output) {
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}

fn text(path: &Path) -> String {
    let o = pliego(&["text", "--pages", "all"], &[path]);
    ok(&o);
    String::from_utf8_lossy(&o.stdout)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn merge_then_pages_ops() {
    let dir = tempfile::tempdir().unwrap();
    let m = dir.path().join("m.pdf");
    let o = Command::new(env!("CARGO_BIN_EXE_pliego"))
        .env("PLIEGO_LANG", "es")
        .arg("merge")
        .arg(corpus("a.pdf"))
        .arg(corpus("b.pdf"))
        .arg("-o")
        .arg(&m)
        .output()
        .unwrap();
    ok(&o);
    assert_eq!(text(&m), "A p1 A p2 A p3 B p1 B p2");

    let e = dir.path().join("e.pdf");
    let o = Command::new(env!("CARGO_BIN_EXE_pliego"))
        .env("PLIEGO_LANG", "es")
        .args(["pages", "extract"])
        .arg(&m)
        .args(["-p", "5,1-2", "-o"])
        .arg(&e)
        .output()
        .unwrap();
    ok(&o);
    assert_eq!(text(&e), "B p2 A p1 A p2");

    // Sin -o ni --in-place no se toca el original.
    let o = Command::new(env!("CARGO_BIN_EXE_pliego"))
        .env("PLIEGO_LANG", "es")
        .args(["pages", "delete"])
        .arg(&m)
        .args(["-p", "1"])
        .output()
        .unwrap();
    assert!(!o.status.success());
    assert_eq!(text(&m), "A p1 A p2 A p3 B p1 B p2");

    let o = Command::new(env!("CARGO_BIN_EXE_pliego"))
        .env("PLIEGO_LANG", "es")
        .args(["pages", "reorder"])
        .arg(&e)
        .args(["--order", "3,1,2", "--in-place"])
        .output()
        .unwrap();
    ok(&o);
    assert_eq!(text(&e), "A p2 B p2 A p1");
}

#[test]
fn split_every_and_json() {
    let dir = tempfile::tempdir().unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_pliego"))
        .env("PLIEGO_LANG", "es")
        .args(["--json", "split", "--every", "2"])
        .arg(corpus("a.pdf"))
        .arg("-o")
        .arg(dir.path())
        .output()
        .unwrap();
    ok(&o);
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["outputs"].as_array().unwrap().len(), 2);
    assert_eq!(text(&dir.path().join("a-002.pdf")), "A p3");
}

#[test]
fn labels_rotate_crop_insert() {
    let dir = tempfile::tempdir().unwrap();
    let r = dir.path().join("r.pdf");
    let run = |args: &[&str]| {
        let o = Command::new(env!("CARGO_BIN_EXE_pliego"))
            .env("PLIEGO_LANG", "es")
            .args(args)
            .output()
            .unwrap();
        ok(&o);
    };
    let a = corpus("a.pdf");
    let (a, r_s) = (a.to_str().unwrap(), r.to_str().unwrap());
    run(&["pages", "rotate", a, "-p", "2", "-d", "-90", "-o", r_s]);
    run(&["pages", "crop", r_s, "--margins", "10,10,10,10", "--in-place"]);
    run(&[
        "pages",
        "labels",
        r_s,
        "--rule",
        "1:lower-roman",
        "--rule",
        "2:decimal:A-",
        "--in-place",
    ]);
    let b = corpus("b.pdf");
    run(&[
        "pages",
        "insert",
        r_s,
        "--at",
        "2",
        "--from",
        b.to_str().unwrap(),
        "--source-pages",
        "1",
        "--in-place",
    ]);
    run(&["pages", "insert", r_s, "--blank", "1", "--in-place"]);
    assert_eq!(text(&r), "A p1 B p1 A p2 A p3");
    let o = Command::new(env!("CARGO_BIN_EXE_pliego"))
        .env("PLIEGO_LANG", "es")
        .args(["--json", "info"])
        .arg(&r)
        .output()
        .unwrap();
    ok(&o);
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["page_count"], 5);
}
