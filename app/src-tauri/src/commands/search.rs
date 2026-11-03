//! Búsqueda de texto.

use pliego_app_service::DocId;
use pliego_core::SearchHit;
use tauri::State;

use crate::AppState;
use crate::error::ApiResult;

const DEFAULT_MAX_RESULTS: u32 = 500;

#[tauri::command]
pub fn search(
    state: State<'_, AppState>,
    doc_id: DocId,
    query: String,
    max_results: Option<u32>,
) -> ApiResult<Vec<SearchHit>> {
    Ok(state
        .service
        .search(doc_id, &query, max_results.unwrap_or(DEFAULT_MAX_RESULTS))?)
}
