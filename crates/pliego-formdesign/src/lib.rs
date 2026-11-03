//! Diseñador de formularios de Pliego sobre un `mupdf::pdf::PdfDocument`.
//!
//! Coordenadas: los rectángulos (`rect`) están en el espacio de usuario del PDF, es decir,
//! `[x0, y0, x1, y1]` con el origen en la esquina inferior izquierda de la página (el valor crudo de `/Rect`).
//! Las páginas empiezan en 0.

mod ap;
mod detect;
mod edit;
mod js;
mod objects;
mod read;
mod spec;
mod tab;

pub use detect::detect_fields;
pub use edit::{add_field, delete_field, update_field};
pub use js::{
    CalcOp, Calculation, FieldFormat, NumberFormat, Validation, get_calculation, recalculate, set_calculation,
    set_format, set_validation,
};
pub use read::list_fields;
pub use spec::{FieldPatch, FieldSpec, FieldType};
pub use tab::{set_tab_order, tab_order};

pub(crate) fn pe(e: mupdf::Error) -> pliego_core::Error {
    pliego_core::Error::Pdf(e.to_string())
}
