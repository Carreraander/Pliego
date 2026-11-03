//! PDF/A: validación propia de las reglas más comunes y (próximamente) conversión.
//!
//! - [`validate`] / [`validate_doc`]: comprueba un documento contra un [`Level`].
//! - [`icc::srgb_profile`]: perfil ICC sRGB generado por código para la intención de salida.
//!
//! La validación propia no sustituye a veraPDF: avisa de los fallos más habituales sin necesitar Java.

mod icc;
mod io;
mod level;
mod validate;

pub use icc::srgb_profile;
pub use level::Level;
pub use validate::{PdfaIssue, Rule, Validation, validate, validate_doc};
