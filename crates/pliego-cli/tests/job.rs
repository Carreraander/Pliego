//! `pliego __job`: el proceso hijo de las herramientas de la app (hueco 1.0 n.º 7), con la CLI real.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use pliego_app_service::isolated::{self, JobSpec};
use pliego_core::{Error, JobContext};

fn cli() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_pliego"))
}

fn corpus(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/organize")
        .join(name)
}

/// Copia los PDF de prueba a una carpeta temporal (el trabajo escribe junto al primero).
fn staged(dir: &Path, names: &[&str]) -> Vec<PathBuf> {
    names
        .iter()
        .map(|n| {
            let to = dir.join(n);
            std::fs::copy(corpus(n), &to).unwrap();
            to
        })
        .collect()
}

#[test]
fn merges_in_the_child_and_reports_the_copy() {
    let dir = tempfile::tempdir().unwrap();
    let inputs = staged(dir.path(), &["a.pdf", "b.pdf"]);
    let spec = JobSpec::Merge {
        inputs,
        options: Default::default(),
        suffix: "combinado".into(),
    };
    let report = isolated::run_in_child(cli(), &spec, &JobContext::silent()).unwrap();
    let out = dir.path().join("a (combinado).pdf");
    assert_eq!(report.outputs, vec![out.clone()]);
    assert!(std::fs::read(&out).unwrap().starts_with(b"%PDF"));
}

#[test]
fn a_tool_that_needs_pdfium_loads_it_in_the_child() {
    if pliego_pdf::pdfium_init_from_environment().is_err() {
        eprintln!("PDFium no disponible: se omite");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let src = staged(dir.path(), &["a.pdf"]).remove(0);
    let spec = JobSpec::Export {
        path: src,
        req: pliego_app_service::convert::ExportRequest {
            format: "txt".into(),
            pages: "all".into(),
            label: "texto".into(),
            ..Default::default()
        },
    };
    let report = isolated::run_in_child(cli(), &spec, &JobContext::silent()).unwrap();
    let text = std::fs::read_to_string(&report.outputs[0]).unwrap();
    assert!(text.contains("A p1"), "{text}");
}

#[test]
fn a_tool_error_comes_back_as_the_same_error_kind() {
    let dir = tempfile::tempdir().unwrap();
    let spec = JobSpec::Compress {
        path: dir.path().join("no-existe.pdf"),
        req: isolated_compress("ebook"),
    };
    let e = isolated::run_in_child(cli(), &spec, &JobContext::silent()).unwrap_err();
    assert!(matches!(e, Error::Io { .. } | Error::Pdf(_)), "{e}");
    // Una petición inválida llega como InvalidInput, no como «se cerró de forma inesperada».
    let spec = JobSpec::Compress {
        path: dir.path().join("x.pdf"),
        req: isolated_compress("no-existe"),
    };
    let e = isolated::run_in_child(cli(), &spec, &JobContext::silent()).unwrap_err();
    assert!(!e.to_string().contains("inesperada"), "{e}");
}

fn isolated_compress(preset: &str) -> pliego_app_service::optimize::CompressRequest {
    pliego_app_service::optimize::CompressRequest {
        preset: preset.into(),
        suffix: "comprimido".into(),
        ..Default::default()
    }
}

#[test]
fn a_garbled_job_is_answered_not_crashed() {
    let mut child = Command::new(cli())
        .arg("__job")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(b"{\"tool\":\"nada\"}\n").unwrap();
    let out = child.wait_with_output().unwrap();
    drop(stdin);
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("InvalidInput"), "{text}");
}

#[test]
fn the_child_exits_when_the_app_goes_away() {
    // Sin trabajo y con la entrada cerrada de golpe: no se queda esperando.
    let mut child = Command::new(cli())
        .arg("__job")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdin.take());
    let out = child.wait_with_output().unwrap();
    assert!(String::from_utf8_lossy(&out.stdout).contains("InvalidInput"));
}

#[test]
fn the_internal_command_is_hidden_from_help() {
    let o = Command::new(cli()).arg("--help").output().unwrap();
    assert!(!String::from_utf8_lossy(&o.stdout).contains("__job"));
}
