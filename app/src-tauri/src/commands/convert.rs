//! Convertir (W24): la lógica está en `pliego_app_service::convert`; aquí solo se traducen argumentos.

use std::path::PathBuf;

use pliego_app_service::convert::{CreateRequest, ExportRequest, OfficeRequest};
use pliego_app_service::isolated::JobSpec;
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
    jobs.spawn_tool(JobSpec::Export {
        path: PathBuf::from(path),
        req,
    })
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
    lang: Option<String>,
) -> JobId {
    let req = OfficeRequest {
        format,
        pages,
        image_dpi,
        fail_on_scanned,
        password,
        label,
        lang: lang.as_deref().map(pliego_core::Lang::from_tag).unwrap_or_default(),
    };
    jobs.spawn_tool(JobSpec::Office {
        path: PathBuf::from(path),
        req,
    })
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
    jobs.spawn_tool(JobSpec::Create { inputs, req })
}

/// Guarda una copia PDF/A (`1b`, `2b`, `2u` o `3b`) junto al original.
#[tauri::command]
pub fn pdfa_convert(jobs: State<'_, JobsState>, path: String, level: String, label: String) -> JobId {
    jobs.spawn_tool(JobSpec::Pdfa {
        path: PathBuf::from(path),
        level,
        label,
    })
}
