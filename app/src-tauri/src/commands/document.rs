//! Abrir, cerrar y consultar documentos.

use std::path::PathBuf;

use pliego_app_service::DocId;
use pliego_app_service::passwords::{self, PasswordStatus};
use pliego_core::{DocInfo, PageSize};
use tauri::State;

use crate::AppState;
use crate::commands::assistant::AssistantState;
use pliego_app_service::api::{ApiError, ApiResult};

#[tauri::command]
pub fn open_document(state: State<'_, AppState>, path: String, password: Option<String>) -> ApiResult<DocId> {
    Ok(state.service.open_document(&PathBuf::from(path), password.as_deref())?)
}

/// ¿Pide contraseña `path`, y vale la dada? Para los archivos que un panel recibe además del abierto.
#[tauri::command]
pub async fn password_status(path: String, password: Option<String>) -> ApiResult<PasswordStatus> {
    let joined =
        tauri::async_runtime::spawn_blocking(move || passwords::status(&PathBuf::from(path), password.as_deref()))
            .await
            .map_err(|e| pliego_core::Error::Other(e.to_string()))?;
    Ok(joined?)
}

#[tauri::command]
pub fn close_document(
    state: State<'_, AppState>,
    assistant: State<'_, AssistantState>,
    doc_id: DocId,
) -> ApiResult<()> {
    state.service.close_document(doc_id)?;
    assistant.forget(doc_id);
    Ok(())
}

#[tauri::command]
pub fn document_info(state: State<'_, AppState>, doc_id: DocId) -> ApiResult<DocInfo> {
    Ok(state.service.document_info(doc_id)?)
}

#[tauri::command]
pub fn page_sizes(state: State<'_, AppState>, doc_id: DocId) -> ApiResult<Vec<PageSize>> {
    Ok(state.service.page_sizes(doc_id)?)
}

/// Para uso de otros módulos tras modificar el documento: sube la revisión y emite `document-changed`.
#[tauri::command]
pub fn bump_revision(state: State<'_, AppState>, doc_id: DocId) -> Result<u64, ApiError> {
    Ok(state.service.bump_revision(doc_id)?)
}

/// Tamaño y fecha del archivo de un documento (Propiedades del documento).
#[tauri::command]
pub fn file_facts(path: String) -> ApiResult<pliego_app_service::facts::FileFacts> {
    pliego_app_service::facts::file_facts(std::path::Path::new(&path)).map_err(ApiError::from)
}

/// «Guardar como»: copia el archivo del documento a `target` y devuelve la ruta final (con `.pdf`).
#[tauri::command]
pub async fn save_document_as(path: String, target: String) -> ApiResult<String> {
    let joined = tauri::async_runtime::spawn_blocking(move || {
        pliego_app_service::saveas::save_copy_as(std::path::Path::new(&path), std::path::Path::new(&target))
    })
    .await
    .map_err(|e| pliego_core::Error::Other(e.to_string()))?;
    Ok(joined?.to_string_lossy().into_owned())
}

/// Cambia título, autor, asunto y palabras clave del PDF abierto (con copia previa) y recarga el visor. Devuelve
/// cuántos cambios se pueden deshacer con `annot_undo`.
#[tauri::command]
pub async fn properties_save(
    state: State<'_, crate::AppState>,
    editor: State<'_, crate::commands::annot::AnnotEditState>,
    doc_id: pliego_app_service::DocId,
    path: String,
    password: Option<String>,
    edit: pliego_app_service::facts::InfoEdit,
    suffix: String,
) -> ApiResult<usize> {
    let path = PathBuf::from(path);
    let saved =
        pliego_app_service::facts::properties_save_saved(&editor.0, &path, password.as_deref(), &edit, &suffix)?;
    state.service.reload_saved(doc_id, &path, &saved)?;
    Ok(editor.0.undo_depth(&path))
}
