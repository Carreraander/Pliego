//! Spike W35: ¿se puede cargar `pdfium.dll` de forma dinámica junto al worker, y cuánto cuesta?
//!
//! Uso: `pdfium-spike <pdfium.dll|carpeta> <archivo.pdf> [ppp=150]`
//! Imprime una línea JSON con tiempos (ms) (la memoria se mide desde fuera con `tools/pdfium/measure.ps1`) para pegar en `docs/handoff/flujos/w35.md`.

use std::path::{Path, PathBuf};
use std::time::Instant;

use pdfium_render::prelude::*;

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let lib = PathBuf::from(args.next().ok_or("falta la ruta de pdfium.dll")?);
    let pdf = PathBuf::from(args.next().ok_or("falta el PDF")?);
    let dpi: f32 = args
        .next()
        .map_or(Ok(150.0), |s| s.parse().map_err(|_| "ppp no válido"))?;

    let t0 = Instant::now();
    // Carga explícita por ruta absoluta: nunca por PATH (evita DLL hijacking y una pdfium.dll ajena).
    let lib_path = if lib.is_dir() {
        Pdfium::pdfium_platform_library_name_at_path(&lib)
    } else {
        lib
    };
    let bindings =
        Pdfium::bind_to_library(&lib_path).map_err(|e| format!("no se pudo cargar {}: {e}", lib_path.display()))?;
    let pdfium = Pdfium::new(bindings);
    let load_ms = t0.elapsed().as_secs_f64() * 1e3;

    let t1 = Instant::now();
    let doc = pdfium
        .load_pdf_from_file(&pdf, None)
        .map_err(|e| format!("abrir {}: {e}", pdf.display()))?;
    let open_ms = t1.elapsed().as_secs_f64() * 1e3;
    let pages = doc.pages().len();

    let t2 = Instant::now();
    let page = doc.pages().get(0).map_err(|e| e.to_string())?;
    let cfg = PdfRenderConfig::new().scale_page_by_factor(dpi / 72.0);
    let bmp = page.render_with_config(&cfg).map_err(|e| e.to_string())?;
    let render_ms = t2.elapsed().as_secs_f64() * 1e3;
    let (w, h) = (bmp.width(), bmp.height());

    let t3 = Instant::now();
    let text = page.text().map_err(|e| e.to_string())?.all();
    let text_ms = t3.elapsed().as_secs_f64() * 1e3;

    // Una DLL equivocada o con otra versión de API suele fallar aquí, no al cargar.
    let out = serde_json::json!({
        "dll": Path::new(&lib_path).file_name().map(|s| s.to_string_lossy().into_owned()),
        "dll_bytes": std::fs::metadata(&lib_path).map(|m| m.len()).ok(),
        "pdf": pdf.file_name().map(|s| s.to_string_lossy().into_owned()),
        "pages": pages,
        "load_ms": round(load_ms),
        "open_ms": round(open_ms),
        "render_ms": round(render_ms),
        "text_ms": round(text_ms),
        "px": [w, h],
        "text_chars": text.chars().count(),
    });
    println!("{out}");
    Ok(())
}

fn round(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}
