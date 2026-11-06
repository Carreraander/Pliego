//! Núcleo de Pliego: tipos compartidos por todos los módulos.
//!
//! Reglas (ver `docs/CONVENTIONS.md`):
//! - Ningún módulo de funciones depende de otro módulo de funciones; solo de `pliego-core`
//!   y, si lo necesita, de `pliego-pdf` (única capa que habla con el motor PDF; nunca de `lopdf` ni de `pdfium-render`).
//! - Las operaciones largas reciben un [`JobContext`] para informar progreso y poder cancelarse.
//! - Las páginas se numeran desde 0 en la API y desde 1 en la CLI y la interfaz.

pub mod engine;
pub mod error;
pub mod job;
pub mod lang;
pub mod log;
pub mod pages;

pub use engine::{
    DocInfo, EngineDocument, Layer, OutlineItem, PageSize, PageWord, PdfEngine, RgbaImage, SearchHit, TileRequest,
};
pub use error::{Error, Result};
pub use job::{CancelToken, JobContext, NoProgress, Progress, Report};
pub use lang::Lang;
pub use pages::PageRange;
