//! Carga de **PDFium**, el motor que dibuja, extrae texto y busca (ola 5, W37; único motor desde W41).
//!
//! **Carga de la DLL.** Solo por ruta absoluta (nunca por `PATH` ni por el directorio actual: evita el secuestro de
//! DLL). [`pdfium_init_from_environment`] busca `pdfium.dll` junto al ejecutable y, para desarrollo y pruebas,
//! acepta `PLIEGO_PDFIUM_DLL` con la ruta absoluta de la DLL. Si nadie la ha cargado, la primera operación que la
//! necesite lo intenta de ese modo.

use std::path::{Path, PathBuf};

use pliego_core::{Error, Result};

/// `true` si la DLL de PDFium está cargada.
pub fn pdfium_available() -> bool {
    crate::pdfium::is_loaded()
}

/// Carga `pdfium.dll` (o `libpdfium.so`) desde `path`, que debe ser **absoluta**. Es idempotente: una segunda
/// llamada con la biblioteca ya cargada no hace nada.
pub fn pdfium_init(path: &Path) -> Result<()> {
    crate::pdfium::init(path)
}

/// Nombre del archivo de la biblioteca en esta plataforma.
pub fn pdfium_library_name() -> &'static str {
    if cfg!(windows) {
        "pdfium.dll"
    } else if cfg!(target_os = "macos") {
        "libpdfium.dylib"
    } else {
        "libpdfium.so"
    }
}

/// Ruta donde se espera la DLL: `PLIEGO_PDFIUM_DLL` si existe (absoluta), o la carpeta del ejecutable.
pub fn pdfium_default_path() -> Result<PathBuf> {
    if let Some(p) = std::env::var_os("PLIEGO_PDFIUM_DLL").filter(|p| !p.is_empty()) {
        return Ok(PathBuf::from(p));
    }
    let exe = std::env::current_exe().map_err(Error::from)?;
    let dir = exe
        .parent()
        .ok_or_else(|| Error::InvalidInput("el ejecutable no tiene carpeta".into()))?;
    Ok(dir.join(pdfium_library_name()))
}

/// Carga PDFium desde [`pdfium_default_path`] y devuelve la ruta usada.
pub fn pdfium_init_from_environment() -> Result<PathBuf> {
    let path = pdfium_default_path()?;
    pdfium_init(&path)?;
    Ok(path)
}

/// Configuración común de los programas de Pliego (worker, app, CLI): carga PDFium desde [`pdfium_default_path`] y
/// devuelve la ruta usada. Error si no está (sin ella no se puede dibujar ni extraer texto).
///
/// `PLIEGO_ENGINE` ya no elige motor: MuPDF no existe. Si vale `mupdf` se avisa por `stderr` y se ignora; cualquier
/// otro valor se ignora en silencio (`pdfium` y `auto` valían antes y siguen valiendo).
pub fn configure_from_environment() -> Result<PathBuf> {
    if std::env::var("PLIEGO_ENGINE").is_ok_and(|v| v.trim().eq_ignore_ascii_case("mupdf")) {
        eprintln!("pliego: PLIEGO_ENGINE=mupdf ya no existe (MuPDF se quitó); se usa PDFium");
    }
    pdfium_init_from_environment()
}
