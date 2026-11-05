//! Edición del contenido de páginas de Pliego (BUSL-1.1).
//!
//! Todas las operaciones trabajan sobre un [`pliego_document::Document`], de modo que cada una sube la
//! revisión y se puede deshacer. Las coordenadas son puntos PDF en el espacio de página del motor
//! (origen arriba a la izquierda, `y` hacia abajo, con la rotación de página aplicada).
//!
//! Al editar un bloque se retira el texto original y se compone el nuevo con reflujo dentro del bloque. Si la
//! fuente original (incrustada o no) tiene todos los caracteres nuevos se reutiliza; si no, se sustituye por una
//! estándar (Helvetica, Times, Courier) o una del sistema. Siempre se avisa en el [`Report`](pliego_core::Report).

mod blocks;
mod content;
mod embed;
mod fonts;
mod images;
mod links;
mod objects;
mod original;
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

use pliego_core::{Error, Result};
use pliego_pdf::{PdfDocument, PdfPage};

/// Carga una página como página PDF, validando el índice (desde 0).
pub(crate) fn load_page(pdf: &PdfDocument, page: usize) -> Result<PdfPage> {
    let count = pdf.page_count()?;
    if page >= count.max(0) as usize {
        return Err(Error::InvalidInput(format!(
            "la página {} no existe (el documento tiene {count})",
            page + 1
        )));
    }
    pdf.load_pdf_page(page as i32)
}
