//! Autoactualizaciones de Pliego.
//!
//! Protocolo (detalle en `docs/design/actualizaciones.md`):
//!
//! 1. La app pide `<base>/v1/<canal>/manifest.json` a un servidor de archivos estáticos (el VPS de Ander,
//!    o `pliego-release serve` en pruebas).
//! 2. El manifiesto es un sobre con la carga útil en base64 y una o varias firmas Ed25519. Solo se acepta si lo
//!    firma una clave de confianza compilada en la app; el TLS es una defensa adicional, no la única.
//! 3. Se comprueba producto, canal, caducidad (contra ataques de congelación) y número de serie
//!    (contra la reproducción de manifiestos antiguos). Nunca se ofrece una versión igual o anterior a la instalada.
//! 4. El despliegue gradual se decide en local con un identificador de instalación aleatorio que no sale del equipo.
//! 5. La descarga se verifica por tamaño y SHA-256 antes de darla por buena.
//!
//! Esta crate no sabe nada de Tauri ni de Windows: la app y el instalador la usan como biblioteca.

mod client;
mod config;
mod env;
mod error;
mod install;
mod keys;
mod manifest;
pub mod platform;
pub mod restart;
mod rollout;
mod runtime;
pub mod schedule;
mod service;
mod stager;
mod state;

#[cfg(feature = "sign")]
pub mod sign;

pub use client::{CheckOutcome, UpdateClient, Verdict, evaluate};
pub use config::{Policy, UpdateConfig, UpdateMode, UserSettings, build_url, resolve};
pub use env::Environment;
pub use error::UpdateError;
pub use install::InstallKind;
pub use keys::{PublicKey, SIGNING_CONTEXT, release_keys};
pub use manifest::{Artifact, ArtifactKind, Envelope, FORMAT, Manifest, Release, SCHEMA, Signature};
pub use rollout::bucket;
pub use runtime::{Runner, Snapshot};
pub use semver::Version;
pub use service::{Phase, Stager, Status, UpdateService};
pub use stager::{LayoutStager, StagedPackage, file_sha256, staged_package};
pub use state::{HistoryEntry, PendingUpdate, RollbackNotice, UpdateState};
