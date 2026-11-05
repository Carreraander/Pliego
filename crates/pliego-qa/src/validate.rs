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
    /// No aplica a este archivo (p. ej. veraPDF sobre un PDF que no declara PDF/A); no cuenta como fallo.
    Skipped(String),
}

fn run(tool: &str, args: &[&str], file: &Path, warn_code: Option<i32>) -> Validation {
    let out = match Command::new(tool).args(args).arg(file).output() {
        Ok(o) => o,
        // En Windows `Command` solo añade `.exe`: veraPDF se instala como lanzador `.cmd`/`.bat`.
        Err(_) if cfg!(windows) => {
            let launched = ["cmd", "bat"]
                .iter()
                .find_map(|ext| Command::new(format!("{tool}.{ext}")).args(args).arg(file).output().ok());
            match launched {
                Some(o) => o,
                None => return Validation::Missing,
            }
        }
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

/// `qpdf --check`: estructura del archivo (xref, flujos, cifrado). Con `password`, abre los PDF cifrados.
pub fn qpdf_check(file: &Path, password: Option<&str>) -> Validation {
    match password {
        Some(p) => run("qpdf", &[&format!("--password={p}"), "--check"], file, Some(3)),
        None => run("qpdf", &["--check"], file, Some(3)),
    }
}

/// ¿Declara el archivo que es PDF/A (`pdfaid:part` en sus metadatos XMP)? Los metadatos de un PDF cifrado pueden no
/// verse; veraPDF tampoco los procesa, así que ese caso se trata como «no PDF/A».
pub fn declares_pdfa(file: &Path) -> bool {
    std::fs::read(file).is_ok_and(|d| d.windows(11).any(|w| w == b"pdfaid:part"))
}

/// veraPDF con el perfil que declare el propio archivo (`--flavour 0`). Solo se ejecuta sobre PDF que declaran PDF/A:
/// con cualquier otro, veraPDF valida contra PDF/A-1b y lo falla siempre, que no dice nada de la salida.
pub fn verapdf(file: &Path) -> Validation {
    if !declares_pdfa(file) {
        return Validation::Skipped("no declara PDF/A".into());
    }
    run("verapdf", &["--flavour", "0", "--format", "text"], file, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verapdf_skips_files_that_do_not_declare_pdfa() {
        let dir = tempfile::tempdir().unwrap();
        let plain = dir.path().join("a.pdf");
        std::fs::write(&plain, b"%PDF-1.7\n1 0 obj<<>>endobj\n").unwrap();
        assert!(matches!(verapdf(&plain), Validation::Skipped(_)));
        let pdfa = dir.path().join("b.pdf");
        std::fs::write(&pdfa, b"%PDF-1.7\n<pdfaid:part>2</pdfaid:part>").unwrap();
        assert!(declares_pdfa(&pdfa));
    }
}
