//! Sesión y recuperación tras un cierre inesperado (lógica en `pliego_appdata::session`).

use pliego_appdata::session::{PreviousRun, SessionDoc, SessionStore};
use tauri::State;

pub struct SessionState {
    pub store: SessionStore,
    /// Cómo acabó la ejecución anterior; se lee una vez al arrancar.
    pub previous: PreviousRun,
}

/// Cómo acabó la vez anterior: si se cerró de golpe y qué pestañas había abiertas.
#[tauri::command]
pub fn session_previous(state: State<'_, SessionState>) -> PreviousRun {
    state.previous.clone()
}

/// La interfaz lo llama al abrir, cerrar o cambiar de pestaña.
#[tauri::command]
pub fn session_save(state: State<'_, SessionState>, docs: Vec<SessionDoc>) -> Result<(), String> {
    state.store.save(&docs)
}

/// Informe de fallo para copiar (versión, sistema y final del registro). No sale del equipo.
#[tauri::command]
pub fn crash_report() -> String {
    pliego_appdata::session::crash_report(&pliego_appdata::data_dir(), env!("CARGO_PKG_VERSION"))
}
