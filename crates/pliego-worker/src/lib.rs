//! Aislamiento del motor MuPDF en un proceso hijo (AGPL-3.0-or-later).
//!
//! - [`protocol`]: mensajes entre la app y el proceso `pliego-worker`.
//! - [`WorkerEngine`]: implementa `PdfEngine`/`EngineDocument` hablando con el worker, intercambiable
//!   con `MupdfEngine`. Si el worker cae o deja de responder, se relanza y se reabre el documento.
//! - [`TileCache`]: caché LRU de teselas con límite de memoria.

pub mod cache;
pub mod client;
pub mod platform;
pub mod protocol;

pub use cache::{TileCache, TileKey};
pub use client::{WorkerConfig, WorkerDocument, WorkerEngine};
