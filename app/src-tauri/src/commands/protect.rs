//! Proteger y redactar (W25). La lógica y sus pruebas están en `pliego_app_service::protect`; aquí solo se traducen
//! argumentos y se lanzan los trabajos. Todas las operaciones escriben una copia junto al original.

use std::path::PathBuf;

use pliego_app_service::api::{ApiError, ApiResult};
use pliego_app_service::isolated::JobSpec;
use pliego_app_service::protect::{self, FoundMatch, ProtectRequest, RedactRequest, SanitizeRequest};
use pliego_appdata::jobs::JobId;
use tauri::State;

use crate::commands::jobs::JobsState;

/// Cifra una copia con contraseña, permisos y AES-256 (o AES-128).
#[tauri::command]
pub fn protect_encrypt(jobs: State<'_, JobsState>, request: ProtectRequest) -> JobId {
    jobs.spawn_tool(JobSpec::Protect { req: request })
}

/// Quita la protección de una copia, conociendo la contraseña.
#[tauri::command]
pub fn protect_remove(jobs: State<'_, JobsState>, path: PathBuf, password: String, suffix: Option<String>) -> JobId {
    jobs.spawn_tool(JobSpec::Unprotect { path, password, suffix })
}

/// Quita metadatos, JavaScript, adjuntos, comentarios… de una copia y deja un informe.
#[tauri::command]
pub fn protect_sanitize(jobs: State<'_, JobsState>, request: SanitizeRequest) -> JobId {
    jobs.spawn_tool(JobSpec::Sanitize { req: request })
}

/// Vista previa de los datos personales que se redactarían (no modifica nada).
#[tauri::command]
pub async fn redact_find(path: PathBuf, password: Option<String>, patterns: Vec<String>) -> ApiResult<Vec<FoundMatch>> {
    tauri::async_runtime::spawn_blocking(move || protect::find_matches(&path, password.as_deref(), &patterns))
        .await
        .map_err(|e| ApiError::from(pliego_core::Error::Other(e.to_string())))?
        .map_err(ApiError::from)
}

/// Redacta una copia (zonas marcadas y/o patrones) y verifica que el texto ya no se puede extraer.
#[tauri::command]
pub fn redact_apply(jobs: State<'_, JobsState>, request: RedactRequest) -> JobId {
    jobs.spawn_tool(JobSpec::Redact { req: request })
}
