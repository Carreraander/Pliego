//! Enlaces internos y externos.

use mupdf::pdf::links::LinkAction;
use mupdf::pdf::{PdfAction, PdfDestination, PdfLink};
use mupdf::{DestinationKind, Rect};
use pliego_core::{Error, Report, Result};
use pliego_document::Document;

use crate::types::{LinkTarget, RectPts};
use crate::{load_page, pe};

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
            PdfAction::Uri(uri.clone())
        }
        LinkTarget::Page { page: dest } => {
            let count = doc.pdf().page_count().map_err(pe)?.max(0) as usize;
            if *dest >= count {
                return Err(Error::InvalidInput(format!(
                    "la página de destino {} no existe",
                    dest + 1
                )));
            }
            PdfAction::GoTo(PdfDestination::Page {
                page: *dest as u32,
                kind: DestinationKind::Fit,
            })
        }
    };
    doc.with_pdf_mut(|pdf| {
        let mut p = load_page(pdf, page)?;
        let link = PdfLink {
            bounds: Rect::new(rect[0], rect[1], rect[2], rect[3]),
            action: LinkAction::Action(action),
        };
        p.add_links(pdf, &[link]).map_err(pe)
    })?;
    Ok(Report {
        summary: format!("Enlace añadido en la página {}", page + 1),
        ..Report::default()
    })
}
