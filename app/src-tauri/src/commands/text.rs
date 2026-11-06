//! Marcadores y texto de página (selección y copia).

use pliego_app_service::DocId;
use pliego_core::{Layer, OutlineItem, PageWord};
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

/// Capas del documento.
#[tauri::command]
pub fn layers(state: State<'_, AppState>, doc_id: DocId) -> ApiResult<Vec<Layer>> {
    Ok(state.service.layers(doc_id)?)
}

/// Muestra u oculta una capa solo en pantalla.
#[tauri::command]
pub fn set_layer_visible(state: State<'_, AppState>, doc_id: DocId, layer: i32, visible: bool) -> ApiResult<()> {
    state.service.set_layer_visible(doc_id, layer, visible)?;
    Ok(())
}

/// Palabras de la página con su caja (puntos de página, origen arriba a la izquierda): capa de texto del visor.
/// `page` empieza en 0.
#[tauri::command]
pub fn page_words(state: State<'_, AppState>, doc_id: DocId, page: u32) -> ApiResult<Vec<PageWord>> {
    Ok(state.service.page_words(doc_id, page)?)
}
