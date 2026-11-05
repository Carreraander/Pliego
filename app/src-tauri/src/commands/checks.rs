//! Comprobaciones de solo lectura (W29): la lógica está en `pliego_app_service::checks`; aquí solo se traducen argumentos.

use std::path::PathBuf;

use pliego_app_service::api::{ApiError, ApiResult};
use pliego_app_service::checks::{self, A11yReport, Level, PreflightReport, Validation};
use pliego_appdata::jobs::JobId;
use pliego_core::Error;
use tauri::State;

use crate::commands::jobs::JobsState;

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
pub async fn validate_pdfa(path: String, level: Level, password: Option<String>) -> ApiResult<Validation> {
    blocking(move || checks::pdfa(&PathBuf::from(path), level, password.as_deref())).await
}

/// Comprobación previa con el perfil `basic`, `print` o `pdfx`.
#[tauri::command]
pub async fn preflight_pdf(path: String, profile: String, password: Option<String>) -> ApiResult<PreflightReport> {
    blocking(move || checks::preflight(&PathBuf::from(path), &profile, password.as_deref())).await
}

/// Orden de lectura de `path` (`null` si el documento no está etiquetado).
#[tauri::command]
pub async fn reading_order(path: String, password: Option<String>) -> ApiResult<Option<Vec<checks::ReadingItem>>> {
    blocking(move || checks::reading_order(&PathBuf::from(path), password.as_deref())).await
}

/// Copia con el orden de tabulación ajustado a la estructura.
#[tauri::command]
pub fn tab_order_copy(jobs: State<'_, JobsState>, path: String, password: Option<String>, suffix: String) -> JobId {
    let path = PathBuf::from(path);
    jobs.0
        .spawn(move |_ctx| checks::tab_order_copy(&path, password.as_deref(), &suffix))
}
