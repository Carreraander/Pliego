//! Comprimir y optimizar desde la app (flujo W24): guarda una copia optimizada junto al original.
//!
//! La lógica vive aquí, sin Tauri, para probarla en Linux; `app/src-tauri/src/commands/optimize.rs` solo traduce.

use std::path::Path;
use std::str::FromStr;

use pliego_core::{Error, JobContext, Report, Result};
pub use pliego_optimize::SpaceUsage;
use pliego_optimize::{OptimizeOptions, Preset};

use pliego_appdata::copyjob::unique_sibling;

/// Lo que pide el panel de Comprimir u Optimizar avanzado.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CompressRequest {
    /// `pantalla`, `ebook`, `impresion` o `sin-perdida`.
    pub preset: String,
    pub dpi: Option<f32>,
    pub jpeg_quality: Option<u8>,
    pub password: Option<String>,
    /// Etiqueta del nombre de la copia, ya traducida («comprimido»).
    pub suffix: String,
}

/// Optimiza `input` y escribe `<nombre> (<etiqueta>).pdf` junto a él. Devuelve el informe de `pliego-optimize`.
pub fn compress_copy(input: &Path, req: &CompressRequest, ctx: &JobContext) -> Result<Report> {
    let preset = Preset::from_str(&req.preset)?;
    if let Some(d) = req.dpi
        && !(36.0..=1200.0).contains(&d)
    {
        return Err(Error::InvalidInput(format!("resolución fuera de rango: {d}")));
    }
    if let Some(q) = req.jpeg_quality
        && !(1..=100).contains(&q)
    {
        return Err(Error::InvalidInput(format!("calidad JPEG fuera de rango: {q}")));
    }
    let out = unique_sibling(input, &req.suffix, "pdf");
    let opts = OptimizeOptions {
        preset,
        target_dpi: req.dpi,
        jpeg_quality: req.jpeg_quality,
        password: req.password.clone(),
        ..OptimizeOptions::default()
    };
    pliego_optimize::optimize(input, &out, &opts, ctx)
}

/// Reparto del tamaño del archivo (para la estimación y el panel avanzado).
pub fn analyze(input: &Path, password: Option<&str>) -> Result<SpaceUsage> {
    pliego_optimize::analyze_with_password(input, password)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn corpus() -> Option<std::path::PathBuf> {
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/basic");
        std::fs::read_dir(&p)
            .ok()?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .find(|p| p.extension().is_some_and(|e| e == "pdf"))
    }

    #[test]
    fn saves_a_copy_and_keeps_the_original() {
        let Some(src) = corpus() else { return };
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("doc.pdf");
        std::fs::copy(&src, &input).unwrap();
        let before = std::fs::read(&input).unwrap();
        let req = CompressRequest {
            preset: "ebook".into(),
            suffix: "comprimido".into(),
            ..Default::default()
        };
        let report = compress_copy(&input, &req, &JobContext::silent()).unwrap();
        assert_eq!(report.outputs, vec![dir.path().join("doc (comprimido).pdf")]);
        assert!(report.outputs[0].exists());
        assert_eq!(std::fs::read(&input).unwrap(), before, "el original no cambia");
        // Una segunda vez no pisa la primera copia.
        let again = compress_copy(&input, &req, &JobContext::silent()).unwrap();
        assert_eq!(again.outputs, vec![dir.path().join("doc (comprimido 2).pdf")]);
    }

    #[test]
    fn rejects_bad_input_before_touching_disk() {
        let req = |preset: &str, dpi, q| CompressRequest {
            preset: preset.into(),
            dpi,
            jpeg_quality: q,
            suffix: "x".into(),
            ..Default::default()
        };
        let ctx = JobContext::silent();
        let p = Path::new("/no/existe.pdf");
        assert!(matches!(
            compress_copy(p, &req("raro", None, None), &ctx),
            Err(Error::InvalidInput(_))
        ));
        assert!(matches!(
            compress_copy(p, &req("ebook", Some(5.0), None), &ctx),
            Err(Error::InvalidInput(_))
        ));
        assert!(matches!(
            compress_copy(p, &req("ebook", None, Some(0)), &ctx),
            Err(Error::InvalidInput(_))
        ));
        // Un archivo que no existe falla con error de E/S, no con pánico.
        assert!(compress_copy(p, &req("ebook", None, None), &ctx).is_err());
    }

    #[test]
    fn cancelled_job_leaves_no_copy() {
        let Some(src) = corpus() else { return };
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("doc.pdf");
        std::fs::copy(&src, &input).unwrap();
        let ctx = JobContext::silent();
        ctx.cancel.cancel();
        let req = CompressRequest {
            preset: "ebook".into(),
            suffix: "comprimido".into(),
            ..Default::default()
        };
        assert!(matches!(compress_copy(&input, &req, &ctx), Err(Error::Cancelled)));
        assert!(!dir.path().join("doc (comprimido).pdf").exists());
    }
}
