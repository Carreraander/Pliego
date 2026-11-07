//! Imprimir (W45): `pliego printer …` como proceso hijo; la lógica está en `pliego_app_service::print`.

use pliego_app_service::api::{ApiError, ApiResult};
use pliego_app_service::ocr::find_cli;
use pliego_app_service::print::{
    self, PreviewPlan, PrintOptions, PrintRequest, PrinterCaps, PrinterEntry, SheetMetrics,
};
use pliego_appdata::jobs::JobId;
use pliego_core::Error;
use tauri::State;

use crate::commands::jobs::JobsState;

/// Manda el PDF a la impresora como trabajo en segundo plano (progreso y cancelación en el centro de tareas).
#[tauri::command]
pub fn print_document(jobs: State<'_, JobsState>, request: PrintRequest) -> JobId {
    jobs.0.spawn(move |ctx| {
        let cli = find_cli()?;
        print::print_document(&cli, &request, ctx)
    })
}

/// Ejecuta una consulta del diálogo fuera del hilo de la interfaz (un controlador de red puede tardar).
async fn query<T: Send + 'static>(
    f: impl FnOnce(&std::path::Path) -> pliego_core::Result<T> + Send + 'static,
) -> ApiResult<T> {
    tauri::async_runtime::spawn_blocking(move || f(&find_cli()?))
        .await
        .map_err(|e| ApiError::from(Error::Other(e.to_string())))?
        .map_err(ApiError::from)
}

/// Impresoras instaladas (la predeterminada primero).
#[tauri::command]
pub async fn print_printers() -> ApiResult<Vec<PrinterEntry>> {
    query(print::printers).await
}

/// Papeles y capacidades de una impresora.
#[tauri::command]
pub async fn print_printer_caps(name: String) -> ApiResult<PrinterCaps> {
    query(move |cli| print::printer_caps(cli, &name)).await
}

/// Hoja de una impresora en puntos (papel y zona imprimible) para la vista previa.
#[tauri::command]
pub async fn print_sheet(name: String, paper: Option<i16>, landscape: bool) -> ApiResult<SheetMetrics> {
    query(move |cli| print::sheet(cli, &name, paper, landscape)).await
}

/// Cómo sale la primera cara en la hoja con las opciones elegidas, y cuántas caras son (las mismas cuentas que la
/// impresión). `sizes` son los tamaños `(ancho, alto)` en puntos de todas las páginas del documento.
#[tauri::command]
pub fn print_preview_layout(
    sizes: Vec<(f32, f32)>,
    sheet: SheetMetrics,
    options: PrintOptions,
) -> ApiResult<PreviewPlan> {
    print::preview_plan(&sizes, &sheet, &options).map_err(ApiError::from)
}

/// ¿La hoja del trabajo es apaisada? (decide qué hoja pedir a la impresora para la vista previa).
#[tauri::command]
pub fn print_preview_landscape(options: PrintOptions, first_width: f32, first_height: f32) -> ApiResult<bool> {
    print::preview_landscape(&options, (first_width, first_height)).map_err(ApiError::from)
}
