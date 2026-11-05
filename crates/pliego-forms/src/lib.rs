//! Formularios AcroForm de Pliego sobre un `pliego_pdf::PdfDocument`.

mod bulk;
mod csv;
mod data;
mod exchange;
mod fields;
mod xml;

pub use bulk::bulk_fill;
pub use data::{Format, export_data, import_data, import_data_bytes};
pub use fields::{FieldInfo, FieldKind, FieldValue, FieldValues, FillResult, fields, fill, flatten, reset};
