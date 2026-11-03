//! Objetos vectoriales (trazados pintados): listar, mover/escalar/girar y borrar.
//!
//! Un objeto es un trazado desde su primer operador de construcción hasta el de pintado. Se identifica por
//! su orden de aparición (`id`), que solo vale mientras no cambie el contenido de la página. Los
//! recortes (`W n`) y lo que está dentro de XObjects de formulario no se tienen en cuenta.

use mupdf::Buffer;
use pliego_core::{Error, Report, Result};
use pliego_document::Document;
use serde::{Deserialize, Serialize};

use crate::content::{PathObject, parse, paths};
use crate::images::{Transform, local_transform, matrix_str, mul, page_bytes, valid_transform};
use crate::types::RectPts;
use crate::{load_page, pe};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VectorObject {
    pub page: usize,
    pub id: usize,
    /// Recuadro aproximado (puntos de control, sin el grosor del trazo) en el espacio MuPDF.
    pub bbox: RectPts,
}

fn bbox(o: &PathObject, p: &mupdf::Matrix) -> RectPts {
    let m = mul(&o.ctm, p);
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for &(x, y) in &o.points {
        let (tx, ty) = m.transform_xy(x, y);
        x0 = x0.min(tx);
        y0 = y0.min(ty);
        x1 = x1.max(tx);
        y1 = y1.max(ty);
    }
    [x0, y0, x1, y1]
}

/// Objetos vectoriales de la página (desde 0), en orden de dibujo.
pub fn vectors(doc: &Document, page: usize) -> Result<Vec<VectorObject>> {
    let pg = load_page(doc.pdf(), page)?;
    let p = pg.object().page_ctm().map_err(pe)?;
    let list = paths(&parse(&page_bytes(&pg)?));
    Ok(list
        .iter()
        .enumerate()
        .map(|(id, o)| VectorObject {
            page,
            id,
            bbox: bbox(o, &p),
        })
        .collect())
}

fn edit(
    doc: &mut Document,
    page: usize,
    id: usize,
    f: impl FnOnce(&PathObject, &mupdf::Matrix, &str) -> Result<String>,
) -> Result<()> {
    doc.with_pdf_mut(|pdf| {
        let pg = load_page(pdf, page)?;
        let bytes = page_bytes(&pg)?;
        let p = pg.object().page_ctm().map_err(pe)?;
        let o = paths(&parse(&bytes))
            .into_iter()
            .nth(id)
            .ok_or_else(|| Error::InvalidInput(format!("la página {} no tiene el objeto vectorial {id}", page + 1)))?;
        let original = String::from_utf8_lossy(&bytes[o.range.0..o.range.1]).into_owned();
        let replacement = f(&o, &p, &original)?;
        let mut out = Vec::with_capacity(bytes.len() + replacement.len());
        out.extend_from_slice(&bytes[..o.range.0]);
        out.extend_from_slice(replacement.as_bytes());
        out.extend_from_slice(&bytes[o.range.1..]);
        let buf = Buffer::try_from(out.as_slice()).map_err(pe)?;
        let stream = pdf.add_stream(&buf, None, false).map_err(pe)?;
        pg.object().dict_put("Contents", stream).map_err(pe)
    })
}

/// Borra el objeto vectorial `id` de la página.
pub fn delete_vector(doc: &mut Document, page: usize, id: usize) -> Result<Report> {
    edit(doc, page, id, |_, _, _| Ok(String::new()))?;
    Ok(Report {
        summary: format!("Objeto vectorial {id} borrado de la página {}", page + 1),
        ..Report::default()
    })
}

/// Mueve, escala y/o gira el objeto vectorial `id` (el grosor del trazo se escala con él).
pub fn transform_vector(doc: &mut Document, page: usize, id: usize, t: &Transform) -> Result<Report> {
    valid_transform(t)?;
    let t = *t;
    edit(doc, page, id, move |o, p, original| {
        let b = bbox(o, p);
        let m = local_transform(&o.ctm, p, &t, ((b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0))?;
        Ok(format!("q {} cm {original} Q", matrix_str(&m)))
    })?;
    Ok(Report {
        summary: format!("Objeto vectorial {id} transformado en la página {}", page + 1),
        ..Report::default()
    })
}
