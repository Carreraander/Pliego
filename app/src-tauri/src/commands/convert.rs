//! Convertir (W24): la lógica está en `pliego_app_service::convert`; aquí solo se traducen argumentos.

use std::path::PathBuf;

use pliego_app_service::convert::{self, CreateRequest, ExportRequest, OfficeRequest};
use pliego_appdata::jobs::JobId;
use tauri::State;

use crate::commands::jobs::JobsState;

/// Exporta `path` a imágenes (carpeta) o a texto/HTML (archivo) junto al original.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn export_pdf(
    jobs: State<'_, JobsState>,
    path: String,
    format: String,
    pages: String,
    dpi: Option<f32>,
    jpeg_quality: Option<u8>,
    password: Option<String>,
    label: String,
) -> JobId {
    let req = ExportRequest {
        format,
        pages,
        dpi,
        jpeg_quality,
        password,
        label,
    };
    jobs.0
        .spawn(move |ctx| convert::export_copy(&PathBuf::from(path), &req, ctx))
}

/// Exporta `path` a Word, Excel o PowerPoint junto al original.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn office_export_pdf(
    jobs: State<'_, JobsState>,
    path: String,
    format: String,
    pages: String,
    image_dpi: Option<f32>,
    fail_on_scanned: bool,
    password: Option<String>,
    label: String,
) -> JobId {
    let req = OfficeRequest {
        format,
        pages,
        image_dpi,
        fail_on_scanned,
        password,
        label,
    };
    jobs.0
        .spawn(move |ctx| convert::office_copy(&PathBuf::from(path), &req, ctx))
}

/// Crea un PDF a partir de imágenes, texto, Markdown, HTML o un documento de Office (`<primero> (PDF).pdf`).
#[tauri::command]
pub fn create_pdf(
    jobs: State<'_, JobsState>,
    paths: Vec<String>,
    page_size: String,
    margin_mm: f32,
    orientation: String,
    upscale: bool,
    label: String,
) -> JobId {
    let inputs: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    let req = CreateRequest {
        page_size,
        margin_mm,
        orientation,
        upscale,
        label,
    };
    jobs.0.spawn(move |ctx| convert::create_copy(&inputs, &req, ctx))
}

/// Guarda una copia PDF/A (`1b`, `2b`, `2u` o `3b`) junto al original.
#[tauri::command]
pub fn pdfa_convert(jobs: State<'_, JobsState>, path: String, level: String, label: String) -> JobId {
    jobs.0
        .spawn(move |ctx| convert::pdfa_copy(&PathBuf::from(path), &level, &label, ctx))
}
