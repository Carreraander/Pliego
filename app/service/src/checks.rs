//! Comprobaciones de solo lectura desde la app (flujo W29): accesibilidad (PDF/UA), PDF/A y comprobación previa.
//!
//! La lógica vive aquí, sin Tauri, para probarla en Linux; `app/src-tauri/src/commands/checks.rs` solo traduce.
//! No modifican nada ni crean copias: devuelven el informe del crate para que el panel lo muestre.

use std::path::Path;

pub use pliego_a11y::{A11yReport, ReadingItem};
use pliego_core::{Error, JobContext, Result};
pub use pliego_pdfa::{Level, Validation};
pub use pliego_print::PreflightReport;
use pliego_print::Profile;

/// Informe de accesibilidad del archivo.
pub fn accessibility(path: &Path, password: Option<&str>) -> Result<A11yReport> {
    pliego_a11y::check_file(path, password)
}

/// Validación PDF/A al `level` indicado.
pub fn pdfa(path: &Path, level: Level) -> Result<Validation> {
    pliego_pdfa::validate(path, level)
}

/// Comprobación previa con el perfil `basic`, `print` o `pdfx`.
pub fn preflight(path: &Path, profile: &str, password: Option<&str>) -> Result<PreflightReport> {
    let profile =
        Profile::by_name(profile).ok_or_else(|| Error::InvalidInput(format!("perfil desconocido: {profile}")))?;
    let doc = pliego_print::open(path, password)?;
    pliego_print::preflight(&doc, &profile, &JobContext::silent())
}

/// Orden de lectura según la estructura del documento; `None` si no está etiquetado.
pub fn reading_order(path: &Path, password: Option<&str>) -> Result<Option<Vec<ReadingItem>>> {
    pliego_a11y::reading_order_file(path, password)
}

/// Guarda una copia `<nombre> (<suffix>).pdf` con el orden de tabulación de todas las páginas
/// ajustado a la estructura. El original no cambia.
pub fn tab_order_copy(path: &Path, password: Option<&str>, suffix: &str) -> Result<pliego_core::Report> {
    let out = pliego_appdata::copyjob::unique_sibling(path, suffix, "pdf");
    let opts = pliego_a11y::FixOptions {
        tab_order: true,
        ..Default::default()
    };
    pliego_a11y::fix_file(path, &out, &opts, password)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_profile_is_rejected_before_opening() {
        let err = preflight(Path::new("no-existe.pdf"), "otro", None).unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)));
    }

    #[test]
    fn missing_file_is_an_error_in_all_checks() {
        let p = Path::new("no-existe.pdf");
        assert!(accessibility(p, None).is_err());
        assert!(pdfa(p, Level::A2b).is_err());
        assert!(preflight(p, "basic", None).is_err());
    }

    #[test]
    fn tab_order_copy_leaves_the_original_alone() {
        let dir = std::env::temp_dir().join(format!("pliego-order-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("malo.pdf");
        std::fs::copy(
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/corpus/a11y/malo.pdf"),
            &src,
        )
        .unwrap();
        let before = std::fs::read(&src).unwrap();
        let report = tab_order_copy(&src, None, "orden").unwrap();
        assert_eq!(std::fs::read(&src).unwrap(), before);
        let out = report.outputs.first().expect("copia creada").clone();
        assert!(out.exists());
        assert!(reading_order(&src, None).unwrap().is_none());
        std::fs::remove_dir_all(dir).ok();
    }
}
