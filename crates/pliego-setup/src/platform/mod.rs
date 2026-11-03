//! Funciones del sistema. En Windows las implementa `windows.rs`; en el resto de sistemas hay versiones neutras
//! que bastan para compilar y probar el motor.

use crate::cli::UiRequest;
use crate::engine::DataDirs;
use crate::i18n::Lang;
use crate::model::Scope;
use crate::system::System;
use std::path::{Path, PathBuf};

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::{
    delete_self_later, ensure_webview2, error_box, installed, is_dark_theme, pick_folder, relaunch_elevated, run_ui,
    spawn_cleanup, system, wait_for_exit,
};

#[cfg(not(windows))]
mod other;
#[cfg(not(windows))]
pub use other::{delete_self_later, run_ui, spawn_cleanup, system, wait_for_exit};

fn env_dir(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from)
}

/// Carpeta de instalación por defecto.
pub fn default_dir(scope: Scope) -> PathBuf {
    match scope {
        Scope::User => env_dir("LOCALAPPDATA")
            .or_else(|| env_dir("HOME").map(|h| h.join(".local/share")))
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Programs")
            .join("Pliego"),
        Scope::Machine => env_dir("ProgramFiles")
            .unwrap_or_else(|| PathBuf::from("/opt"))
            .join("Pliego"),
    }
}

/// Ajustes (`%APPDATA%\Pliego`) y caché (`%LOCALAPPDATA%\Pliego`).
pub fn data_dirs() -> DataDirs {
    let home = env_dir("HOME").unwrap_or_else(|| PathBuf::from("."));
    DataDirs {
        settings: env_dir("APPDATA")
            .unwrap_or_else(|| home.join(".config"))
            .join("Pliego"),
        cache: env_dir("LOCALAPPDATA")
            .unwrap_or_else(|| home.join(".cache"))
            .join("Pliego"),
    }
}

pub fn ui_language() -> Lang {
    let tag = ["LC_ALL", "LANG"]
        .iter()
        .find_map(|n| std::env::var(n).ok())
        .unwrap_or_default();
    #[cfg(windows)]
    let tag = windows::locale_name().unwrap_or(tag);
    Lang::from_locale(&tag)
}

#[allow(dead_code)]
fn _assert_object_safe(_: &dyn System, _: &Path, _: UiRequest) {}
