//! Comparar (W24): la lógica está en `pliego_app_service::compare`; aquí solo se traducen argumentos.

use std::path::PathBuf;

use pliego_app_service::compare::{self, CompareRequest};
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
    jobs.0
        .spawn(move |ctx| compare::compare_copy(&PathBuf::from(path), &req, ctx))
}
