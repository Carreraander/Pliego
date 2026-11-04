//! Comentar (W26): resumen de comentarios e intercambio XFDF. La lógica vive en `pliego_annot::files` (con pruebas);
//! aquí solo se traducen argumentos. Exportar e importar son trabajos que **guardan un archivo nuevo** junto al
//! original (`<nombre> (comentarios).xfdf`, `<nombre> (con comentarios).pdf`); el PDF abierto no se toca.

use std::path::PathBuf;

use pliego_annot::CommentSummary;
use pliego_app_service::api::ApiResult;
use pliego_appdata::copyjob::unique_sibling;
use pliego_appdata::jobs::JobId;
use pliego_core::{Error, Report};
use tauri::State;
use tauri_plugin_dialog::DialogExt;

use crate::commands::jobs::JobsState;

/// Comentarios del PDF de `path`, agrupados en hilos.
#[tauri::command]
pub async fn annot_summary(path: String) -> ApiResult<CommentSummary> {
    let joined =
        tauri::async_runtime::spawn_blocking(move || pliego_annot::files::summary_of(&PathBuf::from(path), None))
            .await
            .map_err(|e| Error::Other(e.to_string()))?;
    Ok(joined?)
}

/// Exporta los comentarios a `<nombre> (comentarios).xfdf`.
#[tauri::command]
pub fn export_comments_xfdf(jobs: State<'_, JobsState>, path: String) -> JobId {
    let src = PathBuf::from(path);
    jobs.0.spawn(move |_ctx| {
        let out = unique_sibling(&src, "comentarios", "xfdf");
        let n = pliego_annot::files::export_xfdf_file(&src, None, &out)?;
        Ok(Report::with_output(out, format!("{n} comentario(s) exportados")))
    })
}

/// Importa un XFDF en una copia `<nombre> (con comentarios).pdf`.
#[tauri::command]
pub fn import_comments_xfdf(jobs: State<'_, JobsState>, path: String, xfdf: String) -> JobId {
    let (src, xfdf) = (PathBuf::from(path), PathBuf::from(xfdf));
    jobs.0.spawn(move |_ctx| {
        let out = unique_sibling(&src, "con comentarios", "pdf");
        let n = pliego_annot::files::import_xfdf_file(&src, None, &xfdf, &out)?;
        Ok(Report::with_output(out, format!("{n} comentario(s) importados")))
    })
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
