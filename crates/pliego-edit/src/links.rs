//! Enlaces internos y externos.

use pliego_core::{Error, Report, Result};
use pliego_document::Document;
use pliego_pdf::{DestinationKind, LinkTarget as PdfLinkTarget, NewLink, Rect};

use crate::load_page;
use crate::types::{LinkTarget, RectPts};

/// Crea un enlace en `rect` de la página `page`.
pub fn add_link(doc: &mut Document, page: usize, rect: RectPts, target: &LinkTarget) -> Result<Report> {
    if rect[2] <= rect[0] || rect[3] <= rect[1] {
        return Err(Error::InvalidInput("el área del enlace está vacía".into()));
    }
    let action = match target {
        LinkTarget::Uri { uri } => {
            if !uri.is_ascii() || uri.trim().is_empty() {
                return Err(Error::InvalidInput(
                    "la dirección del enlace debe ser ASCII y no estar vacía".into(),
                ));
            }
            PdfLinkTarget::Uri(uri.clone())
        }
        LinkTarget::Page { page: dest } => {
            let count = doc.pdf().page_count()?.max(0) as usize;
            if *dest >= count {
                return Err(Error::InvalidInput(format!(
                    "la página de destino {} no existe",
                    dest + 1
                )));
            }
            PdfLinkTarget::Page {
                page: *dest as u32,
                kind: DestinationKind::Fit,
            }
        }
    };
    doc.with_pdf_mut(|pdf| {
        let mut p = load_page(pdf, page)?;
        let link = NewLink {
            bounds: Rect::new(rect[0], rect[1], rect[2], rect[3]),
            target: action,
        };
        p.add_links(pdf, &[link])
    })?;
    Ok(Report {
        summary: format!("Enlace añadido en la página {}", page + 1),
        ..Report::default()
    })
}

/// Enlace existente de una página.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LinkItem {
    pub page: usize,
    /// Posición en `/Annots`; vale mientras no se borren o añadan anotaciones de la página.
    pub id: usize,
    /// Recuadro en el espacio de página (origen arriba a la izquierda, con el giro aplicado).
    pub rect: RectPts,
    /// Destino si se sabe leer (URI o página); `None` para destinos con nombre u otras acciones.
    pub target: Option<LinkTarget>,
}

fn rect_of_obj(o: &pliego_pdf::Object) -> Option<[f32; 4]> {
    let mut v = [0f32; 4];
    for (i, slot) in v.iter_mut().enumerate() {
        *slot = o.get_array(i as i32).ok()??.as_float().ok()?;
    }
    Some(v)
}

fn is_link(a: &pliego_pdf::Object) -> bool {
    a.dict_get_name("Subtype").ok().flatten().as_deref() == Some("Link")
}

fn target_of(pdf: &pliego_pdf::PdfDocument, a: &pliego_pdf::Object) -> Option<LinkTarget> {
    let dest_page = |d: &pliego_pdf::Object| -> Option<LinkTarget> {
        let first = d.get_array(0).ok()??;
        let n = pdf.lookup_page_number(&first).ok()?;
        Some(LinkTarget::Page { page: n as usize })
    };
    if let Some(act) = a.get_dict("A").ok().flatten() {
        match act.dict_get_name("S").ok().flatten().as_deref() {
            Some("URI") => {
                let u = act.get_dict("URI").ok().flatten()?.as_string().ok()?;
                return Some(LinkTarget::Uri { uri: u });
            }
            Some("GoTo") => return dest_page(&act.get_dict("D").ok().flatten()?),
            _ => return None,
        }
    }
    let d = a.get_dict("Dest").ok().flatten()?;
    if d.is_array().ok()? { dest_page(&d) } else { None }
}

/// Enlaces de la página (desde 0), en el orden de `/Annots`.
pub fn links(doc: &Document, page: usize) -> Result<Vec<LinkItem>> {
    let pg = load_page(doc.pdf(), page)?;
    let ctm = pg.ctm()?;
    let Some(annots) = pg.object().get_dict("Annots")? else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for (id, item) in annots.array_iter()?.into_iter().enumerate() {
        let Some(a) = item.resolve()? else { continue };
        if !is_link(&a) {
            continue;
        }
        let Some(r) = a.get_dict("Rect")?.and_then(|r| rect_of_obj(&r)) else {
            continue;
        };
        let q = Rect::new(r[0].min(r[2]), r[1].min(r[3]), r[0].max(r[2]), r[1].max(r[3])).transform(&ctm);
        out.push(LinkItem {
            page,
            id,
            rect: [q.x0, q.y0, q.x1, q.y1],
            target: target_of(doc.pdf(), &a),
        });
    }
    Ok(out)
}

fn link_at(pg: &pliego_pdf::PdfPage, id: usize) -> Result<(pliego_pdf::Object, pliego_pdf::Object)> {
    let annots = pg
        .object()
        .get_dict("Annots")?
        .ok_or_else(|| Error::InvalidInput("la página no tiene enlaces".into()))?;
    let a = annots
        .get_array(id as i32)?
        .and_then(|o| o.resolve().ok().flatten())
        .filter(is_link)
        .ok_or_else(|| Error::InvalidInput(format!("la anotación {id} no es un enlace")))?;
    Ok((annots, a))
}

/// Borra el enlace `id` (posición en `/Annots`) de la página.
pub fn delete_link(doc: &mut Document, page: usize, id: usize) -> Result<Report> {
    doc.with_pdf_mut(|pdf| {
        let pg = load_page(pdf, page)?;
        let (mut annots, _) = link_at(&pg, id)?;
        annots.array_delete(id as i32)
    })?;
    Ok(Report {
        summary: format!("Enlace borrado de la página {}", page + 1),
        ..Report::default()
    })
}

/// Cambia el destino del enlace `id` conservando su recuadro.
pub fn update_link(doc: &mut Document, page: usize, id: usize, target: &LinkTarget) -> Result<Report> {
    let (rect, _) = {
        let l = links(doc, page)?
            .into_iter()
            .find(|l| l.id == id)
            .ok_or_else(|| Error::InvalidInput(format!("la anotación {id} no es un enlace")))?;
        (l.rect, ())
    };
    // Se valida el destino creando primero el enlace nuevo; si falla no se borra el viejo.
    add_link(doc, page, rect, target)?;
    let nuevo = links(doc, page)?.last().map(|l| l.id);
    if let Err(e) = delete_link(doc, page, id) {
        if let Some(n) = nuevo {
            let _ = delete_link(doc, page, n);
        }
        return Err(e);
    }
    Ok(Report {
        summary: format!("Enlace actualizado en la página {}", page + 1),
        ..Report::default()
    })
}
