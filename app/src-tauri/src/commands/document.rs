//! Abrir, cerrar y consultar documentos.

use std::path::PathBuf;

use pliego_app_service::DocId;
use pliego_core::{DocInfo, PageSize};
use tauri::State;

use crate::AppState;
use crate::commands::assistant::AssistantState;
use pliego_app_service::api::{ApiError, ApiResult};

#[tauri::command]
pub fn open_document(state: State<'_, AppState>, path: String, password: Option<String>) -> ApiResult<DocId> {
    Ok(state.service.open_document(&PathBuf::from(path), password.as_deref())?)
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
