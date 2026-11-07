//! Medir (W29): la lógica con pruebas está en `pliego_app_service::measure`; aquí solo se traducen argumentos.
//! Los puntos están en el espacio de página del visor. Guardar escribe en el propio PDF (copia de seguridad previa)
//! y comparte la pila de deshacer de Comentar (`annot_undo`).

use std::path::PathBuf;

use pliego_app_service::DocId;
use pliego_app_service::api::ApiResult;
use pliego_app_service::measure::{self, Measurement, Pt, Scale, Unit};
use pliego_core::{Error, Lang};
use serde::Serialize;
use tauri::State;

use crate::AppState;
use crate::commands::annot::AnnotEditState;

/// Escala guardada en la página (`null` si no tiene).
#[tauri::command]
pub async fn measure_page_scale(path: String, page: u32, password: Option<String>) -> ApiResult<Option<Scale>> {
    let joined = tauri::async_runtime::spawn_blocking(move || {
        measure::page_scale(&PathBuf::from(path), password.as_deref(), page)
    })
    .await
    .map_err(|e| Error::Other(e.to_string()))?;
    Ok(joined?)
}

/// Escala por calibración: `page_points` puntos del papel equivalen a `real` `unit`.
#[tauri::command]
pub fn measure_calibrate(page_points: f64, real: f64, unit: Unit) -> ApiResult<Scale> {
    Ok(measure::calibrate(page_points, real, unit)?)
}

/// Escala 1:`n` expresada en `unit`.
#[tauri::command]
pub fn measure_ratio(n: f64, unit: Unit) -> ApiResult<Scale> {
    Ok(measure::ratio_scale(n, unit)?)
}

/// Mide una figura sin tocar el documento (para mostrar el resultado mientras se marcan puntos).
#[tauri::command]
pub fn measure_shape(scale: Scale, points: Vec<Pt>, closed: bool, lang: Option<String>) -> ApiResult<Measurement> {
    Ok(measure::measure(&scale, &points, closed, lang_of(lang))?)
}

/// Idioma del texto de la medida: el de la interfaz (`es` si no se indica).
fn lang_of(tag: Option<String>) -> Lang {
    tag.as_deref().map(Lang::from_tag).unwrap_or_default()
}

#[derive(Serialize)]
pub struct Saved {
    pub measurement: Measurement,
    /// Cuántos cambios se pueden deshacer.
    pub undo_depth: usize,
}

/// Guarda la escala y la anotación de medida en el PDF y recarga el visor.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn measure_save(
    state: State<'_, AppState>,
    editor: State<'_, AnnotEditState>,
    doc_id: DocId,
    path: String,
    password: Option<String>,
    page: u32,
    scale: Scale,
    points: Vec<Pt>,
    closed: bool,
    suffix: String,
    lang: Option<String>,
) -> ApiResult<Saved> {
    let path = PathBuf::from(path);
    let (saved, measurement) = measure::save_saved(
        &editor.0,
        &path,
        password.as_deref(),
        &suffix,
        page,
        &scale,
        &points,
        closed,
        lang_of(lang),
    )?;
    state.service.reload_saved(doc_id, &path, &saved)?;
    Ok(Saved {
        measurement,
        undo_depth: editor.0.undo_depth(&path),
    })
}
