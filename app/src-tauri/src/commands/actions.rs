//! Acciones en lote (W29): la lógica está en `pliego_app_service::actions`; aquí solo se traducen argumentos.

use std::path::PathBuf;

use pliego_app_service::actions::{self, Action, BatchRequest};
use pliego_app_service::api::{ApiError, ApiResult};
use pliego_appdata::jobs::JobId;
use tauri::State;

use crate::commands::jobs::JobsState;

/// Lee y valida una acción guardada para mostrar su nombre, descripción y pasos antes de ejecutarla.
#[tauri::command]
pub fn actions_load(path: String) -> ApiResult<Action> {
    actions::load_action(&PathBuf::from(path)).map_err(ApiError::from)
}

/// Ejecuta la acción sobre `inputs` (PDF o carpetas). Sin `output`, crea `<primera entrada> (<suffix>)` al lado.
#[tauri::command]
pub fn actions_run(
    jobs: State<'_, JobsState>,
    action: String,
    inputs: Vec<String>,
    output: Option<String>,
    resume: bool,
    suffix: String,
) -> JobId {
    let req = BatchRequest {
        action: PathBuf::from(action),
        inputs: inputs.into_iter().map(PathBuf::from).collect(),
        output: output.map(PathBuf::from),
        resume,
        suffix,
    };
    jobs.0.spawn(move |ctx| actions::run_batch(&req, ctx))
}
