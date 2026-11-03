fn main() {
    // `generate_context!` exige que exista la carpeta del frontend compilado. En CI y en pruebas de Rust
    // no se ejecuta `npm run build`, así que la creamos vacía si falta.
    let _ = std::fs::create_dir_all("../dist");
    tauri_build::build();
}
