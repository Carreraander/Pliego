//! Incrusta el icono de Pliego en `Pliego.exe` y `bin\pliego.exe` (sin él, Windows muestra el icono genérico).

fn main() {
    println!("cargo:rerun-if-changed=resources/pliego-launcher.rc");
    println!("cargo:rerun-if-changed=../../app/src-tauri/icons/icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        // Si no hay `rc.exe`/`windres` (p. ej. al comprobar desde Linux) solo se avisa.
        let _ = embed_resource::compile("resources/pliego-launcher.rc", embed_resource::NONE).manifest_optional();
    }
}
