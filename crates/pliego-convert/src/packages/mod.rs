//! Libros y documentos empaquetados en ZIP que se convierten a PDF: CBZ (cómic), EPUB (libro electrónico) y XPS
//! (documento de Windows). Sustituyen a la apertura directa que hacía MuPDF antes de W41.
//!
//! - [`cbz_to_pdf`]: una página por imagen, en orden natural de nombres.
//! - [`epub_to_pdf`]: el texto de cada capítulo del `spine` con el maquetador propio (HTML básico, sin CSS) y las
//!   imágenes de los capítulos (JPG, PNG, GIF, WebP, BMP).
//! - [`xps_to_pdf`]: páginas fijas con trazados, texto con su fuente incrustada e imágenes.
//!
//! No se abre nada de forma remota ni se ejecuta código: solo se leen entradas del ZIP con límite de tamaño.

mod cbz;
mod epub;
mod xps;
mod zipio;

pub use cbz::cbz_to_pdf;
pub use epub::epub_to_pdf;
pub use xps::xps_to_pdf;
