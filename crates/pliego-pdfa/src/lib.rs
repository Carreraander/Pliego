//! PDF/A: validación propia de las reglas más comunes y conversión a 1b, 2b, 2u y 3b.
//!
//! - [`convert`]: convierte un PDF al nivel pedido (o explica por qué no puede).
//! - [`validate`] / [`validate_doc`]: comprueba un documento contra un [`Level`].
//! - [`icc::srgb_profile`]: perfil ICC sRGB generado por código para la intención de salida.
//!
//! La validación propia no sustituye a veraPDF: avisa de los fallos más habituales sin necesitar Java.

mod convert;
mod icc;
mod io;
mod level;
mod substitute;
mod util;
mod validate;
mod xmp;

pub use convert::convert;
pub use icc::srgb_profile;
pub use level::Level;
pub use validate::{PdfaIssue, Rule, Validation, validate, validate_doc};
