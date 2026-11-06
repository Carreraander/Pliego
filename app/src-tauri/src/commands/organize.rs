//! Organizar y combinar (W23): la lógica está en `pliego_app_service::organize`; aquí solo se traducen argumentos.
//! Cada comando es un trabajo que **guarda una copia** junto al original y devuelve el `JobId`.

use std::path::PathBuf;

use pliego_app_service::organize::{self, InsertSource, MergeOptions, NumberRequest, PageEdit, SplitRule};
use pliego_appdata::jobs::JobId;
use tauri::State;

use crate::commands::jobs::JobsState;

/// Combina `paths` (en ese orden) en `<primero> (<suffix>).pdf`.
#[tauri::command]
pub fn merge_pdfs(
    jobs: State<'_, JobsState>,
    paths: Vec<String>,
    options: Option<MergeOptions>,
    suffix: Option<String>,
) -> JobId {
    let inputs: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    let suffix = suffix.unwrap_or_else(|| "combinado".into());
    jobs.0
        .spawn(move |ctx| organize::merge_copy(&inputs, &options.unwrap_or_default(), &suffix, ctx))
}

/// Divide `path` en la carpeta `<nombre> (<suffix>)` junto al original.
#[tauri::command]
pub fn split_pdf(
    jobs: State<'_, JobsState>,
    path: String,
    rule: SplitRule,
    password: Option<String>,
    suffix: Option<String>,
) -> JobId {
    let suffix = suffix.unwrap_or_else(|| "partes".into());
    jobs.0
        .spawn(move |ctx| organize::split_copy(&PathBuf::from(path), password.as_deref(), &rule, &suffix, ctx))
}

/// Inserta páginas en blanco (`count`) o de otro PDF (`source_path` + `pages` desde 0) antes de la página `at`.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn insert_pages(
    jobs: State<'_, JobsState>,
    path: String,
    password: Option<String>,
    at: u32,
    blank: Option<u32>,
    source_path: Option<String>,
    source_password: Option<String>,
    pages: Option<Vec<u32>>,
    suffix: Option<String>,
) -> JobId {
    let suffix = suffix.unwrap_or_else(|| "insertado".into());
    jobs.0.spawn(move |_ctx| {
        let source = match (blank, source_path) {
            (Some(count), _) => InsertSource::Blank { count },
            (None, Some(p)) => InsertSource::File {
                path: PathBuf::from(p),
                password: source_password,
                pages: pages.unwrap_or_default(),
            },
            (None, None) => {
                return Err(pliego_core::Error::InvalidInput(
                    "indica páginas en blanco o un archivo".into(),
                ));
            }
        };
        organize::insert_copy(&PathBuf::from(path), password.as_deref(), &source, at, &suffix)
    })
}

/// Imprime el número de página (como Encabezado y pie) y guarda una copia.
#[tauri::command]
pub fn number_pages(
    jobs: State<'_, JobsState>,
    path: String,
    password: Option<String>,
    request: NumberRequest,
    suffix: Option<String>,
) -> JobId {
    let suffix = suffix.unwrap_or_else(|| "numerado".into());
    jobs.0
        .spawn(move |ctx| organize::number_copy(&PathBuf::from(path), password.as_deref(), &request, &suffix, ctx))
}

/// Recorta las páginas (`pages` desde 0; vacío = todas) quitando `margins` (puntos) y guarda una copia.
#[tauri::command]
pub fn crop_pages(
    jobs: State<'_, JobsState>,
    path: String,
    password: Option<String>,
    pages: Vec<u32>,
    margins: organize::CropMargins,
    suffix: Option<String>,
) -> JobId {
    let suffix = suffix.unwrap_or_else(|| "recortado".into());
    jobs.0
        .spawn(move |ctx| organize::crop_copy(&PathBuf::from(path), password.as_deref(), &pages, margins, &suffix, ctx))
}

/// Guarda una copia con las páginas en el orden, giro y selección que dejó la persona en Organizar (también «Extraer»).
#[tauri::command]
pub fn organize_pages(
    jobs: State<'_, JobsState>,
    path: String,
    password: Option<String>,
    edits: Vec<PageEdit>,
    suffix: Option<String>,
) -> JobId {
    let suffix = suffix.unwrap_or_else(|| "organizado".into());
    jobs.0
        .spawn(move |ctx| organize::pages_copy(&PathBuf::from(path), password.as_deref(), &edits, &suffix, ctx))
}

/// Marcadores del documento para editarlos (con altura de destino y enlaces web).
#[tauri::command]
pub async fn outline_tree(
    path: String,
    password: Option<String>,
) -> pliego_app_service::api::ApiResult<Vec<pliego_organize::OutlineNode>> {
    let joined =
        tauri::async_runtime::spawn_blocking(move || organize::outline_tree(&PathBuf::from(path), password.as_deref()))
            .await
            .map_err(|e| pliego_core::Error::Other(e.to_string()))?;
    Ok(joined?)
}

/// Sustituye los marcadores del PDF abierto y recarga el visor. Devuelve cuántos cambios se pueden deshacer
/// (`annot_undo` deshace, como en los demás modos que editan el archivo).
#[tauri::command]
pub async fn outline_save(
    state: State<'_, crate::AppState>,
    editor: State<'_, crate::commands::annot::AnnotEditState>,
    doc_id: pliego_app_service::DocId,
    path: String,
    password: Option<String>,
    nodes: Vec<pliego_organize::OutlineNode>,
    suffix: String,
) -> pliego_app_service::api::ApiResult<usize> {
    let path = PathBuf::from(path);
    let bytes = organize::outline_save(&editor.0, &path, password.as_deref(), &nodes, &suffix)?;
    state.service.reload_bytes(doc_id, &bytes)?;
    Ok(editor.0.undo_depth(&path))
}
