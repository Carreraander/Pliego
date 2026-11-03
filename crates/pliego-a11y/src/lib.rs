//! Accesibilidad de PDF: comprobador (reglas de PDF/UA y de lectores de pantalla) y correcciones.
//!
//! - [`check`] / [`check_file`]: informe con hallazgos y correcciones sugeridas.
//! - [`set_language`], [`set_title`], [`set_alt_text`], [`set_tab_order`]: correcciones sobre un documento abierto.
//! - [`autotag`] / [`autotag_file`]: etiquetado inicial con heurísticas (encabezados, listas, párrafos, figuras).
//! - [`fix_file`]: aplica varias correcciones a un archivo y guarda en otro.

mod autotag;
mod check;
mod content;
mod edit;
mod io;
mod report;
mod structure;

use std::path::Path;

pub use autotag::{AutotagOptions, AutotagSummary, autotag, autotag_file};
pub use check::{check, is_valid_language_tag};
pub use edit::{FixOptions, fix_file, set_alt_text, set_language, set_tab_order, set_title};
pub use report::{A11yIssue, A11yReport, IssueCode, Severity};

/// Abre `path` y lo comprueba.
pub fn check_file(path: &Path, password: Option<&str>) -> pliego_core::Result<A11yReport> {
    let doc = io::open_pdf(path, password)?;
    check(&doc)
}
