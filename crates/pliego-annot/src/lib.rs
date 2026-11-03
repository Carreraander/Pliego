//! Anotaciones PDF de Pliego: crear, listar, editar y borrar sobre un `mupdf::pdf::PdfDocument`.
//!
//! Las funciones trabajan en memoria; guardar el documento es cosa de quien llama.

mod flatten;
mod ops;
mod summary;
mod types;

pub use flatten::flatten;
pub use ops::{add, delete, list, update};
pub use summary::{CommentSummary, Thread, summary};
pub use types::*;
