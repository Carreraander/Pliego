//! OCR (W24): `pliego ocr` como proceso hijo; la lógica está en `pliego_app_service::ocr`.

use pliego_app_service::api::{ApiError, ApiResult};
use pliego_app_service::ocr::{self, OcrLanguages, OcrRequest};
use pliego_appdata::jobs::JobId;
use pliego_core::Error;
use tauri::State;

use crate::commands::jobs::JobsState;

/// Reconoce el texto de un PDF escaneado y guarda `<nombre> (<etiqueta>).pdf` junto al original.
#[tauri::command]
pub fn ocr_pdf(jobs: State<'_, JobsState>, request: OcrRequest) -> JobId {
    jobs.0.spawn(move |ctx| {
        let cli = ocr::find_cli()?;
        ocr::ocr_copy(&cli, &request, ctx)
    })
}

/// Idiomas de OCR instalados y descargables.
#[tauri::command]
pub async fn ocr_languages() -> ApiResult<OcrLanguages> {
    tauri::async_runtime::spawn_blocking(|| ocr::languages(&ocr::find_cli()?))
        .await
        .map_err(|e| ApiError::from(Error::Other(e.to_string())))?
        .map_err(ApiError::from)
}

/// Descarga el modelo de un idioma (con verificación de huella) a la carpeta de modelos del usuario.
#[tauri::command]
pub fn ocr_download_language(jobs: State<'_, JobsState>, code: String) -> JobId {
    jobs.0.spawn(move |ctx| {
        let cli = ocr::find_cli()?;
        ocr::download_language(&cli, &code, ctx)
    })
}
