//! Firma digital PAdES (ETSI EN 319 142) y validación de firmas para Pliego.
//!
//! - [`sign`]: firma por actualización incremental con cualquier [`Signer`].
//! - [`validate`]: comprueba integridad, firma y cadena de cada firma del documento.

mod cms;
mod pdfwrite;
mod sign;
mod signer;
mod validate;

pub use sign::{SignOptions, sign, sign_bytes};
pub use signer::{DigestAlg, Pkcs12Signer, SignatureScheme, Signer, load_certificate, subject_name};
pub use validate::{ChainStatus, SignatureReport, ValidationOptions, validate, validate_bytes, validate_with};
