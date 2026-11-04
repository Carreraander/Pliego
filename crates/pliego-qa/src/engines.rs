//! Registro de motores por nombre para el arnés. Para añadir el motor nuevo (PDFium) basta una entrada en
//! [`by_name`]; el resto del arnés no cambia.

use std::path::Path;

use pliego_core::{EngineDocument, Error, PdfEngine, Result};
use pliego_engine_mupdf::MupdfEngine;

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
        Ok(Box::new(MupdfEngine.open(path, None)?))
    }
}

/// Nombres de motor disponibles.
pub const NAMES: &[&str] = &["mupdf"];

pub fn by_name(name: &str) -> Result<Box<dyn DynEngine>> {
    match name {
        "mupdf" => Ok(Box::new(Mupdf)),
        other => Err(Error::InvalidInput(format!(
            "motor desconocido «{other}»; disponibles: {}",
            NAMES.join(", ")
        ))),
    }
}
