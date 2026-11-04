//! Comentar (W26): resumen de comentarios e intercambio XFDF. La lógica (con pruebas) está en
//! `pliego_app_service::annot`; aquí solo se traducen argumentos. Exportar e importar son trabajos que guardan un
//! archivo nuevo junto al original; el PDF abierto no se toca.

use std::path::PathBuf;

use pliego_app_service::annot::{self, CommentSummary};
use pliego_app_service::api::ApiResult;
use pliego_appdata::jobs::JobId;
use pliego_core::Error;
use tauri::State;
use tauri_plugin_dialog::DialogExt;

use crate::commands::jobs::JobsState;

/// Comentarios del PDF de `path`, agrupados en hilos.
#[tauri::command]
pub async fn annot_summary(path: String) -> ApiResult<CommentSummary> {
    let joined = tauri::async_runtime::spawn_blocking(move || annot::summary_of(&PathBuf::from(path), None))
        .await
        .map_err(|e| Error::Other(e.to_string()))?;
    Ok(joined?)
}

/// Exporta los comentarios a `<nombre> (<suffix>).xfdf`.
#[tauri::command]
pub fn export_comments_xfdf(jobs: State<'_, JobsState>, path: String, suffix: String) -> JobId {
    jobs.0
        .spawn(move |_ctx| annot::export_xfdf_copy(&PathBuf::from(path), None, &suffix))
}

/// Importa un XFDF en una copia `<nombre> (<suffix>).pdf`.
#[tauri::command]
pub fn import_comments_xfdf(jobs: State<'_, JobsState>, path: String, xfdf: String, suffix: String) -> JobId {
    jobs.0
        .spawn(move |_ctx| annot::import_xfdf_copy(&PathBuf::from(path), None, &PathBuf::from(xfdf), &suffix))
}

/// Selector nativo de un archivo XFDF; `None` si se cancela.
#[tauri::command]
pub async fn pick_xfdf_file(app: tauri::AppHandle, title: Option<String>) -> Option<String> {
    let mut dialog = app.dialog().file().add_filter("XFDF", &["xfdf"]);
    if let Some(t) = title {
        dialog = dialog.set_title(t);
    }
    dialog
        .blocking_pick_file()
        .and_then(|p| p.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned())
}
