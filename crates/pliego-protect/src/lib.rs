//! Proteger documentos: cifrado AES, contraseñas, permisos y saneado.
//!
//! - [`encrypt`] / [`decrypt`] / [`set_permissions`] trabajan sobre rutas y nunca sobrescriben la entrada.
//! - [`sanitize`] quita información oculta y devuelve un informe de lo eliminado.
//!
//! Cifrado por certificado (/PubSec): MuPDF 0.8 solo escribe cifrado por contraseña, así que queda para la fase 2.

mod encrypt;
mod io;
mod sanitize;

pub use encrypt::{Cipher, Encryption, Permissions, decrypt, encrypt, permissions_of, set_permissions};
pub use sanitize::{RemovedItem, SanitizeOptions, sanitize, sanitize_document};
