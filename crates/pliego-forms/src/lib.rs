//! Formularios AcroForm de Pliego sobre un `mupdf::pdf::PdfDocument`.

mod bulk;
mod csv;
mod data;
mod fields;

pub use bulk::bulk_fill;
pub use data::{Format, export_data, import_data};
pub use fields::{FieldInfo, FieldKind, FieldValue, FieldValues, FillResult, fields, fill, flatten, reset};

pub(crate) fn pe(e: mupdf::Error) -> pliego_core::Error {
    pliego_core::Error::Pdf(e.to_string())
}
