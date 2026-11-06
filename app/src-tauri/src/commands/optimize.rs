//! Comprimir y optimizar (W24): la lógica está en `pliego_app_service::optimize`; aquí solo se traducen argumentos.

use std::path::PathBuf;

use pliego_app_service::api::ApiResult;
use pliego_app_service::isolated::JobSpec;
use pliego_app_service::optimize::{self, CompressRequest, SpaceUsage};
use pliego_appdata::jobs::JobId;
use tauri::State;

use crate::commands::jobs::JobsState;

/// Guarda una copia optimizada de `path` y devuelve el trabajo (progreso y resultado en el centro de tareas).
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn compress_pdf(
    jobs: State<'_, JobsState>,
    path: String,
    preset: String,
    dpi: Option<f32>,
    jpeg_quality: Option<u8>,
    password: Option<String>,
    suffix: String,
) -> JobId {
    let req = CompressRequest {
        preset,
        dpi,
        jpeg_quality,
        password,
        suffix,
    };
    jobs.spawn_tool(JobSpec::Compress {
        path: PathBuf::from(path),
        req,
    })
}

/// Reparto del tamaño del archivo por categorías.
#[tauri::command]
pub fn analyze_pdf(path: String, password: Option<String>) -> ApiResult<SpaceUsage> {
    Ok(optimize::analyze(&PathBuf::from(path), password.as_deref())?)
}
