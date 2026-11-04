//! Organizar y combinar (W23): Combinar archivos y Dividir. Son trabajos que **guardan una copia** junto al original
//! (`pliego_appdata::copyjob`) y devuelven el `JobId`; el centro de tareas ofrece «Abrir resultado».

use std::path::PathBuf;

use pliego_appdata::copyjob::{unique_in, unique_sibling};
use pliego_appdata::jobs::JobId;
use pliego_core::Error;
use pliego_organize::{MergeOptions, SplitRule};
use tauri::State;

use crate::commands::jobs::JobsState;

/// Combina `paths` (en ese orden) en `<primero> (combinado).pdf`.
#[tauri::command]
pub fn merge_pdfs(jobs: State<'_, JobsState>, paths: Vec<String>, options: Option<MergeOptions>) -> JobId {
    let inputs: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    jobs.0.spawn(move |ctx| {
        let first = inputs
            .first()
            .ok_or_else(|| Error::InvalidInput("no hay archivos que combinar".into()))?;
        let out = unique_sibling(first, "combinado", "pdf");
        pliego_organize::merge(&inputs, &out, &options.unwrap_or_default(), ctx)
    })
}

/// Divide `path` en la carpeta `<nombre> (partes)` junto al original.
#[tauri::command]
pub fn split_pdf(jobs: State<'_, JobsState>, path: String, rule: SplitRule, password: Option<String>) -> JobId {
    let src = PathBuf::from(path);
    jobs.0.spawn(move |ctx| {
        let doc = pliego_organize::open(&src, password.as_deref())?;
        let stem = src
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "documento".into());
        let dir = src.parent().map(PathBuf::from).unwrap_or_default();
        let out_dir = unique_in(&dir, &stem, "partes", "");
        pliego_organize::split(&doc, &stem, &rule, &out_dir, ctx)
    })
}
