//! Localiza Tesseract.
//!
//! - Variable `TESSERACT_LIB_DIR` (y opcionalmente `TESSERACT_LIB_NAME`, por defecto `tesseract`): se usa tal cual.
//!   Es lo que emplea `tools/ocr/build-tesseract-windows.ps1` en Windows.
//! - Si no, `pkg-config` (Linux/CI: paquete `libtesseract-dev`).

use std::env;

fn main() {
    println!("cargo:rerun-if-env-changed=TESSERACT_LIB_DIR");
    println!("cargo:rerun-if-env-changed=TESSERACT_LIB_NAME");
    println!("cargo:rerun-if-env-changed=TESSERACT_EXTRA_LIBS");
    if let Ok(dir) = env::var("TESSERACT_LIB_DIR") {
        let name = env::var("TESSERACT_LIB_NAME").unwrap_or_else(|_| "tesseract".into());
        println!("cargo:rustc-link-search=native={dir}");
        println!("cargo:rustc-link-lib={name}");
        if let Ok(lept) = env::var("LEPTONICA_LIB_NAME") {
            println!("cargo:rustc-link-lib={lept}");
        }
        // Dependencias transitivas de una instalación estática (separadas por comas).
        for extra in env::var("TESSERACT_EXTRA_LIBS")
            .unwrap_or_default()
            .split(',')
            .filter(|s| !s.is_empty())
        {
            println!("cargo:rustc-link-lib={extra}");
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
