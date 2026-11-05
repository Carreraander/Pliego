//! Firma digital PAdES (ETSI EN 319 142) y validación de firmas para Pliego.
//!
//! - [`sign`]: firma por actualización incremental con cualquier [`Signer`].
//! - [`validate`]: comprueba integridad, firma y cadena de cada firma del documento.
//! - En Windows, `WindowsStoreSigner` firma con el almacén de certificados del usuario (CNG/NCrypt).

mod appearance;
mod changes;
mod cms;
mod doctimestamp;
mod dss;
mod pdfwrite;
mod platform;
mod sign;
mod signer;
mod timestamp;
mod validate;

pub use dss::{
    HttpRevocation, RevocationData, RevocationSource, StaticRevocation, add_dss, add_dss_vri, build_ocsp_request,
    collect_revocation, ocsp_response_is_successful,
};
#[cfg(windows)]
pub use platform::windows::{WindowsCertificate, WindowsStoreSigner, list_windows_certificates};
pub use sign::{
    PadesLevel, SignLang, SignOptions, VisibleSignature, sign, sign_bytes, sign_bytes_with, sign_bytes_with_tsa,
};
pub use signer::{DigestAlg, Pkcs12Signer, SignatureScheme, Signer, load_certificate, subject_name};
pub use timestamp::{HttpTsa, LocalTsa, TimestampAuthority, TimestampInfo};
pub use validate::{ChainStatus, SignatureReport, ValidationOptions, validate, validate_bytes, validate_with};
