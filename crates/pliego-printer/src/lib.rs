//! `pliego-printer`: imprimir un PDF en una impresora de Windows (W45).
//!
//! La interfaz muestra su propio diálogo (impresora, páginas, copias, ajuste a la hoja, orientación, dos caras, grises,
//! comentarios) y el trabajo lo hace este crate: aplana los campos de formulario en una copia en memoria
//! ([`prepare`]), abre el PDF en PDFium ([`pliego_pdf::print::PrintDocument`]), coloca cada página en la hoja
//! ([`layout`]) y la dibuja directamente en el contexto de dispositivo de la impresora (`FPDF_RenderPage`, GDI). Se
//! ejecuta en el proceso `pliego printer send` (no en la app) para que un PDF que tumbe PDFium no cierre la interfaz.
//!
//! Todo lo que no toca Windows (opciones, selección de páginas, maquetación, reparto de copias y el bucle del trabajo)
//! es independiente del sistema y se prueba en cualquier plataforma. Fuera de Windows las funciones de impresora dan
//! [`pliego_core::Error::Unsupported`].

pub mod devices;
pub mod job;
pub mod layout;
pub mod options;
#[cfg(not(windows))]
mod other;
pub mod prepare;
#[cfg(windows)]
mod windows;

#[cfg(not(windows))]
use other as sys;
#[cfg(windows)]
use windows as sys;

use std::path::Path;

pub use devices::{Paper, PrinterCaps, PrinterEntry};
pub use job::{JobSummary, PrintTarget, run_job};
pub use layout::{CopiesPlan, Placement, SheetMetrics, choose_landscape, place, plan_copies};
pub use options::{Duplex, Orientation, PrintOptions, ScaleMode, Subset};

use pliego_core::{JobContext, Report, Result};

/// Impresoras instaladas; la predeterminada primero.
pub fn list_printers() -> Result<Vec<PrinterEntry>> {
    sys::list_printers()
}

/// Nombre de la impresora predeterminada de Windows.
pub fn default_printer() -> Option<String> {
    sys::default_printer()
}

/// Capacidades de una impresora: papeles, dos caras, color, copias.
pub fn capabilities(printer: &str) -> Result<PrinterCaps> {
    sys::capabilities(printer)
}

/// Hoja de `printer` en puntos (tamaño del papel y márgenes no imprimibles) para el papel y la orientación dados.
pub fn sheet_in_points(printer: &str, paper: Option<i16>, landscape: bool) -> Result<SheetMetrics> {
    sys::sheet_in_points(printer, paper, landscape)
}

/// Imprime `path` (con `password` si está protegido). Devuelve el informe del trabajo.
pub fn print_file(path: &Path, password: Option<&str>, options: &PrintOptions, ctx: &JobContext) -> Result<Report> {
    sys::print_file(path, password, options, ctx)
}
