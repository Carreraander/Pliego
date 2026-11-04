//! Comprobaciones de solo lectura (W29): accesibilidad (PDF/UA), PDF/A y comprobación previa de imprenta.
//! Abren el archivo del documento en un hilo aparte y devuelven el informe del crate para que el panel lo muestre;
//! no modifican nada ni crean copias.

use std::path::PathBuf;

use pliego_a11y::A11yReport;
use pliego_app_service::api::{ApiError, ApiResult};
use pliego_core::{Error, JobContext};
use pliego_pdfa::{Level, Validation};
use pliego_print::{PreflightReport, Profile};

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> pliego_core::Result<T> + Send + 'static) -> ApiResult<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| ApiError::from(Error::InvalidInput(e.to_string())))?
        .map_err(ApiError::from)
}

/// Informe de accesibilidad de `path`.
#[tauri::command]
pub async fn check_accessibility(path: String, password: Option<String>) -> ApiResult<A11yReport> {
    blocking(move || pliego_a11y::check_file(&PathBuf::from(path), password.as_deref())).await
}

/// Validación PDF/A (`level`: `1b`, `2b`, `2u` o `3b`).
#[tauri::command]
pub async fn validate_pdfa(path: String, level: Level) -> ApiResult<Validation> {
    blocking(move || pliego_pdfa::validate(&PathBuf::from(path), level)).await
}

/// Comprobación previa con el perfil `basic`, `print` o `pdfx`.
#[tauri::command]
pub async fn preflight_pdf(path: String, profile: String, password: Option<String>) -> ApiResult<PreflightReport> {
    blocking(move || {
        let profile =
            Profile::by_name(&profile).ok_or_else(|| Error::InvalidInput(format!("perfil desconocido: {profile}")))?;
        let doc = pliego_print::open(&PathBuf::from(path), password.as_deref())?;
        pliego_print::preflight(&doc, &profile, &JobContext::silent())
    })
    .await
}
