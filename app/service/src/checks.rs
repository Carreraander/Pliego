//! Comprobaciones de solo lectura desde la app (flujo W29): accesibilidad (PDF/UA), PDF/A y comprobación previa.
//!
//! La lógica vive aquí, sin Tauri, para probarla en Linux; `app/src-tauri/src/commands/checks.rs` solo traduce.
//! No modifican nada ni crean copias: devuelven el informe del crate para que el panel lo muestre.

use std::path::Path;

pub use pliego_a11y::A11yReport;
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
}
