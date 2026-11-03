//! Pliego-Setup.exe / Uninstall.exe. Ver `docs/design/instalador.md`.
//!
//! Un único ejecutable: lleva el paquete `.paq` pegado al final (instalador) o, copiado como `Uninstall.exe`,
//! sirve para desinstalar y para las tareas posteriores a actualizar.
#![cfg_attr(windows, windows_subsystem = "windows")]
// Fuera de Windows no existe la interfaz y parte del motor queda sin uso.
#![cfg_attr(not(windows), allow(dead_code))]

mod cli;
mod engine;
mod i18n;
mod logging;
mod model;
mod pathenv;
mod platform;
mod system;
#[cfg(windows)]
mod ui;

fn main() {
    // `std::process::exit` admite el código completo de Windows (1602, 1603…); `ExitCode` solo un byte.
    std::process::exit(cli::run(std::env::args_os().skip(1).collect()) as i32);
}
