//! Fuera de Windows no hay impresoras que manejar: las mismas funciones que `windows.rs`, que dan «no disponible».

use std::path::Path;

use pliego_core::{Error, JobContext, Report, Result};

use crate::devices::{PrinterCaps, PrinterEntry};
use crate::layout::SheetMetrics;
use crate::options::PrintOptions;

fn only_windows<T>() -> Result<T> {
    Err(Error::Unsupported("imprimir solo está disponible en Windows".into()))
}

pub fn default_printer() -> Option<String> {
    None
}

pub fn list_printers() -> Result<Vec<PrinterEntry>> {
    only_windows()
}

pub fn capabilities(_name: &str) -> Result<PrinterCaps> {
    only_windows()
}

pub fn sheet_in_points(_name: &str, _paper: Option<i16>, _landscape: bool) -> Result<SheetMetrics> {
    only_windows()
}

pub fn print_file(_path: &Path, _password: Option<&str>, options: &PrintOptions, _ctx: &JobContext) -> Result<Report> {
    options.validate()?;
    only_windows()
}
