//! Crear PDF desde otros formatos y exportar PDF a formatos comunes (flujo W09).
//!
//! Biblioteca primero: funciones sobre rutas, sin estado global. Las operaciones largas reciben un
//! [`pliego_core::JobContext`]. Las páginas se numeran desde 0 en la API.

mod export;
mod images;
mod util;

pub use export::{ExportImageFormat, PdfToImagesOptions, TextExportOptions, pdf_to_images, pdf_to_text};
pub use images::{ImagesToPdfOptions, Orientation, PageSizeSpec, images_to_pdf};
