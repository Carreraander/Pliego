//! Comparar (W24): la lógica está en `pliego_app_service::compare`; aquí solo se traducen argumentos.

use std::path::PathBuf;

use pliego_app_service::api::{ApiError, ApiResult};
use pliego_app_service::compare::{self, CompareRequest};
use pliego_app_service::isolated::JobSpec;
use pliego_appdata::jobs::JobId;
use tauri::State;

use crate::commands::jobs::JobsState;

/// Compara `path` con `other` y guarda el informe PDF junto a `path`; ninguno de los dos se modifica.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn compare_pdfs(
    jobs: State<'_, JobsState>,
    path: String,
    other: String,
    password_a: Option<String>,
    password_b: Option<String>,
    text: bool,
    visual: bool,
    label: String,
) -> JobId {
    let req = CompareRequest {
        other: PathBuf::from(other),
        password_a,
        password_b,
        text,
        visual,
        label,
    };
    jobs.spawn_tool(JobSpec::Compare {
        path: PathBuf::from(path),
        req,
    })
}

/// Comparación para la vista lado a lado: devuelve las páginas emparejadas, los cambios y sus posiciones, sin guardar nada.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn compare_view(
    path: String,
    other: String,
    password_a: Option<String>,
    password_b: Option<String>,
    text: bool,
    visual: bool,
) -> ApiResult<compare::Comparison> {
    let req = CompareRequest {
        other: PathBuf::from(other),
        password_a,
        password_b,
        text,
        visual,
        label: String::new(),
    };
    tauri::async_runtime::spawn_blocking(move || {
        compare::compare_view(&PathBuf::from(path), &req, &pliego_core::JobContext::silent())
    })
    .await
    .map_err(|e| ApiError::from(pliego_core::Error::InvalidInput(e.to_string())))?
    .map_err(ApiError::from)
}
