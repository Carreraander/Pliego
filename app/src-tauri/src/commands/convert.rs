//! Convertir (W24): la lógica está en `pliego_app_service::convert`; aquí solo se traducen argumentos.

use std::path::PathBuf;

use pliego_app_service::convert::{self, ExportRequest, OfficeRequest};
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
