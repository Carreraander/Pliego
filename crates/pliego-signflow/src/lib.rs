//! Flujo de firmas local: preparar un PDF para varios firmantes en orden, guiar al siguiente
//! firmante y reconstruir el registro de auditoría a partir de las propias firmas.
//!
//! El paquete es un PDF corriente con un adjunto `pliego-signflow.json` (la lista de firmantes, su orden
//! y la huella del documento original). Cada firma se añade por actualización incremental con
//! [`pliego_sign`], en un campo con nombre asignado al firmante; así la firma de cada persona cubre
//! el manifiesto y todas las firmas anteriores.

mod audit;
mod flow;
mod manifest;
mod package;

pub use audit::{AuditEntry, AuditEvent, audit_log};
pub use flow::{PendingSigner, next_signer, sign_next};
pub use manifest::{Manifest, SignerSpec, Signers};
pub use package::{Package, prepare, read_manifest};
