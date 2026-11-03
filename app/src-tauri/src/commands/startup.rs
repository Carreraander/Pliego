//! Archivos recibidos al arrancar (`pliego-app.exe archivo.pdf`) o desde una segunda instancia.

use tauri::State;

use crate::AppState;

pub use pliego_app_service::api::pdf_args;

/// Entrega una sola vez los archivos con los que se arrancó la app.
#[tauri::command]
pub fn startup_files(state: State<'_, AppState>) -> Vec<String> {
    std::mem::take(&mut *state.startup_files.lock().unwrap_or_else(|e| e.into_inner()))
}
