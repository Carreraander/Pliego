//! Validadores externos opcionales. Si la herramienta no está instalada el resultado es `Missing`, no un fallo.

use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Validation {
    Ok,
    /// La herramienta terminó con avisos (qpdf, código 3): el archivo se puede leer pero tiene defectos.
    Warn(String),
    Fail(String),
    Missing,
}

fn run(tool: &str, args: &[&str], file: &Path, warn_code: Option<i32>) -> Validation {
    let out = match Command::new(tool).args(args).arg(file).output() {
        Ok(o) => o,
        Err(_) => return Validation::Missing,
    };
    let text = String::from_utf8_lossy(if out.stdout.is_empty() {
        &out.stderr
    } else {
        &out.stdout
    });
    let first = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("").to_owned();
    match out.status.code() {
        Some(0) => Validation::Ok,
        Some(c) if Some(c) == warn_code => Validation::Warn(first),
        _ => Validation::Fail(first),
    }
}

/// `qpdf --check`: estructura del archivo (xref, flujos, cifrado).
pub fn qpdf_check(file: &Path) -> Validation {
    run("qpdf", &["--check"], file, Some(3))
}

/// veraPDF con el perfil que declare el propio archivo (`--flavour 0`).
pub fn verapdf(file: &Path) -> Validation {
    run("verapdf", &["--flavour", "0", "--format", "text"], file, None)
}
