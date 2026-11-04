//! Sellos y marcas (W27): la lógica está en `pliego_app_service::stamp`; aquí solo se traducen argumentos. Cada
//! herramienta es un trabajo que guarda una copia junto al original y devuelve el `JobId`.

use std::path::PathBuf;

use pliego_app_service::stamp::{StampOp, stamp_copy};
use pliego_app_service::stamps::{StampRequest, stamp_annot_copy};
use pliego_appdata::jobs::JobId;
use tauri::State;

use crate::commands::jobs::JobsState;

/// Aplica `op` a las páginas `pages` (`"all"`, `"1-3,5"`, `"odd"`…) de `path` y guarda una copia.
#[tauri::command]
pub fn stamp_pdf(
    jobs: State<'_, JobsState>,
    path: String,
    pages: String,
    op: StampOp,
    password: Option<String>,
) -> JobId {
    jobs.0
        .spawn(move |ctx| stamp_copy(&PathBuf::from(path), &pages, password.as_deref(), &op, ctx))
}

/// Coloca un sello (predefinido, de texto o de imagen) como anotación en las páginas `pages` y guarda una copia.
#[tauri::command]
pub fn stamp_annot_pdf(
    jobs: State<'_, JobsState>,
    path: String,
    pages: String,
    request: StampRequest,
    password: Option<String>,
) -> JobId {
    jobs.0
        .spawn(move |ctx| stamp_annot_copy(&PathBuf::from(path), &pages, password.as_deref(), &request, ctx))
}
