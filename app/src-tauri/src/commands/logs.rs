//! Registro de diagnóstico: arranque del log y acceso a su carpeta. La lógica vive en `pliego_core::log`
//! (rotación, sin contenidos ni contraseñas) y `pliego_appdata::recents::open_folder_command`.

use std::path::Path;

/// Activa el log en `<datos>/logs`, anota el arranque y registra los pánicos. Si la carpeta no se puede escribir,
/// la app sigue sin log.
pub fn start(data_dir: &Path) {
    let dir = pliego_core::log::logs_dir(data_dir);
    if let Err(e) = pliego_core::log::init(&dir) {
        eprintln!("pliego: sin registro de diagnóstico en {}: {e}", dir.display());
        return;
    }
    pliego_core::log::info(
        "arranque",
        format!(
            "Pliego {} · {} {} · datos en {}",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
            std::env::consts::ARCH,
            data_dir.display()
        ),
    );
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let place = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_default();
        let hilo = std::thread::current();
        pliego_core::log::error(
            "pánico",
            format!("hilo {:?} en {place}: {info}", hilo.name().unwrap_or("sin nombre")),
        );
        previous(info);
    }));
}

/// Carpeta de registros (se crea si no existe) para mostrarla en Ajustes.
#[tauri::command]
pub fn logs_dir() -> String {
    let dir = pliego_core::log::logs_dir(&pliego_appdata::data_dir());
    let _ = std::fs::create_dir_all(&dir);
    dir.to_string_lossy().into_owned()
}

/// Abre la carpeta de registros en el explorador del sistema.
#[tauri::command]
pub fn open_logs_folder() -> Result<(), String> {
    let dir = logs_dir();
    let (program, args) = pliego_appdata::recents::open_folder_command(&dir);
    std::process::Command::new(program)
        .args(args)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("No se pudo abrir la carpeta de registros: {e}"))
}
