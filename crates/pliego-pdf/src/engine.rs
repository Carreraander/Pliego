//! Elección del motor de **render, texto y búsqueda** (ola 5, W37).
//!
//! El modelo de objetos y el guardado siguen siendo de MuPDF hasta W36/W41; lo que se puede cambiar por documento
//! es quién dibuja y quién extrae el texto:
//!
//! - [`RenderBackend::Mupdf`] (por defecto): todo con MuPDF, como hasta ahora.
//! - [`RenderBackend::Pdfium`]: [`crate::Page::to_pixmap`], [`crate::Page::render_region`], [`crate::Page::bounds`],
//!   [`crate::Page::to_text_page`], [`crate::Page::text`], [`crate::Page::words`] y [`crate::Page::search`] usan PDFium.
//!   Requiere compilar con la característica `pdfium` y cargar `pdfium.dll` con [`pdfium_init`].
//!
//! El motor de un documento se fija al abrirlo (con el valor por defecto de [`default_render_backend`]) y se puede
//! cambiar con `Document::set_render_backend`. Una página lleva el motor del documento en el momento de
//! `load_page`.
//!
//! **Carga de la DLL.** Solo por ruta absoluta (nunca por `PATH` ni por el directorio actual: evita el secuestro de
//! DLL). [`pdfium_init_from_environment`] busca `pdfium.dll` junto al ejecutable y, para desarrollo y pruebas,
//! acepta `PLIEGO_PDFIUM_DLL` con la ruta absoluta de la DLL.
//!
//! **Respaldo.** Si PDFium no puede abrir un documento que MuPDF sí abre, con [`set_pdfium_fallback`]`(true)` la
//! capa sigue con MuPDF para ese documento y lo avisa por `stderr`; por defecto está desactivado para que el arnés
//! de comparación vea el fallo.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use pliego_core::{Error, Result};

/// Quién dibuja y extrae el texto de un documento.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum RenderBackend {
    #[default]
    Mupdf,
    Pdfium,
}

impl RenderBackend {
    /// Nombre estable (`"mupdf"`, `"pdfium"`), el mismo que usa el arnés de comparación y `PLIEGO_ENGINE`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Mupdf => "mupdf",
            Self::Pdfium => "pdfium",
        }
    }

    /// Interpreta un nombre (sin distinguir mayúsculas). `None` si no es uno de los motores.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "mupdf" => Some(Self::Mupdf),
            "pdfium" => Some(Self::Pdfium),
            _ => None,
        }
    }
}

static DEFAULT_IS_PDFIUM: AtomicBool = AtomicBool::new(false);
static FALLBACK: AtomicBool = AtomicBool::new(false);

/// Motor que usarán los documentos que se abran a partir de ahora.
pub fn default_render_backend() -> RenderBackend {
    if DEFAULT_IS_PDFIUM.load(Ordering::Relaxed) {
        RenderBackend::Pdfium
    } else {
        RenderBackend::Mupdf
    }
}

/// Fija el motor por defecto de los documentos que se abran después. `Pdfium` exige haber cargado la DLL antes
/// ([`pdfium_init`]); si no, error.
pub fn set_default_render_backend(backend: RenderBackend) -> Result<()> {
    if backend == RenderBackend::Pdfium && !pdfium_available() {
        return Err(Error::Unsupported(
            "PDFium no está disponible: compila con la característica `pdfium` y carga la DLL con `pdfium_init`".into(),
        ));
    }
    DEFAULT_IS_PDFIUM.store(backend == RenderBackend::Pdfium, Ordering::Relaxed);
    Ok(())
}

/// Con `true`, un documento que PDFium no abre sigue con MuPDF (y se avisa por `stderr`) en lugar de fallar.
pub fn set_pdfium_fallback(enabled: bool) {
    FALLBACK.store(enabled, Ordering::Relaxed);
}

pub fn pdfium_fallback() -> bool {
    FALLBACK.load(Ordering::Relaxed)
}

/// `true` si PDFium está compilado y la DLL cargada.
pub fn pdfium_available() -> bool {
    #[cfg(feature = "pdfium")]
    {
        crate::pdfium::is_loaded()
    }
    #[cfg(not(feature = "pdfium"))]
    {
        false
    }
}

/// Carga `pdfium.dll` (o `libpdfium.so`) desde `path`, que debe ser **absoluta**. Es idempotente: una segunda
/// llamada con la biblioteca ya cargada no hace nada.
pub fn pdfium_init(path: &Path) -> Result<()> {
    #[cfg(feature = "pdfium")]
    {
        crate::pdfium::init(path)
    }
    #[cfg(not(feature = "pdfium"))]
    {
        let _ = path;
        Err(Error::Unsupported(
            "esta compilación no incluye PDFium (característica `pdfium`)".into(),
        ))
    }
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

/// Configuración común de los programas de Pliego (worker, app, CLI) según el entorno:
///
/// - `PLIEGO_ENGINE=mupdf`: MuPDF, sin cargar PDFium.
/// - `PLIEGO_ENGINE=pdfium`: PDFium obligatorio; si la DLL no carga, error.
/// - sin variable o `auto`: PDFium si la DLL carga, MuPDF si no (se avisa por `stderr`).
///
/// En los modos que usan PDFium se activa el respaldo a MuPDF por documento ([`set_pdfium_fallback`]) salvo con
/// `PLIEGO_PDFIUM_STRICT=1`. Devuelve el motor por defecto resultante.
pub fn configure_from_environment() -> Result<RenderBackend> {
    let requested = std::env::var("PLIEGO_ENGINE").unwrap_or_default();
    let requested = requested.trim().to_ascii_lowercase();
    let strict = std::env::var("PLIEGO_PDFIUM_STRICT").is_ok_and(|v| v == "1");
    match requested.as_str() {
        "mupdf" => {
            set_default_render_backend(RenderBackend::Mupdf)?;
            Ok(RenderBackend::Mupdf)
        }
        "" | "auto" | "pdfium" => {
            let must = requested == "pdfium";
            match pdfium_init_from_environment() {
                Ok(_) => {
                    set_pdfium_fallback(!strict);
                    set_default_render_backend(RenderBackend::Pdfium)?;
                    Ok(RenderBackend::Pdfium)
                }
                Err(e) if must => Err(e),
                Err(e) => {
                    eprintln!("pliego: PDFium no disponible ({e}); se usa MuPDF");
                    Ok(RenderBackend::Mupdf)
                }
            }
        }
        other => Err(Error::InvalidInput(format!(
            "PLIEGO_ENGINE=«{other}» no válido; usa mupdf, pdfium o auto"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_names() {
        assert_eq!(RenderBackend::parse("PDFium"), Some(RenderBackend::Pdfium));
        assert_eq!(RenderBackend::parse(" mupdf "), Some(RenderBackend::Mupdf));
        assert_eq!(RenderBackend::parse("otro"), None);
        assert_eq!(RenderBackend::Pdfium.name(), "pdfium");
    }
}
