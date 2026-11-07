//! Diseño de formularios (W29b). La lógica con pruebas está en `pliego_app_service::formdesign`; aquí solo se
//! traducen argumentos. Los cambios van al propio PDF (copia de seguridad previa) y comparten la pila de deshacer de
//! Comentar (`annot_undo`). Los rectángulos están en el espacio de página del visor.

use std::path::PathBuf;

use pliego_app_service::DocId;
use pliego_app_service::api::ApiResult;
use pliego_app_service::formdesign::{self, FieldPatch, FieldSpec};
use pliego_core::Error;
use tauri::State;

use crate::AppState;
use crate::commands::annot::AnnotEditState;

/// Campos del formulario del PDF de `path`.
#[tauri::command]
pub async fn form_fields(path: String, password: Option<String>) -> ApiResult<Vec<FieldSpec>> {
    let joined =
        tauri::async_runtime::spawn_blocking(move || formdesign::fields(&PathBuf::from(path), password.as_deref()))
            .await
            .map_err(|e| Error::Other(e.to_string()))?;
    Ok(joined?)
}

/// Añade un campo y recarga el visor; devuelve cuántos cambios se pueden deshacer.
#[tauri::command]
pub async fn form_add_field(
    state: State<'_, AppState>,
    editor: State<'_, AnnotEditState>,
    doc_id: DocId,
    path: String,
    password: Option<String>,
    spec: FieldSpec,
    suffix: String,
) -> ApiResult<usize> {
    let path = PathBuf::from(path);
    let saved = formdesign::add_saved(&editor.0, &path, password.as_deref(), &spec, &suffix)?;
    state.service.reload_saved(doc_id, &path, &saved)?;
    Ok(editor.0.undo_depth(&path))
}

/// Cambia propiedades de un campo.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn form_update_field(
    state: State<'_, AppState>,
    editor: State<'_, AnnotEditState>,
    doc_id: DocId,
    path: String,
    password: Option<String>,
    name: String,
    patch: FieldPatch,
    suffix: String,
) -> ApiResult<usize> {
    let path = PathBuf::from(path);
    let saved = formdesign::update_saved(&editor.0, &path, password.as_deref(), &name, &patch, &suffix)?;
    state.service.reload_saved(doc_id, &path, &saved)?;
    Ok(editor.0.undo_depth(&path))
}

/// Borra un campo.
#[tauri::command]
pub async fn form_delete_field(
    state: State<'_, AppState>,
    editor: State<'_, AnnotEditState>,
    doc_id: DocId,
    path: String,
    password: Option<String>,
    name: String,
    suffix: String,
) -> ApiResult<usize> {
    let path = PathBuf::from(path);
    let saved = formdesign::delete_saved(&editor.0, &path, password.as_deref(), &name, &suffix)?;
    state.service.reload_saved(doc_id, &path, &saved)?;
    Ok(editor.0.undo_depth(&path))
}
