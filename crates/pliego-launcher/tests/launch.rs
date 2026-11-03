//! Pruebas del lanzador con una app falsa (solo Unix: el «pliego-app.exe» es un script de shell).
#![cfg(unix)]

use pliego_install::Layout;
use pliego_launcher::{Target, run};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

fn fake_app(root: &Path, version: &str, body: &str) {
    let dir = root.join(format!("app-{version}"));
    fs::create_dir_all(&dir).unwrap();
    for name in ["pliego-app.exe", "pliego.exe"] {
        let p = dir.join(name);
        fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
    }
}

#[test]
fn passes_arguments_and_exit_code() {
    let d = tempfile::tempdir().unwrap();
    fake_app(
        d.path(),
        "0.4.0",
        r#"echo "$@" > "$PLIEGO_INSTALL_ROOT/args.txt"; exit 7"#,
    );
    Layout::new(d.path()).set_current("0.4.0").unwrap();
    let code = run(d.path(), Target::Cli, vec!["pages".into(), "a b.pdf".into()]).unwrap();
    assert_eq!(code, 7);
    assert_eq!(
        fs::read_to_string(d.path().join("args.txt")).unwrap().trim(),
        "pages a b.pdf"
    );
}

#[test]
fn activates_pending_before_running() {
    let d = tempfile::tempdir().unwrap();
    fake_app(d.path(), "0.4.0", "exit 0");
    fake_app(
        d.path(),
        "0.5.0",
        r#"echo $PLIEGO_VERSION > "$PLIEGO_INSTALL_ROOT/ran.txt""#,
    );
    let l = Layout::new(d.path());
    l.set_current("0.4.0").unwrap();
    l.set_pending("0.5.0").unwrap();
    run(d.path(), Target::App, vec![]).unwrap();
    assert_eq!(fs::read_to_string(d.path().join("ran.txt")).unwrap().trim(), "0.5.0");
    assert_eq!(l.current().unwrap().as_deref(), Some("0.5.0"));
    assert_eq!(l.pending().unwrap(), None);
}

/// Una versión nueva que se cierra al instante dos veces se descarta y se vuelve a la anterior.
#[test]
fn failing_new_version_rolls_back() {
    let d = tempfile::tempdir().unwrap();
    fake_app(d.path(), "0.4.0", r#"echo ok > "$PLIEGO_INSTALL_ROOT/ok.txt""#);
    fake_app(d.path(), "0.5.0", "exit 3");
    let l = Layout::new(d.path());
    l.set_current("0.4.0").unwrap();
    l.mark_healthy("0.4.0").unwrap();
    l.set_pending("0.5.0").unwrap();

    assert_eq!(run(d.path(), Target::App, vec![]).unwrap(), 3);
    assert_eq!(run(d.path(), Target::App, vec![]).unwrap(), 3);
    assert_eq!(run(d.path(), Target::App, vec![]).unwrap(), 0);
    assert!(d.path().join("ok.txt").is_file());
    assert_eq!(l.current().unwrap().as_deref(), Some("0.4.0"));
    assert_eq!(l.skipped(), vec!["0.5.0"]);
}

#[test]
fn cli_failures_do_not_count_against_health() {
    let d = tempfile::tempdir().unwrap();
    fake_app(d.path(), "0.4.0", "exit 2");
    fake_app(d.path(), "0.3.0", "exit 0");
    let l = Layout::new(d.path());
    l.set_current("0.3.0").unwrap();
    l.set_pending("0.4.0").unwrap();
    for _ in 0..4 {
        assert_eq!(run(d.path(), Target::Cli, vec![]).unwrap(), 2);
    }
    assert_eq!(l.current().unwrap().as_deref(), Some("0.4.0"));
}

#[test]
fn reports_clear_errors() {
    let d = tempfile::tempdir().unwrap();
    let e = run(d.path(), Target::App, vec![]).unwrap_err();
    assert!(e.contains("no está instalado"), "{e}");
    fake_app(d.path(), "0.4.0", "exit 0");
    fs::remove_file(d.path().join("app-0.4.0/pliego-app.exe")).unwrap();
    Layout::new(d.path()).set_current("0.4.0").unwrap();
    let e = run(d.path(), Target::App, vec![]).unwrap_err();
    assert!(e.contains("Falta pliego-app.exe"), "{e}");
}
