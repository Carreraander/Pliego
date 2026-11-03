//! Anotaciones PDF de Pliego: crear, listar, editar y borrar sobre un `mupdf::pdf::PdfDocument`.
//!
//! Las funciones trabajan en memoria; guardar el documento es cosa de quien llama.

mod ops;
mod types;

pub use ops::{add, delete, list, update};
pub use types::*;
