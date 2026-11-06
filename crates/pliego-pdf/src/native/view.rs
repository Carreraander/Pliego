//! Lo que se **ve** de una página: dibujo, texto, búsqueda y enlaces.
//!
//! El modelo propio (lopdf) no dibuja: PDFium lee una copia en memoria del documento (la original si no ha cambiado,
//! o una serialización sin cifrar si hay cambios; véase [`crate::pdfium`]) y entrega mapas de bits y texto. Los
//! enlaces se leen del propio modelo.

use lopdf::Object as L;
use pliego_core::{Error, Result};

use super::document::{PdfDocument, decode_text};
use super::page::{PdfPage, rect_of};
use super::{outline, pdf_err, save, tree};
use crate::geom::{IRect, Matrix, Quad, Rect};
use crate::links::{Link, LinkDestination};
use crate::pdfium::ViewSource;
use crate::pixmap::{Colorspace, Pixmap};
use crate::text::{TextBlock, TextBlockKind, TextFlags, TextPage, TextWord};

impl PdfDocument {
    /// Página `page_no` (desde 0). Igual que [`PdfDocument::load_pdf_page`].
    pub fn load_page(&self, page_no: i32) -> Result<PdfPage> {
        self.load_pdf_page(page_no)
    }
}

impl PdfPage {
    /// Número de esta página en su documento (desde 0).
    fn index(&self) -> Result<i32> {
        let id = self
            .page
            .obj_id()
            .ok_or_else(|| pdf_err("la página no pertenece a un documento"))?;
        let st = self.page.state()?;
        let mut s = st.borrow_mut();
        let id = s.resolve_id(id);
        tree::page_list(&mut s)
            .iter()
            .position(|p| *p == id)
            .map(|i| i as i32)
            .ok_or_else(|| pdf_err("la página ya no está en el documento"))
    }

    /// Esta página en PDFium, calibrada con el tamaño de `bounds()` (por `/UserUnit`).
    fn pdfium(&self) -> Result<crate::pdfium::PageLink> {
        let index = self.index()?;
        let st = self.page.state()?;
        let s = st.borrow();
        if s.locked {
            return Err(Error::Password);
        }
        let mut link = s.view.page(index, s.generation, || {
            // Sin cambios desde que se abrió: el archivo original (PDFium descifra por su cuenta con la contraseña
            // ya validada). Con cambios, o en un documento nuevo: una copia sin cifrar de lo que hay en memoria.
            // Un documento abierto solo para ver se lee siempre del archivo y no admite cambios.
            if let Some(path) = &s.view_only {
                return if s.generation == 0 {
                    Ok(ViewSource::File(path.clone(), s.password.clone()))
                } else {
                    Err(view_only_error())
                };
            }
            match (&s.base, s.generation) {
                (Some(b), 0) => Ok(ViewSource::Bytes(b.bytes.clone(), s.password.clone())),
                _ => save::snapshot_clear(&s).map(|bytes| ViewSource::Bytes(bytes, None)),
            }
        })?;
        drop(s);
        if let Ok(b) = self.bounds() {
            link.calibrate(b.x1 - b.x0, b.y1 - b.y0);
        }
        Ok(link)
    }

    /// Dibuja la página entera. `ctm` lleva el espacio de página a píxeles (p. ej. escala = ppp/72).
    /// `alpha`: fondo transparente en lugar de blanco. `show_extras`: incluye anotaciones y campos.
    pub fn to_pixmap(&self, ctm: &Matrix, cs: Colorspace, alpha: bool, show_extras: bool) -> Result<Pixmap> {
        let link = self.pdfium()?;
        // Caja de la página transformada, redondeada hacia fuera con una tolerancia de 0,001.
        let r = link.bounds()?.transform(ctm);
        let region = IRect::new(
            (r.x0 + 0.001).floor() as i32,
            (r.y0 + 0.001).floor() as i32,
            (r.x1 - 0.001).ceil() as i32,
            (r.y1 - 0.001).ceil() as i32,
        );
        link.render(ctm, region, cs, alpha, show_extras)
    }

