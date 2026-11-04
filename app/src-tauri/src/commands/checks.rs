//! Comprobaciones de solo lectura (W29): la lógica está en `pliego_app_service::checks`; aquí solo se traducen argumentos.

use std::path::PathBuf;

use pliego_app_service::api::{ApiError, ApiResult};
use pliego_app_service::checks::{self, A11yReport, Level, PreflightReport, Validation};
use pliego_core::Error;

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> pliego_core::Result<T> + Send + 'static) -> ApiResult<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| ApiError::from(Error::InvalidInput(e.to_string())))?
        .map_err(ApiError::from)
}

/// Informe de accesibilidad de `path`.
#[tauri::command]
pub async fn check_accessibility(path: String, password: Option<String>) -> ApiResult<A11yReport> {
    blocking(move || checks::accessibility(&PathBuf::from(path), password.as_deref())).await
}

/// Validación PDF/A (`level`: `1b`, `2b`, `2u` o `3b`).
#[tauri::command]
pub async fn validate_pdfa(path: String, level: Level) -> ApiResult<Validation> {
    blocking(move || checks::pdfa(&PathBuf::from(path), level)).await
}

/// Comprobación previa con el perfil `basic`, `print` o `pdfx`.
#[tauri::command]
pub async fn preflight_pdf(path: String, profile: String, password: Option<String>) -> ApiResult<PreflightReport> {
    blocking(move || checks::preflight(&PathBuf::from(path), &profile, password.as_deref())).await
}
