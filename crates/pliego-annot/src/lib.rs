//! Anotaciones PDF de Pliego: crear, listar, editar y borrar sobre un `mupdf::pdf::PdfDocument`.
//!
//! Las funciones trabajan en memoria; guardar el documento es cosa de quien llama.

mod flatten;
mod ops;
mod stamp;
mod summary;
mod types;
mod xfdf;
mod xml;

pub use flatten::flatten;
pub use ops::{add, delete, list, update};
pub use summary::{CommentSummary, Thread, summary};
pub use types::*;
pub use xfdf::{export_xfdf, import_xfdf};
