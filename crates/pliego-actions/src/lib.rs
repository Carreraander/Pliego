//! Acciones en lote: una secuencia de pasos (OCR → optimizar → proteger…) guardada en TOML o JSON que se
//! aplica a archivos y carpetas de PDFs, con progreso, informe y reanudación.
//!
//! Este crate **no depende de los módulos de funciones** (ver `docs/CONVENTIONS.md`): cada módulo se
//! enchufa mediante un [`StepRunner`] registrado en un [`Registry`]. Quien ensambla (la CLI, la app) registra
//! los pasos que quiera ofrecer. Las opciones de cada paso son las opciones serializables del módulo.

mod action;
mod inputs;
mod journal;
mod registry;
mod runner;

pub use action::{Action, ErrorPolicy, Step};
pub use inputs::{Input, collect_inputs};
pub use registry::{Registry, StepRunner};
pub use runner::{FileFailure, RunOptions, RunReport, run};
