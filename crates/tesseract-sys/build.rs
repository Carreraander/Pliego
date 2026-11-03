//! Localiza Tesseract.
//!
//! - Variable `TESSERACT_LIB_DIR` (y `TESSERACT_LIB_NAME`, `LEPTONICA_LIB_NAME`): se usa tal cual. Es lo que prepara
//!   `tools/ocr/setup-tesseract-windows.ps1` (DLL de vcpkg; las DLL deben estar en el PATH al ejecutar).
//! - Si no, `pkg-config` (Linux/CI: paquete `libtesseract-dev`).

use std::env;

fn main() {
    for n in ["TESSERACT_LIB_DIR", "TESSERACT_LIB_NAME", "LEPTONICA_LIB_NAME"] {
        println!("cargo:rerun-if-env-changed={n}");
    }
    if let Ok(dir) = env::var("TESSERACT_LIB_DIR") {
        let name = env::var("TESSERACT_LIB_NAME").unwrap_or_else(|_| "tesseract".into());
        println!("cargo:rustc-link-search=native={dir}");
        println!("cargo:rustc-link-lib={name}");
        if let Ok(lept) = env::var("LEPTONICA_LIB_NAME") {
            println!("cargo:rustc-link-lib={lept}");
        }
        return;
    }
    if let Err(e) = pkg_config::Config::new().atleast_version("5.0").probe("tesseract") {
        panic!(
            "No se encontró Tesseract 5 ({e}). En Linux instala libtesseract-dev; en Windows define \
             TESSERACT_LIB_DIR (ver docs/ocr.md)."
        );
    }
}
