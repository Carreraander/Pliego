//! Ajustes de la app (`%APPDATA%\Pliego\settings.json`). El esquema lo define y versiona la interfaz; aquí solo se
//! guarda y se recupera con seguridad (ver `pliego-appdata`).

use pliego_appdata::settings::SettingsStore;
use serde_json::Value;
use tauri::State;

pub struct SettingsState(pub SettingsStore);

/// Ajustes guardados, o `null` en el primer arranque.
#[tauri::command]
pub fn get_settings(state: State<'_, SettingsState>) -> Option<Value> {
    state.0.load()
}

#[tauri::command]
pub fn set_settings(state: State<'_, SettingsState>, value: Value) -> Result<(), String> {
    state.0.save(&value)
}
