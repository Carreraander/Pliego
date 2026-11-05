//! Escritura de PDF página a página a partir de otros documentos (p. ej. maquetados desde HTML).

use pliego_core::Result;

use crate::backend::{err, matrix_to, path_str, rect_to};
use crate::geom::{Matrix, Rect};
use crate::page::Page;

/// Escritor de un PDF nuevo. Al soltarlo se cierra el archivo; hasta entonces no está completo.
pub struct PdfWriter {
    raw: mupdf::DocumentWriter,
}

impl PdfWriter {
    /// Crea (o sobrescribe) `path`. `options` es la cadena de opciones del motor (normalmente vacía).
    pub fn new(path: impl AsRef<std::path::Path>, options: &str) -> Result<PdfWriter> {
        let p = path_str(path.as_ref())?;
        mupdf::DocumentWriter::new(p, "pdf", options)
            .map(|raw| PdfWriter { raw })
            .map_err(err)
    }

    /// Añade una página de caja `media_box` y dibuja en ella `page` con la matriz `ctm`.
    pub fn write_page(&mut self, media_box: Rect, page: &Page, ctm: &Matrix) -> Result<()> {
        let dev = self.raw.begin_page(rect_to(media_box)).map_err(err)?;
        page.base().run(&dev, &matrix_to(ctm)).map_err(err)?;
        self.raw.end_page(dev).map_err(err)
    }
}
