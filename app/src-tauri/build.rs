fn main() {
    // `generate_context!` exige que exista la carpeta del frontend compilado. En CI y en pruebas de Rust
    // no se ejecuta `npm run build`, así que la creamos vacía si falta.
    let _ = std::fs::create_dir_all("../dist");
    // Igual con el sidecar `pliego-worker` (`bundle.externalBin`): tauri-build exige que exista el archivo
    // `binaries/pliego-worker-<triple>[.exe]`. Fuera de `tauri build` basta un marcador vacío; el real lo deja
    // `npm run build:worker` (scripts/stage-worker.mjs).
    if let Ok(target) = std::env::var("TARGET") {
        let ext = if target.contains("windows") { ".exe" } else { "" };
        let _ = std::fs::create_dir_all("binaries");
        let path = std::path::PathBuf::from(format!("binaries/pliego-worker-{target}{ext}"));
        if !path.exists() {
            let _ = std::fs::write(path, []);
        }
    }
    tauri_build::build();
}
