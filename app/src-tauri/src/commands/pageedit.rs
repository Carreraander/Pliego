//! Edición de texto sobre el lienzo (W29b). La lógica con pruebas está en `pliego_app_service::pageedit`; aquí solo
//! se traducen argumentos. Los cambios van al propio PDF (copia de seguridad previa) y comparten la pila de deshacer
//! de Comentar (`annot_undo`). Las coordenadas son las del visor.

use std::path::PathBuf;

use pliego_app_service::DocId;
use pliego_app_service::api::ApiResult;
use pliego_app_service::pageedit::{self, Edited, RectPts, Style, TextBlock};
use pliego_core::Error;
use serde::Serialize;
use tauri::State;

use crate::AppState;
use crate::commands::annot::AnnotEditState;

/// Resultado de una edición: cuántos cambios se pueden deshacer y los avisos (cambio de fuente, etc.).
#[derive(Serialize)]
pub struct EditOutcome {
    pub undo_depth: usize,
    pub warnings: Vec<String>,
}

/// Bloques de texto editables de la página `page` (desde 0).
#[tauri::command]
pub async fn edit_text_blocks(path: String, page: usize) -> ApiResult<Vec<TextBlock>> {
    let joined = tauri::async_runtime::spawn_blocking(move || pageedit::blocks(&PathBuf::from(path), page, None))
        .await
        .map_err(|e| Error::Other(e.to_string()))?;
    Ok(joined?)
}

fn finish(
    state: &AppState,
    editor: &AnnotEditState,
    doc_id: DocId,
    path: &std::path::Path,
    edited: Edited,
) -> ApiResult<EditOutcome> {
    state.service.reload_bytes(doc_id, &edited.bytes)?;
    Ok(EditOutcome {
        undo_depth: editor.0.undo_depth(path),
        warnings: edited.report.warnings,
    })
}

/// Sustituye el texto de un bloque y recarga el visor.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn edit_text_replace(
    state: State<'_, AppState>,
    editor: State<'_, AnnotEditState>,
    doc_id: DocId,
    path: String,
    page: usize,
    block: usize,
    text: String,
    style: Style,
    flow: Option<bool>,
    suffix: String,
) -> ApiResult<EditOutcome> {
    let path = PathBuf::from(path);
    let edited = pageedit::replace(
        &editor.0,
        &path,
        page,
        block,
        &text,
        &style,
        flow.unwrap_or(true),
        &suffix,
    )?;
    finish(&state, &editor, doc_id, &path, edited)
}

/// Añade texto nuevo dentro de un recuadro y recarga el visor.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn edit_text_insert(
    state: State<'_, AppState>,
    editor: State<'_, AnnotEditState>,
    doc_id: DocId,
    path: String,
    page: usize,
    rect: RectPts,
    text: String,
    style: Style,
    suffix: String,
) -> ApiResult<EditOutcome> {
    let path = PathBuf::from(path);
    let edited = pageedit::insert(&editor.0, &path, page, rect, &text, &style, &suffix)?;
    finish(&state, &editor, doc_id, &path, edited)
}
