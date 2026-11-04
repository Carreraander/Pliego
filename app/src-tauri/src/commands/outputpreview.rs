//! Vista previa de salida (W29): la lógica está en `pliego_app_service::outputpreview`; aquí solo se traducen argumentos.

use std::path::PathBuf;

use pliego_app_service::api::{ApiError, ApiResult};
use pliego_app_service::outputpreview::{self, OutputPreview};
use pliego_core::Error;

/// Planchas CMYK de la página `page` (desde 0) a `dpi` ppp, tinta total y tintas del documento.
#[tauri::command]
pub async fn output_preview(path: String, page: usize, dpi: f32, password: Option<String>) -> ApiResult<OutputPreview> {
    tauri::async_runtime::spawn_blocking(move || {
        outputpreview::output_preview(&PathBuf::from(path), password.as_deref(), page, dpi)
    })
    .await
    .map_err(|e| ApiError::from(Error::InvalidInput(e.to_string())))?
    .map_err(ApiError::from)
}
