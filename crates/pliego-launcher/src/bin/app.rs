//! `Pliego.exe`: lanzador de la app, sin consola.
#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() -> std::process::ExitCode {
    pliego_launcher::main(pliego_launcher::Target::App)
}
