//! Registro de motores por nombre para el arnés. Hoy: `mupdf` (motor de referencia) y `pdfium` (render y texto con
//! PDFium sobre la misma capa `pliego-pdf`; la DLL se busca con `PLIEGO_PDFIUM_DLL` o junto al ejecutable).

use std::path::Path;

use pliego_core::{EngineDocument, Error, Result};
use pliego_engine::{DefaultEngine, RenderBackend};

/// Motor con documentos de tipo borrado, para poder elegirlo en tiempo de ejecución.
pub trait DynEngine {
    fn name(&self) -> &str;
    fn open(&self, path: &Path) -> Result<Box<dyn EngineDocument>>;
}

struct Mupdf;

impl DynEngine for Mupdf {
    fn name(&self) -> &str {
        "mupdf"
    }
    fn open(&self, path: &Path) -> Result<Box<dyn EngineDocument>> {
        Ok(Box::new(DefaultEngine.open_with_backend(
            path,
            None,
            RenderBackend::Mupdf,
        )?))
    }
}

struct Pdfium;

impl DynEngine for Pdfium {
    fn name(&self) -> &str {
        "pdfium"
    }
    fn open(&self, path: &Path) -> Result<Box<dyn EngineDocument>> {
        // Idempotente; falla con un mensaje claro si no se encuentra la DLL.
        pliego_pdf::pdfium_init_from_environment()?;
        Ok(Box::new(DefaultEngine.open_with_backend(
            path,
            None,
            RenderBackend::Pdfium,
        )?))
    }
}

/// Nombres de motor disponibles.
pub const NAMES: &[&str] = &["mupdf", "pdfium"];

pub fn by_name(name: &str) -> Result<Box<dyn DynEngine>> {
    match name {
        "mupdf" => Ok(Box::new(Mupdf)),
        "pdfium" => Ok(Box::new(Pdfium)),
        other => Err(Error::InvalidInput(format!(
            "motor desconocido «{other}»; disponibles: {}",
            NAMES.join(", ")
        ))),
    }
}
