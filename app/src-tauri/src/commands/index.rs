//! Índice de búsqueda de carpetas (W29): la lógica está en `pliego_app_service::search_index`; aquí solo se traducen argumentos.

use std::path::PathBuf;

use pliego_app_service::api::{ApiError, ApiResult};
use pliego_app_service::isolated::JobSpec;
use pliego_app_service::search_index::{self, Hit};
use pliego_appdata::jobs::JobId;
use pliego_core::Error;
use tauri::State;

use crate::commands::jobs::JobsState;

/// Indexa `folder` como trabajo (progreso y resultado en el centro de tareas).
#[tauri::command]
pub fn index_folder(jobs: State<'_, JobsState>, folder: String) -> JobId {
    let folder = PathBuf::from(folder);
    jobs.spawn_tool(JobSpec::IndexFolder {
        data_dir: pliego_appdata::data_dir(),
        folder,
    })
}

/// Busca en el índice.
#[tauri::command]
pub async fn index_search(query: String, limit: Option<usize>) -> ApiResult<Vec<Hit>> {
    tauri::async_runtime::spawn_blocking(move || {
        search_index::search(&pliego_appdata::data_dir(), &query, limit.unwrap_or(50))
    })
    .await
    .map_err(|e| ApiError::from(Error::InvalidInput(e.to_string())))?
    .map_err(ApiError::from)
}
