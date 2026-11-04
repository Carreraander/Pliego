//! Detectar campos (W29): la lógica está en `pliego_app_service::formdesign::detect_copy`; aquí solo se traducen argumentos.

use std::path::PathBuf;

use pliego_app_service::formdesign;
use pliego_appdata::jobs::JobId;
use tauri::State;

use crate::commands::jobs::JobsState;

/// Detecta campos en el documento y guarda una copia `<nombre> (<suffix>).pdf` con ellos creados.
#[tauri::command]
pub fn form_detect(jobs: State<'_, JobsState>, path: String, password: Option<String>, suffix: String) -> JobId {
    let path = PathBuf::from(path);
    jobs.0
        .spawn(move |ctx| formdesign::detect_copy(&path, password.as_deref(), &suffix, ctx))
}
