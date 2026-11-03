//! Crear PDF desde otros formatos y exportar PDF a formatos comunes (flujo W09).
//!
//! Biblioteca primero: funciones sobre rutas, sin estado global. Las operaciones largas reciben un
//! [`pliego_core::JobContext`]. Las páginas se numeran desde 0 en la API.

mod export;
mod formats;
mod images;
mod structured;
mod util;

pub use export::{ExportImageFormat, PdfToImagesOptions, TextExportOptions, pdf_to_images, pdf_to_text};
pub use formats::{ExtractImagesOptions, extract_images, pdf_to_html, pdf_to_svg};
pub use images::{ImagesToPdfOptions, Orientation, PageSizeSpec, images_to_pdf};
pub use structured::{BBox, Block, Line, Span, StructuredPage, structured_text, structured_text_pages};
