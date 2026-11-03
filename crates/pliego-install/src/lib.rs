//! Formato `.paq` y disposición en disco de una instalación de Pliego.
//!
//! - [`Layout`]: carpeta de instalación (`current`, `pending`, `app-<v>\`, `healthy`), activación atómica,
//!   vuelta atrás, limpieza y regla de salud. Sin dependencias: lo usa el lanzador.
//! - [`paq`] (característica `pack`): empaquetar y extraer con verificación.
//! - [`footer`] (característica `pack`): pegar un `.paq` al final de un ejecutable.

mod error;
mod layout;
mod version;

#[cfg(feature = "pack")]
pub mod footer;
#[cfg(feature = "pack")]
pub mod paq;

pub use error::{Error, Result};
pub use layout::{HEALTHY_AFTER, LaunchPlan, Layout, MAX_FAILED_STARTS};
pub use version::{cmp_versions, is_valid_version};
