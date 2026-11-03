//! Sistemas que no son Windows: el instalador solo existe para Windows, pero el motor se compila y prueba aquí.

use crate::cli::UiRequest;
use crate::model::{Result, Scope, exit};
use crate::system::{System, UninstallEntry};
use std::path::{Path, PathBuf};

pub struct Host;

pub fn system() -> Box<dyn System> {
    Box::new(Host)
}

pub fn run_ui(_: UiRequest) -> u32 {
    eprintln!("La interfaz del instalador solo está disponible en Windows. Usa --silent.");
    exit::FAILED
}

pub fn spawn_cleanup(_: &Path) {}
pub fn wait_for_exit(_: u32) {}
pub fn delete_self_later() {}

impl System for Host {
    fn is_admin(&self) -> bool {
        false
    }
    fn windows_build(&self) -> u32 {
        0
    }
    fn free_space(&self, _: &Path) -> Option<u64> {
        None
    }
    fn pliego_running(&self, _: &Path) -> bool {
        false
    }
    fn close_pliego(&self, _: &Path, _: u32) -> bool {
        true
    }
    fn register_uninstall(&self, _: Scope, _: &UninstallEntry) -> Result<()> {
        Ok(())
    }
    fn unregister_uninstall(&self, _: Scope) -> Result<()> {
        Ok(())
    }
    fn register_file_association(&self, _: Scope, _: &Path) -> Result<()> {
        Ok(())
    }
    fn unregister_file_association(&self, _: Scope) -> Result<()> {
        Ok(())
    }
    fn create_shortcuts(&self, _: Scope, _: &Path, _: bool) -> Result<Vec<PathBuf>> {
        Ok(vec![])
    }
    fn path_add(&self, _: Scope, _: &Path) -> Result<()> {
        Ok(())
    }
    fn path_remove(&self, _: Scope, _: &Path) -> Result<()> {
        Ok(())
    }
    fn shell_extensions(&self, _: &Path, _: bool, _: Scope) -> Result<()> {
        Ok(())
    }
    fn notify_assoc_changed(&self) {}
    fn open_default_apps(&self) {}
    fn open_url(&self, _: &str) {}
}
