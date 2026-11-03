//! Funciones del sistema operativo: en Windows, `windows.rs`; en otros sistemas, versiones neutras.

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::{detach, error_box, init};

#[cfg(not(windows))]
pub fn init() {}
#[cfg(not(windows))]
pub fn error_box(msg: &str) {
    eprintln!("pliego: {msg}");
}
#[cfg(not(windows))]
pub fn detach(_cmd: &mut std::process::Command) {}
