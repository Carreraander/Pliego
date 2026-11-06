//! Autoactualizaciones: capa fina de Tauri sobre `pliego-update` (todo lo que tiene lógica vive y se prueba allí).
//!
//! Comandos: `update_status`, `update_check_now`, `update_download`, `update_restart_now`, `update_set_settings`,
//! `update_skip_version` y `update_dismiss_notices`. Evento: `update-state-changed` con el mismo objeto que
//! devuelve `update_status`. Diseño en `docs/design/actualizaciones.md`.

use std::time::Duration;

use pliego_appdata::settings::SettingsStore;
use pliego_install::Layout;
use pliego_update::{
    Environment, LayoutStager, Phase, Runner, Snapshot, UpdateService, UserSettings, platform, restart, staged_package,
};
use serde_json::Value;
use tauri::{AppHandle, Emitter, State};

/// Estado compartido; `runner` es `None` si no se pudo abrir el estado local (las actualizaciones quedan apagadas).
pub struct UpdatesState {
    runner: Option<Runner>,
    env: Environment,
}

const NO_SERVICE: &str = "El servicio de actualizaciones no está disponible.";

/// Arranca el servicio con los ajustes guardados (`updates.mode|channel|metered`) y la directiva de la organización.
pub fn start(app: &AppHandle, settings: &SettingsStore) -> UpdatesState {
    let env = Environment::detect(env!("CARGO_PKG_VERSION"));
    pliego_core::log::info("updates", env.launch_log_line());
    if let Some(line) = env.rollback_log_line() {
        pliego_core::log::warn("updates", line);
    }
    mark_healthy_later(&env);
    let (user, respect_metered) = UserSettings::from_app_settings(settings.load().as_ref());
    let config = env.config(&user, &platform::policy());
    let stager = LayoutStager::new(env.kind, env.root.clone().unwrap_or_default(), &env.downloads);
    let service = UpdateService::open(
        config,
        env.kind,
        env.state_path.clone(),
        env.downloads.clone(),
        stager,
        env.rolled_back_from.as_ref(),
    );
    let runner = match service {
        Ok(service) => {
            let handle = app.clone();
            Some(Runner::start(
                service,
                respect_metered,
                platform::is_metered,
                move |snap| {
                    let _ = handle.emit("update-state-changed", snap);
                },
            ))
        }
        Err(e) => {
            pliego_core::log::error("updates", format!("actualizaciones desactivadas: {e}"));
            eprintln!("pliego: actualizaciones desactivadas: {e}");
            None
        }
    };
    UpdatesState { runner, env }
}

/// La app escribe `app-<versión>\healthy` tras 10 s funcionando; el lanzador lo usa para volver atrás si una versión
/// nueva no arranca.
fn mark_healthy_later(env: &Environment) {
    let Some(root) = env.root.clone() else { return };
    let version = env.current_version.to_string();
    std::thread::spawn(move || {
        std::thread::sleep(pliego_install::HEALTHY_AFTER);
        match Layout::new(root).mark_healthy(&version) {
            Ok(_) => pliego_core::log::info("updates", format!("la versión {version} arrancó bien")),
            Err(e) => pliego_core::log::warn(
                "updates",
                format!("no se pudo marcar la versión {version} como sana: {e}"),
            ),
        }
    });
}

fn runner(state: &UpdatesState) -> Result<&Runner, String> {
    state.runner.as_ref().ok_or_else(|| NO_SERVICE.to_owned())
}

#[tauri::command]
pub fn update_status(state: State<'_, UpdatesState>) -> Result<Snapshot, String> {
    Ok(runner(&state)?.snapshot())
}

#[tauri::command]
pub fn update_check_now(state: State<'_, UpdatesState>) -> Result<(), String> {
    runner(&state)?.check_now();
    Ok(())
}

/// "Descargar" (modo "solo avisar" o conexión de uso medido).
#[tauri::command]
pub fn update_download(state: State<'_, UpdatesState>) -> Result<(), String> {
    runner(&state)?.download();
    Ok(())
}

#[tauri::command]
pub fn update_skip_version(state: State<'_, UpdatesState>) -> Result<(), String> {
    runner(&state)?.skip_version();
    Ok(())
}

#[tauri::command]
pub fn update_dismiss_notices(state: State<'_, UpdatesState>) -> Result<(), String> {
    runner(&state)?.dismiss_notices();
    Ok(())
}

/// Cierra solo el aviso de vuelta atrás (la tarjeta de novedades pendiente se conserva).
#[tauri::command]
pub fn update_dismiss_rollback(state: State<'_, UpdatesState>) -> Result<(), String> {
    runner(&state)?.dismiss_rollback();
    Ok(())
}

/// Aplica el modo, el canal y la conexión medida de Ajustes > Actualizaciones (`{mode, channel, metered}`).
/// La directiva de la organización sigue mandando.
#[tauri::command]
pub fn update_set_settings(state: State<'_, UpdatesState>, value: Value) -> Result<(), String> {
    let (user, respect_metered) = UserSettings::from_app_settings(Some(&serde_json::json!({ "updates": value })));
    let config = state.env.config(&user, &platform::policy());
    runner(&state)?.apply_settings(config, respect_metered);
    Ok(())
}

/// "Reiniciar ahora": la interfaz ya pidió guardar los documentos. Lanza la orden de reinicio y cierra la app.
#[tauri::command]
pub fn update_restart_now(app: AppHandle, state: State<'_, UpdatesState>) -> Result<(), String> {
    let snap = runner(&state)?.snapshot();
    if !matches!(snap.status.phase, Phase::Ready { .. }) {
        return Err("No hay ninguna actualización preparada.".into());
    }
    let root = state
        .env
        .root
        .as_deref()
        .ok_or("Pliego no está instalado: no se puede reiniciar solo.")?;
    let staged = staged_package(&state.env.downloads);
    let cmd = restart::restart_command(
        state.env.kind,
        root,
        &state.env.current_version.to_string(),
        staged.as_ref(),
    )?;
    restart::spawn(&cmd).map_err(|e| format!("No se pudo reiniciar Pliego: {e}"))?;
    // Margen para que la interfaz termine de cerrarse antes de que la orden (que espera 2 s) abra la nueva versión.
    let handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(200));
        handle.exit(0);
    });
    Ok(())
}
