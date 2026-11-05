//! Aislamiento del motor (lopdf + PDFium) en un proceso hijo (BUSL-1.1).
//!
//! - [`protocol`]: mensajes entre la app y el proceso `pliego-worker`.
//! - [`WorkerEngine`]: implementa `PdfEngine`/`EngineDocument` hablando con el worker, intercambiable
//!   con `DefaultEngine`. Si el worker cae o deja de responder, se relanza y se reabre el documento.
//! - [`TileCache`]: caché LRU de teselas con límite de memoria.

pub mod cache;
pub mod client;
pub mod platform;
pub mod protocol;

pub use cache::{TileCache, TileKey};
pub use client::{WorkerConfig, WorkerDocument, WorkerEngine};
