//! Edición de texto sobre el lienzo (W29b). La lógica con pruebas está en `pliego_app_service::pageedit`; aquí solo
//! se traducen argumentos. Los cambios van al propio PDF (copia de seguridad previa) y comparten la pila de deshacer
//! de Comentar (`annot_undo`). Las coordenadas son las del visor.

use std::path::PathBuf;

use pliego_app_service::DocId;
use pliego_app_service::api::ApiResult;
use pliego_app_service::pageedit::{
    self, Edited, ImageObject, LinkItem, LinkTarget, ObjectKind, RectPts, Style, TextBlock, Transform, VectorObject,
};
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

/// Imágenes colocadas en la página `page` (desde 0).
#[tauri::command]
pub async fn edit_images(path: String, page: usize) -> ApiResult<Vec<ImageObject>> {
    let joined = tauri::async_runtime::spawn_blocking(move || pageedit::images(&PathBuf::from(path), page, None))
        .await
        .map_err(|e| Error::Other(e.to_string()))?;
    Ok(joined?)
}

/// Objetos vectoriales de la página `page` (desde 0).
#[tauri::command]
pub async fn edit_vectors(path: String, page: usize) -> ApiResult<Vec<VectorObject>> {
    let joined = tauri::async_runtime::spawn_blocking(move || pageedit::vectors(&PathBuf::from(path), page, None))
        .await
        .map_err(|e| Error::Other(e.to_string()))?;
    Ok(joined?)
}

/// Enlaces de la página `page` (desde 0).
#[tauri::command]
pub async fn edit_links(path: String, page: usize) -> ApiResult<Vec<LinkItem>> {
    let joined = tauri::async_runtime::spawn_blocking(move || pageedit::links(&PathBuf::from(path), page, None))
        .await
        .map_err(|e| Error::Other(e.to_string()))?;
    Ok(joined?)
}

/// Inserta una imagen (archivo en disco) ocupando `rect`.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn edit_image_insert(
    state: State<'_, AppState>,
    editor: State<'_, AnnotEditState>,
    doc_id: DocId,
    path: String,
    page: usize,
    rect: RectPts,
    image: String,
    suffix: String,
) -> ApiResult<EditOutcome> {
    let path = PathBuf::from(path);
    let edited = pageedit::image_insert(&editor.0, &path, page, rect, &PathBuf::from(image), &suffix)?;
    finish(&state, &editor, doc_id, &path, edited)
}

/// Sustituye la imagen `id` por otra (archivo en disco).
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn edit_image_replace(
    state: State<'_, AppState>,
    editor: State<'_, AnnotEditState>,
    doc_id: DocId,
    path: String,
    page: usize,
    id: usize,
    image: String,
    suffix: String,
) -> ApiResult<EditOutcome> {
    let path = PathBuf::from(path);
    let edited = pageedit::image_replace(&editor.0, &path, page, id, &PathBuf::from(image), &suffix)?;
    finish(&state, &editor, doc_id, &path, edited)
}

/// Recorta la imagen `id` a `rect` (espacio de página).
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn edit_image_crop(
    state: State<'_, AppState>,
    editor: State<'_, AnnotEditState>,
    doc_id: DocId,
    path: String,
    page: usize,
    id: usize,
    rect: RectPts,
    suffix: String,
) -> ApiResult<EditOutcome> {
    let path = PathBuf::from(path);
    let edited = pageedit::image_crop(&editor.0, &path, page, id, rect, &suffix)?;
    finish(&state, &editor, doc_id, &path, edited)
}

/// Mueve, redimensiona y/o gira una imagen o un objeto vectorial.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn edit_object_transform(
    state: State<'_, AppState>,
    editor: State<'_, AnnotEditState>,
    doc_id: DocId,
    path: String,
    page: usize,
    kind: ObjectKind,
    id: usize,
    transform: Transform,
    suffix: String,
) -> ApiResult<EditOutcome> {
    let path = PathBuf::from(path);
    let edited = pageedit::object_transform(&editor.0, &path, page, kind, id, &transform, &suffix)?;
    finish(&state, &editor, doc_id, &path, edited)
}

/// Borra una imagen o un objeto vectorial.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn edit_object_delete(
    state: State<'_, AppState>,
    editor: State<'_, AnnotEditState>,
    doc_id: DocId,
    path: String,
    page: usize,
    kind: ObjectKind,
    id: usize,
    suffix: String,
) -> ApiResult<EditOutcome> {
    let path = PathBuf::from(path);
    let edited = pageedit::object_delete(&editor.0, &path, page, kind, id, &suffix)?;
    finish(&state, &editor, doc_id, &path, edited)
}

/// Crea un enlace en `rect`.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn edit_link_add(
    state: State<'_, AppState>,
    editor: State<'_, AnnotEditState>,
    doc_id: DocId,
    path: String,
    page: usize,
    rect: RectPts,
    target: LinkTarget,
    suffix: String,
) -> ApiResult<EditOutcome> {
    let path = PathBuf::from(path);
    let edited = pageedit::link_add(&editor.0, &path, page, rect, &target, &suffix)?;
    finish(&state, &editor, doc_id, &path, edited)
}

/// Cambia el destino del enlace `id`.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn edit_link_update(
    state: State<'_, AppState>,
    editor: State<'_, AnnotEditState>,
    doc_id: DocId,
    path: String,
    page: usize,
    id: usize,
    target: LinkTarget,
    suffix: String,
) -> ApiResult<EditOutcome> {
    let path = PathBuf::from(path);
    let edited = pageedit::link_update(&editor.0, &path, page, id, &target, &suffix)?;
    finish(&state, &editor, doc_id, &path, edited)
}

/// Borra el enlace `id`.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn edit_link_delete(
    state: State<'_, AppState>,
    editor: State<'_, AnnotEditState>,
    doc_id: DocId,
    path: String,
    page: usize,
    id: usize,
    suffix: String,
) -> ApiResult<EditOutcome> {
    let path = PathBuf::from(path);
    let edited = pageedit::link_delete(&editor.0, &path, page, id, &suffix)?;
    finish(&state, &editor, doc_id, &path, edited)
}
