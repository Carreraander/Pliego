//! Datos de la app de escritorio, independientes de Tauri (así se prueban sin enlazar WebView2):
//! ajustes (`%APPDATA%\Pliego\settings.json`), recientes y registro de trabajos en segundo plano.
//!
//! La capa Tauri (`app/src-tauri/src/commands/{settings,recents,jobs}.rs`) solo traduce comandos y eventos.

pub mod about;
pub mod copyjob;
pub mod jobs;
pub mod recents;
pub mod session;
pub mod settings;
pub mod weblink;

use std::path::PathBuf;

/// Carpeta de datos de Pliego: `%APPDATA%\Pliego` en Windows, `$XDG_CONFIG_HOME/Pliego` o `~/.config/Pliego` en el
/// resto. `PLIEGO_DATA_DIR` la sustituye (pruebas y desarrollo).
pub fn data_dir() -> PathBuf {
    resolve_data_dir(|k| std::env::var_os(k).map(PathBuf::from))
}

fn resolve_data_dir(env: impl Fn(&str) -> Option<PathBuf>) -> PathBuf {
    if let Some(dir) = env("PLIEGO_DATA_DIR") {
        return dir;
    }
    let base = env("APPDATA")
        .or_else(|| env("XDG_CONFIG_HOME"))
        .or_else(|| env("HOME").map(|h| h.join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("Pliego")
}

/// Escribe `bytes` en `path` sin dejar un archivo a medias: temporal en la misma carpeta y renombrado.
pub(crate) fn write_atomic(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_dir_prefers_override_then_appdata() {
        let env = |pairs: &'static [(&str, &str)]| {
            move |k: &str| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| PathBuf::from(v))
        };
        assert_eq!(
            resolve_data_dir(env(&[("PLIEGO_DATA_DIR", "/x"), ("APPDATA", "/a")])),
            PathBuf::from("/x")
        );
        assert_eq!(resolve_data_dir(env(&[("APPDATA", "/a")])), PathBuf::from("/a/Pliego"));
        assert_eq!(
            resolve_data_dir(env(&[("HOME", "/h")])),
            PathBuf::from("/h/.config/Pliego")
        );
    }
}
