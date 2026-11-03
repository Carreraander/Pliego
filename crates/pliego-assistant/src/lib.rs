//! Asistente local de Pliego: resume, responde preguntas citando páginas y extrae datos clave.
//!
//! Todo ocurre en el equipo del usuario: este crate solo abre conexiones de red para descargar un modelo
//! (característica `download`), y solo cuando la persona lo pide. El modelo de lenguaje se
//! accede a través del trait [`LlmBackend`] (el motor llama.cpp se implementa detrás de la característica
//! `llama`); la recuperación de pasajes (troceado por página, BM25 y, si el modelo lo permite, vectores)
//! funciona sin modelo y es lo que decide qué páginas se citan.
//!
//! El asistente está **desactivado por defecto** ([`AssistantConfig::default`]).

mod assistant;
mod backend;
mod chunk;
mod citations;
mod config;
#[cfg(feature = "download")]
mod download;
mod keydata;
#[cfg(feature = "llama")]
mod llama;
mod models;
mod retrieve;
mod text;

pub use assistant::{Answer, Assistant, Citation, Summary};
pub use backend::{GenerateRequest, LlmBackend, Message, Role};
pub use chunk::{Chunk, DocumentText};
pub use config::AssistantConfig;
#[cfg(feature = "download")]
pub use download::{download_file, download_model, installed_path};
pub use keydata::{KeyDatum, KeyKind, extract_key_data};
#[cfg(feature = "llama")]
pub use llama::LlamaCppBackend;
pub use models::{ModelSpec, Requirements, catalog, default_models_dir};
pub use retrieve::{DocIndex, Passage};
