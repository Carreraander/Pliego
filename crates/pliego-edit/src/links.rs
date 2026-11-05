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
