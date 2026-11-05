use pliego_core::Result;
use pliego_pdf::PdfDocument;

use crate::ops::list;

/// Aplana las anotaciones: pasan a formar parte del contenido de la página y dejan de ser editables.
/// Los campos de formulario no se tocan (para eso está `pliego_forms::flatten`).
/// Devuelve cuántas anotaciones se aplanaron.
pub fn flatten(doc: &mut PdfDocument) -> Result<usize> {
    let before = list(doc, None)?.len();
    if before == 0 {
        return Ok(0);
    }
    doc.bake(true, false)?;
    Ok(before)
}
