//! Localiza Tesseract.
//!
//! - Variable `TESSERACT_LIB_DIR` (y `TESSERACT_LIB_NAME`, `LEPTONICA_LIB_NAME`, `TESSERACT_EXTRA_LIBS` separadas por
//!   comas): se usa tal cual. Es lo que prepara `tools/ocr/setup-tesseract-windows.ps1`. Con `PROFILE=debug` se
//!   prefieren las variantes `*_DEBUG` (otro runtime de MSVC) si existen.
//! - Si no, `pkg-config` (Linux/CI: paquete `libtesseract-dev`).

use std::env;

fn var(name: &str, debug: bool) -> Option<String> {
    if debug && let Ok(v) = env::var(format!("{name}_DEBUG")) {
        return Some(v);
    }
    env::var(name).ok()
}

fn main() {
    for n in [
        "TESSERACT_LIB_DIR",
        "TESSERACT_LIB_NAME",
        "LEPTONICA_LIB_NAME",
        "TESSERACT_EXTRA_LIBS",
    ] {
        println!("cargo:rerun-if-env-changed={n}");
        println!("cargo:rerun-if-env-changed={n}_DEBUG");
    }
    let debug = env::var("PROFILE").is_ok_and(|p| p == "debug");
    if let Some(dir) = var("TESSERACT_LIB_DIR", debug) {
        let name = var("TESSERACT_LIB_NAME", debug).unwrap_or_else(|| "tesseract".into());
        println!("cargo:rustc-link-search=native={dir}");
        println!("cargo:rustc-link-lib={name}");
        if let Some(lept) = var("LEPTONICA_LIB_NAME", debug) {
            println!("cargo:rustc-link-lib={lept}");
        }
        // Dependencias transitivas de una instalación estática.
        for extra in var("TESSERACT_EXTRA_LIBS", debug)
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
