//! Sellos y marcas (W27): Marca de agua, Encabezado y pie, Fondo y Bates. Cada una es un trabajo que **guarda una
//! copia** junto al original (`<nombre> (marca de agua).pdf`…) y devuelve el `JobId`. La lógica vive en
//! `pliego_stamp::stamp_file`, que se prueba sin Tauri.

use std::path::PathBuf;

use pliego_appdata::copyjob::unique_sibling;
use pliego_appdata::jobs::JobId;
use pliego_stamp::{StampOp, stamp_file};
use tauri::State;

use crate::commands::jobs::JobsState;

/// Aplica `op` a las páginas `pages` (`"all"`, `"1-3,5"`, `"odd"`…) de `path` y guarda una copia.
#[tauri::command]
pub fn stamp_pdf(
    jobs: State<'_, JobsState>,
    path: String,
    pages: String,
    op: StampOp,
    password: Option<String>,
) -> JobId {
    let src = PathBuf::from(path);
    jobs.0.spawn(move |ctx| {
        let out = unique_sibling(&src, op.suffix(), "pdf");
        stamp_file(&src, &out, password.as_deref(), &pages, &op, ctx)
    })
}
