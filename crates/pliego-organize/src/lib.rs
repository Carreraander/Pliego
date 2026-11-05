//! Organizar páginas: combinar, dividir, extraer, insertar, borrar, rotar, reordenar, recortar y
//! etiquetas de página.
//!
//! Las operaciones de un solo documento trabajan sobre `&mut PdfDocument` (páginas desde 0);
//! las de archivo (`merge`, `split`) leen rutas y escriben a un temporal que luego se renombra,
//! de modo que nunca se deja un archivo a medias.

mod annots;
mod io;
mod labels;
mod merge;
mod outline;
mod pages;
mod split;

pub use io::{open, save};
pub use labels::{LabelStyle, PageLabelSpec, set_page_labels};
pub use merge::{MergeOptions, merge};
pub use outline::{OutlineNode, read_outline, write_outline};
pub use pages::{
    CropSpec, crop, delete, duplicate, extract, insert_blank, insert_from, move_page, page_count, reorder, replace,
    rotate,
};
pub use split::{SplitRule, split};

pub use pliego_pdf::PdfDocument;
