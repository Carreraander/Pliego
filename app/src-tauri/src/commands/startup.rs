//! Archivos recibidos al arrancar (`pliego-app.exe archivo.pdf`) o desde una segunda instancia.

use std::path::Path;

use tauri::State;

use crate::AppState;

/// Rutas de PDF entre los argumentos de la línea de comandos (se ignoran el ejecutable y las opciones `-x`).
pub fn pdf_args<I: IntoIterator<Item = String>>(args: I) -> Vec<String> {
    args.into_iter()
        .skip(1)
        .filter(|a| !a.starts_with('-'))
        .filter(|a| Path::new(a).extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf")))
        .collect()
}

/// Entrega una sola vez los archivos con los que se arrancó la app.
#[tauri::command]
pub fn startup_files(state: State<'_, AppState>) -> Vec<String> {
    std::mem::take(&mut *state.startup_files.lock().unwrap_or_else(|e| e.into_inner()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn keeps_only_pdf_paths() {
        let got = pdf_args(v(&["pliego-app.exe", "a.pdf", "--flag", "b.PDF", "c.txt"]));
        assert_eq!(got, v(&["a.pdf", "b.PDF"]));
        assert!(pdf_args(v(&["pliego-app.exe"])).is_empty());
    }
}
