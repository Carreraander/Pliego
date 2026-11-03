//! Redacción irreversible y búsqueda de datos personales.
//!
//! Flujo: [`find_patterns`] (vista previa de coincidencias) → [`mark`] (marcas de redacción)
//! → [`apply`] (elimina de verdad texto, imágenes y vectores bajo las marcas) → [`verify`].
//!
//! [`apply`] comprueba el resultado antes de escribir el archivo: si algún texto redactado se
//! pudiera extraer o buscar, devuelve un error y no escribe nada.

mod apply;
mod patterns;
pub mod validators;

pub use apply::{Fill, ImageMode, Leftover, RedactJob, RedactOptions, apply, apply_document, mark, verify};
pub use patterns::{Area, Match, Pattern, find_patterns};
