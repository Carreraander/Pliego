//! Marcadores y texto de página (selección y copia).

use pliego_app_service::DocId;
use pliego_core::OutlineItem;
use tauri::State;

use crate::AppState;
use pliego_app_service::api::ApiResult;

#[tauri::command]
pub fn outline(state: State<'_, AppState>, doc_id: DocId) -> ApiResult<Vec<OutlineItem>> {
    Ok(state.service.outline(doc_id)?)
}

/// `page` empieza en 0.
#[tauri::command]
pub fn page_text(state: State<'_, AppState>, doc_id: DocId, page: u32) -> ApiResult<String> {
    Ok(state.service.page_text(doc_id, page)?)
}