    /// Dibuja solo la región `region` (en píxeles, ya con `ctm` aplicado) sobre fondo blanco.
    pub fn render_region(&self, ctm: &Matrix, region: IRect, cs: Colorspace, show_extras: bool) -> Result<Pixmap> {
        if region.is_empty() {
            return Err(Error::InvalidInput("la región a dibujar está vacía".into()));
        }
        self.pdfium()?.render(ctm, region, cs, false, show_extras)
    }

    /// Texto estructurado. Con `PRESERVE_IMAGES` se añaden los bloques de imagen (de `image_placements`).
    pub fn to_text_page(&self, flags: TextFlags) -> Result<TextPage> {
        let mut blocks = self.pdfium()?.text_page()?.blocks();
        if flags.contains(TextFlags::PRESERVE_IMAGES) {
            for (_, m) in self.image_placements()? {
                let r = Rect::new(0.0, 0.0, 1.0, 1.0).transform(&m);
                let at = blocks.iter().position(|b| b.bounds.y0 > r.y0).unwrap_or(blocks.len());
                blocks.insert(
                    at,
                    TextBlock {
                        kind: TextBlockKind::Image,
                        bounds: r,
                        lines: Vec::new(),
                        image_transform: Some(m),
                    },
                );
            }
        }
        Ok(TextPage::from_blocks(blocks))
    }

    /// Texto plano de la página.
    pub fn text(&self) -> Result<String> {
        self.to_text_page(TextFlags::empty())?.to_text()
    }

    pub fn words(&self) -> Result<Vec<TextWord>> {
        Ok(self.to_text_page(TextFlags::empty())?.words())
    }

    /// Cuadriláteros de hasta `hit_max` apariciones de `needle` (sin distinguir mayúsculas).
    pub fn search(&self, needle: &str, hit_max: u32) -> Result<Vec<Quad>> {
        self.pdfium()?.search(needle, hit_max)
    }

    /// Enlaces de la página (URI y saltos internos), con su caja en el espacio de página. El iterador es un
    /// `vec::IntoIter` para poder usar `.collect()` directamente.
    pub fn links(&self) -> Result<std::vec::IntoIter<Link>> {
        let ctm = self.ctm()?;
        let st = self.page.state()?.clone();
        let pages = tree::page_list(&mut st.borrow_mut());
        let annots = match self.page.get_dict("Annots")? {
            Some(a) if a.is_array()? => a.array_iter()?,
            _ => Vec::new(),
        };
        let mut out = Vec::new();
        for a in annots {
            if !a
                .get_dict("Subtype")?
                .is_some_and(|s| s.as_name().is_ok_and(|n| n == b"Link"))
            {
                continue;
            }
            let Some(rect) = a.get_dict("Rect")?.and_then(|r| rect_of(&r)) else {
                continue;
            };
            let clone_of = |o: Option<super::Object>| -> Result<Option<L>> {
                match o {
                    Some(o) => o.read(|v| Ok(Some(v.clone()))),
                    None => Ok(None),
                }
            };
            let mut dest: Option<LinkDestination> = None;
            let mut uri = String::new();
            let s = st.borrow();
            if let Some(d) = clone_of(a.get_dict("Dest")?)? {
                dest = outline::dest_of(&s, &pages, &d);
            } else if let Some(action) = a.get_dict("A")? {
                match action.get_dict("S")?.and_then(|n| n.as_name().ok()).as_deref() {
                    Some(b"GoTo") => {
                        if let Some(d) = clone_of(action.get_dict("D")?)? {
                            dest = outline::dest_of(&s, &pages, &d);
                        }
                    }
                    Some(b"URI") => {
                        if let Some(u) = clone_of(action.get_dict("URI")?)? {
                            uri = decode_text(&u).unwrap_or_default();
                        }
                    }
                    _ => {}
                }
            }
            out.push(Link {
                bounds: rect.transform(&ctm),
                dest,
                uri,
            });
        }
        Ok(out.into_iter())
    }
}

/// Error de las operaciones que un documento abierto con `PdfDocument::open_for_viewing` no admite.
pub(crate) fn view_only_error() -> Error {
    Error::Unsupported(
        "el documento está abierto solo para verlo (PDF grande): no se puede modificar ni guardar desde ahí".into(),
    )
}
