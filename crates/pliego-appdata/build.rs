//! Lee del `Cargo.lock` del workspace las versiones de los componentes que enseña «Acerca de» (PDFium, Tauri…), para que
//! la pantalla diga siempre la versión realmente enlazada.

use std::{env, fs, path::PathBuf};

const COMPONENTS: &[(&str, &str)] = &[
    ("pdfium-render", "PDFIUM"),
    ("tauri", "TAURI"),
    ("lopdf", "LOPDF"),
    ("boa_engine", "BOA"),
];

fn main() {
    let lock = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../Cargo.lock");
    println!("cargo:rerun-if-changed={}", lock.display());
    let text = fs::read_to_string(&lock).unwrap_or_default();
    for (package, var) in COMPONENTS {
        let version = find_version(&text, package).unwrap_or_else(|| "desconocida".into());
        println!("cargo:rustc-env=PLIEGO_DEP_{var}={version}");
    }
}

/// Versión de la primera entrada `[[package]]` con ese nombre.
fn find_version(lock: &str, package: &str) -> Option<String> {
    let header = format!("name = \"{package}\"");
    let mut lines = lock.lines();
    while let Some(line) = lines.next() {
        if line == header {
            let v = lines.next()?.strip_prefix("version = \"")?;
            return Some(v.trim_end_matches('"').to_owned());
        }
    }
    None
}
