//! Comentar (W26): resumen de comentarios e intercambio XFDF. La lógica (con pruebas) está en
//! `pliego_app_service::annot`; aquí solo se traducen argumentos. Exportar e importar son trabajos que guardan un
//! archivo nuevo junto al original; el PDF abierto no se toca.

use std::path::PathBuf;

use pliego_app_service::DocId;
use pliego_app_service::annot::{self, AnnotId, AnnotSpec, CommentSummary, Editor, FieldInfo, FieldValues};
use pliego_app_service::api::ApiResult;
use pliego_app_service::isolated::JobSpec;
use pliego_appdata::jobs::JobId;
use pliego_core::Error;
use tauri::State;
use tauri_plugin_dialog::DialogExt;

use crate::AppState;
use crate::commands::jobs::JobsState;

/// Estado de la edición de comentarios sobre el propio PDF (pila de deshacer por archivo).
#[derive(Default)]
pub struct AnnotEditState(pub Editor);

/// Comentarios del PDF de `path`, agrupados en hilos.
#[tauri::command]
pub async fn annot_summary(path: String, password: Option<String>) -> ApiResult<CommentSummary> {
    let joined =
        tauri::async_runtime::spawn_blocking(move || annot::summary_of(&PathBuf::from(path), password.as_deref()))
            .await
            .map_err(|e| Error::Other(e.to_string()))?;
    Ok(joined?)
}

/// Exporta los comentarios a `<nombre> (<suffix>).xfdf`.
#[tauri::command]
pub fn export_comments_xfdf(
    jobs: State<'_, JobsState>,
    path: String,
    password: Option<String>,
    suffix: String,
) -> JobId {
    jobs.spawn_tool(JobSpec::ExportXfdf {
        path: PathBuf::from(path),
        password,
        suffix,
    })
}

/// Importa un XFDF en una copia `<nombre> (<suffix>).pdf`.
#[tauri::command]
pub fn import_comments_xfdf(
    jobs: State<'_, JobsState>,
    path: String,
    password: Option<String>,
    xfdf: String,
    suffix: String,
) -> JobId {
    jobs.spawn_tool(JobSpec::ImportXfdf {
        path: PathBuf::from(path),
        password,
        xfdf: PathBuf::from(xfdf),
        suffix,
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

/// Añade un comentario al PDF abierto (copia de seguridad antes del primer cambio) y recarga el visor.
/// Devuelve cuántos cambios se pueden deshacer.
#[tauri::command]
pub async fn annot_add(
    state: State<'_, AppState>,
    editor: State<'_, AnnotEditState>,
    doc_id: DocId,
    path: String,
    password: Option<String>,
    spec: AnnotSpec,
    suffix: String,
) -> ApiResult<usize> {
    let path = PathBuf::from(path);
    let saved = editor.0.add_saved(&path, password.as_deref(), &spec, &suffix)?;
    state.service.reload_saved(doc_id, &path, &saved)?;
    Ok(editor.0.undo_depth(&path))
}

/// Borra un comentario del PDF abierto y recarga el visor.
#[tauri::command]
pub async fn annot_delete(
    state: State<'_, AppState>,
    editor: State<'_, AnnotEditState>,
    doc_id: DocId,
    path: String,
    password: Option<String>,
    id: AnnotId,
    suffix: String,
) -> ApiResult<usize> {
    let path = PathBuf::from(path);
    let saved = editor.0.delete_saved(&path, password.as_deref(), id, &suffix)?;
    state.service.reload_saved(doc_id, &path, &saved)?;
    Ok(editor.0.undo_depth(&path))
}

/// Deshace el último cambio de comentarios; devuelve cuántos quedan por deshacer.
#[tauri::command]
pub async fn annot_undo(
    state: State<'_, AppState>,
    editor: State<'_, AnnotEditState>,
    doc_id: DocId,
    path: String,
) -> ApiResult<usize> {
    let path = PathBuf::from(path);
    if let Some(saved) = editor.0.undo_saved(&path)? {
        state.service.reload_saved(doc_id, &path, &saved)?;
    }
    Ok(editor.0.undo_depth(&path))
}

/// Campos del formulario de `path`, con su recuadro en la página (Rellenar y firmar).
#[tauri::command]
pub async fn forms_fields(path: String, password: Option<String>) -> ApiResult<Vec<FieldInfo>> {
    let joined =
        tauri::async_runtime::spawn_blocking(move || annot::form_fields(&PathBuf::from(path), password.as_deref()))
            .await
            .map_err(|e| Error::Other(e.to_string()))?;
    Ok(joined?)
}

/// Resultado de rellenar o ejecutar una acción: cuántos cambios se pueden deshacer y los avisos de los scripts del
/// formulario (`app.alert`, `console.println`, acciones bloqueadas).
#[derive(serde::Serialize)]
pub struct FormEdit {
    pub undo_depth: usize,
    pub messages: Vec<String>,
}

/// Rellena campos del PDF abierto (copia de seguridad antes del primer cambio) y recarga el visor. Ejecuta el
/// JavaScript del formulario (validación, cálculo y formato).
#[tauri::command]
pub async fn forms_fill(
    state: State<'_, AppState>,
    editor: State<'_, AnnotEditState>,
    doc_id: DocId,
    path: String,
    password: Option<String>,
    values: FieldValues,
    suffix: String,
) -> ApiResult<FormEdit> {
    let path = PathBuf::from(path);
    let (saved, messages) = editor
        .0
        .fill_report_saved(&path, password.as_deref(), &values, &suffix)?;
    state.service.reload_saved(doc_id, &path, &saved)?;
    Ok(FormEdit {
        undo_depth: editor.0.undo_depth(&path),
        messages,
    })
}

/// Ejecuta la acción de un botón (o de otro campo) del PDF abierto y recarga el visor.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn forms_run_action(
    state: State<'_, AppState>,
    editor: State<'_, AnnotEditState>,
    doc_id: DocId,
    path: String,
    password: Option<String>,
    name: String,
    trigger: String,
    suffix: String,
) -> ApiResult<FormEdit> {
    let path = PathBuf::from(path);
    let (saved, messages) = editor
        .0
        .run_action_saved(&path, password.as_deref(), &name, &trigger, &suffix)?;
    state.service.reload_saved(doc_id, &path, &saved)?;
    Ok(FormEdit {
        undo_depth: editor.0.undo_depth(&path),
        messages,
    })
}

/// Resultado de filtrar una pulsación (ver `pliego_forms::KeystrokeResult`).
#[tauri::command]
pub async fn forms_keystroke(
    path: String,
    password: Option<String>,
    name: String,
    change: String,
    current: String,
    sel_start: usize,
    sel_end: usize,
) -> ApiResult<annot::KeystrokeResult> {
    let joined = tauri::async_runtime::spawn_blocking(move || {
        annot::form_keystroke(
            &PathBuf::from(path),
            password.as_deref(),
            &name,
            &change,
            &current,
            (sel_start, sel_end),
        )
    })
    .await
    .map_err(|e| Error::Other(e.to_string()))?;
    Ok(joined?)
}
