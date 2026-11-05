//! Registro de motores por nombre para el arnés. Hoy solo `pdfium` (render y texto con PDFium sobre la misma capa
//! `pliego-pdf`; la DLL se busca con `PLIEGO_PDFIUM_DLL` o junto al ejecutable). MuPDF ya no existe (W41): el arnés
//! sirve para comparar dos versiones del motor entre sí (p. ej. otra DLL) y para el oráculo de redacción.

use std::path::Path;

use pliego_core::{EngineDocument, Error, PdfEngine, Result};
use pliego_engine::DefaultEngine;

/// Motor con documentos de tipo borrado, para poder elegirlo en tiempo de ejecución.
pub trait DynEngine {
    fn name(&self) -> &str;
    fn open(&self, path: &Path) -> Result<Box<dyn EngineDocument>>;
}

struct Pdfium;

impl DynEngine for Pdfium {
    fn name(&self) -> &str {
        "pdfium"
    }
    fn open(&self, path: &Path) -> Result<Box<dyn EngineDocument>> {
        // Idempotente; falla con un mensaje claro si no se encuentra la DLL.
        pliego_pdf::pdfium_init_from_environment()?;
        Ok(Box::new(DefaultEngine.open(path, None)?))
    }
}

/// Nombres de motor disponibles.
pub const NAMES: &[&str] = &["pdfium"];

pub fn by_name(name: &str) -> Result<Box<dyn DynEngine>> {
    match name {
        "pdfium" => Ok(Box::new(Pdfium)),
        "mupdf" => Err(Error::InvalidInput(
            "el motor «mupdf» ya no existe (se quitó en W41); usa «pdfium»".into(),
        )),
        other => Err(Error::InvalidInput(format!(
            "motor desconocido «{other}»; disponibles: {}",
            NAMES.join(", ")
        ))),
    }
}
