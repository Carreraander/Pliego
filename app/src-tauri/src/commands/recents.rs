//! Documentos recientes de la pantalla de Inicio.

use pliego_appdata::recents::{RecentStore, RecentView};
use tauri::State;
use tauri::ipc::Response;

use crate::AppState;

pub struct RecentsState(pub RecentStore);

#[tauri::command]
pub fn recents_list(state: State<'_, RecentsState>) -> Vec<RecentView> {
    state.0.list()
}

#[tauri::command]
pub fn recents_add(state: State<'_, RecentsState>, path: String) -> Result<(), String> {
    state.0.add(&path)
}

#[tauri::command]
pub fn recents_remove(state: State<'_, RecentsState>, path: String) -> Result<(), String> {
    state.0.remove(&path)
}

#[tauri::command]
pub fn recents_pin(state: State<'_, RecentsState>, path: String, pinned: bool) -> Result<(), String> {
    state.0.set_pinned(&path, pinned)
}

/// Miniatura (PNG) de la primera página de un reciente. Abre el documento un instante y lo cierra; si hace falta
/// contraseña o no se puede abrir, devuelve error y la interfaz muestra un icono.
#[tauri::command]
pub async fn recent_thumbnail(app_state: State<'_, AppState>, path: String) -> Result<Response, String> {
    use pliego_core::TileRequest;
    let service = app_state.service.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let doc = service
            .open_document(std::path::Path::new(&path), None)
            .map_err(|e| e.to_string())?;
        let png = service
            .page_sizes(doc)
            .map_err(|e| e.to_string())
            .and_then(|sizes| sizes.first().copied().ok_or_else(|| "sin páginas".to_string()))
            .and_then(|first| {
                // Miniatura de unos 120 px de ancho: una sola tesela.
                let zoom = (120.0 / first.width).clamp(0.05, 1.0);
                service
                    .tile(
                        doc,
                        TileRequest {
                            page: 0,
                            zoom,
                            col: 0,
                            row: 0,
                            size: 512,
                        },
                    )
                    .map_err(|e| e.to_string())
            });
        let _ = service.close_document(doc);
        png.map(Response::new)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Muestra el archivo en el explorador del sistema («Abrir ubicación»).
#[tauri::command]
pub fn reveal_path(path: String) -> Result<(), String> {
    let (program, args) = pliego_appdata::recents::reveal_command(&path);
    std::process::Command::new(program)
        .args(args)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("No se pudo abrir la ubicación: {e}"))
}

/// Datos de «Acerca de».
#[tauri::command]
pub fn about_info() -> pliego_appdata::about::AboutInfo {
    pliego_appdata::about::about(env!("CARGO_PKG_VERSION"))
}

/// Abre en el navegador (o el cliente de correo) la dirección de un enlace de un PDF. Solo `http`, `https` y `mailto`;
/// la interfaz pide confirmación antes de llamar.
#[tauri::command]
pub fn open_web_link(url: String) -> Result<(), String> {
    let url = pliego_appdata::weblink::check_url(&url)?;
    let (program, args) = pliego_appdata::weblink::open_command(&url);
    pliego_core::log::info("enlaces", "se abre un enlace web desde un documento");
    std::process::Command::new(program)
        .args(args)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("No se pudo abrir el enlace: {e}"))
}
