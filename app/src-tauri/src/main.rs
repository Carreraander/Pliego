// Evita la ventana de consola extra en Windows en modo release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    pliego_app_lib::run();
}
