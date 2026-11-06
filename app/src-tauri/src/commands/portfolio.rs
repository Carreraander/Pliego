//! Carteras PDF (W29): la lógica está en `pliego_app_service::portfolio`; aquí solo se traducen argumentos.

use std::path::PathBuf;

use pliego_app_service::api::{ApiError, ApiResult};
use pliego_app_service::isolated::JobSpec;
use pliego_app_service::portfolio::{self, Listing};
use pliego_appdata::jobs::JobId;
use pliego_core::Error;
use tauri::State;

use crate::commands::jobs::JobsState;

/// Crea una cartera con `paths` junto al primero (`<nombre> (<suffix>).pdf`).
#[tauri::command]
pub fn portfolio_create(jobs: State<'_, JobsState>, paths: Vec<String>, suffix: String, lang: Option<String>) -> JobId {
    let lang = lang.as_deref().map(pliego_core::Lang::from_tag).unwrap_or_default();
    let files: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    jobs.spawn_tool(JobSpec::PortfolioCreate { files, suffix, lang })
}

/// Archivos incrustados de `path`.
#[tauri::command]
pub async fn portfolio_list(path: String, password: Option<String>) -> ApiResult<Listing> {
    tauri::async_runtime::spawn_blocking(move || portfolio::list(&PathBuf::from(path), password.as_deref()))
        .await
        .map_err(|e| ApiError::from(Error::InvalidInput(e.to_string())))?
        .map_err(ApiError::from)
}

/// Extrae adjuntos (todos si `names` está vacío) a una carpeta nueva junto a `path`.
#[tauri::command]
pub fn portfolio_extract(
    jobs: State<'_, JobsState>,
    path: String,
    names: Vec<String>,
    password: Option<String>,
    suffix: String,
) -> JobId {
    let path = PathBuf::from(path);
    jobs.spawn_tool(JobSpec::PortfolioExtract {
        path,
        names,
        password,
        suffix,
    })
}
