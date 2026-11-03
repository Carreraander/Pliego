//! Edición del contenido de páginas de Pliego (AGPL-3.0-or-later).
//!
//! Todas las operaciones trabajan sobre un [`pliego_document::Document`], de modo que cada una sube la
//! revisión y se puede deshacer. Las coordenadas son puntos PDF en el espacio de página de MuPDF
//! (origen arriba a la izquierda, `y` hacia abajo, con la rotación de página aplicada).
//!
//! De momento el texto se reescribe con fuentes estándar (Helvetica, Times, Courier): al editar un bloque se
//! retira el texto original y se compone el nuevo con reflujo dentro del bloque. Siempre se avisa de la
//! sustitución de fuente en el [`Report`](pliego_core::Report).

mod blocks;
mod content;
mod embed;
mod fonts;
mod images;
mod links;
mod objects;
mod text;
mod types;
mod unicode;

pub use blocks::text_blocks;
pub use images::{
    ImageObject, Transform, crop_image, delete_object, images, insert_image, replace_image, transform_object,
};
pub use links::add_link;
pub use objects::{VectorObject, delete_vector, transform_vector, vectors};
pub use text::{insert_text, replace_text};
pub use types::*;

use mupdf::pdf::PdfPage;
use pliego_core::{Error, Result};

pub(crate) fn pe(e: mupdf::Error) -> Error {
    Error::Pdf(e.to_string())
}

/// Carga una página como página PDF, validando el índice (desde 0).
pub(crate) fn load_page(pdf: &mupdf::pdf::PdfDocument, page: usize) -> Result<PdfPage> {
    let count = pdf.page_count().map_err(pe)?;
    if page >= count.max(0) as usize {
        return Err(Error::InvalidInput(format!(
            "la página {} no existe (el documento tiene {count})",
            page + 1
        )));
    }
    let p = pdf.load_page(page as i32).map_err(pe)?;
    PdfPage::try_from(p).map_err(pe)
}
