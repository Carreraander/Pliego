//! PDF/A: validación propia de las reglas más comunes y conversión a 1b, 2b, 2u y 3b.
//!
//! - [`convert`]: convierte un PDF al nivel pedido (o explica por qué no puede).
//! - [`validate`] / [`validate_doc`]: comprueba un documento contra un [`Level`].
//! - [`update_info`]: cambia título, autor, asunto y palabras clave (y el XMP si es PDF/A).
//! - [`icc::srgb_profile`]: perfil ICC sRGB generado por código para la intención de salida.
//! - [`verapdf`]: validación completa opcional con veraPDF (descarga aparte, necesita Java).
//!
//! # Fuentes
//! PDF/A exige incrustar todas las fuentes. El motor trae las 14 fuentes estándar (Helvetica, Times,
//! Courier y variantes) pero no expone sus datos, así que las que el documento use sin incrustar se
//! sustituyen por una fuente Type3 vectorial equivalente (mismas anchuras, glifos dibujados con los
//! contornos de la fuente original) y una correspondencia a Unicode, de modo que el texto sigue siendo
//! buscable y copiable. Solo se hace con códigos WinAnsi; cualquier otra fuente sin incrustar hace
//! que [`convert`] falle con un mensaje que la nombra, en lugar de declarar PDF/A en falso.
//!
//! La validación propia no sustituye a veraPDF: avisa de los fallos más habituales sin necesitar Java.

mod convert;
mod icc;
mod info;
mod io;
mod level;
mod substitute;
mod util;
mod validate;
pub mod verapdf;
mod xmp;

pub use convert::convert;
pub use icc::srgb_profile;
pub use info::{InfoEdit, XmpSync, update_info};
pub use level::Level;
pub use validate::{PdfaIssue, Rule, Validation, validate, validate_doc, validate_with_password};
