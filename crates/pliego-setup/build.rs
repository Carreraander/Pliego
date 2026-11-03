//! Incrusta el manifiesto de Windows (Common-Controls 6 para `TaskDialog`, DPI por monitor, sin elevación).

fn main() {
    println!("cargo:rerun-if-changed=resources/pliego-setup.rc");
    println!("cargo:rerun-if-changed=resources/pliego-setup.manifest");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        // Si no hay `rc.exe`/`windres` (p. ej. al comprobar desde Linux) solo se avisa.
        let _ = embed_resource::compile("resources/pliego-setup.rc", embed_resource::NONE).manifest_optional();
    }
}
