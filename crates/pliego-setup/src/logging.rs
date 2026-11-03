//! Registro en `%TEMP%\pliego-setup.log` (o `--log`). Nunca falla: si no se puede escribir, se ignora.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

static LOG: OnceLock<Mutex<PathBuf>> = OnceLock::new();

/// Ruta por defecto del registro.
pub fn default_path() -> PathBuf {
    std::env::temp_dir().join("pliego-setup.log")
}

pub fn init(path: Option<PathBuf>) {
    let _ = LOG.set(Mutex::new(path.unwrap_or_else(default_path)));
}

pub fn path() -> PathBuf {
    LOG.get()
        .and_then(|m| m.lock().ok().map(|p| p.clone()))
        .unwrap_or_else(default_path)
}

fn write(level: &str, msg: &str) {
    let p = path();
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(p) {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let _ = writeln!(f, "[{secs}] {level} {msg}");
    }
}

pub fn info(msg: &str) {
    write("INFO ", msg);
}
pub fn warn(msg: &str) {
    write("WARN ", msg);
}
pub fn error(msg: &str) {
    write("ERROR", msg);
}
