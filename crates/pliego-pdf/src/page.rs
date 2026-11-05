//! Páginas: [`Page`] (cualquier formato) y [`PdfPage`] (página de un PDF, con acceso a su diccionario,
//! contenido, recursos, anotaciones y campos).

use std::ops::{Deref, DerefMut};

use pliego_core::{Error, Result};

use crate::PdfDocument;
use crate::backend::{err, irect_to, matrix_from, matrix_to, point_to, rect_from, rect_to};
use crate::geom::{IRect, Matrix, Quad, Rect};
use crate::links::{DestinationKind, Link, LinkDestination, LinkTarget, NewLink};
use crate::object::Object;
use crate::pixmap::{Colorspace, Pixmap};
use crate::text::{TextFlags, TextPage, TextWord};

pub(crate) enum PageRaw {
    Plain(mupdf::Page),
    Pdf(mupdf::pdf::PdfPage),
}

/// Página de cualquier documento. No es `Send`.
pub struct Page {
    pub(crate) raw: PageRaw,
}

impl Page {
    pub(crate) fn from_plain(p: mupdf::Page) -> Self {
        Self { raw: PageRaw::Plain(p) }
    }

    pub(crate) fn base(&self) -> &mupdf::Page {
        match &self.raw {
            PageRaw::Plain(p) => p,
            PageRaw::Pdf(p) => p,
        }
    }

    /// Caja de la página en puntos, con la rotación aplicada y origen arriba a la izquierda.
    pub fn bounds(&self) -> Result<Rect> {
        self.base().bounds().map(rect_from).map_err(err)
    }

    /// Dibuja la página entera. `ctm` lleva el espacio de página a píxeles (p. ej. escala = ppp/72).
    /// `alpha`: fondo transparente en lugar de blanco. `show_extras`: incluye anotaciones y campos.
    pub fn to_pixmap(&self, ctm: &Matrix, cs: Colorspace, alpha: bool, show_extras: bool) -> Result<Pixmap> {
        let pix = self
            .base()
            .to_pixmap(&matrix_to(ctm), &cs.to_backend(), alpha, show_extras)
            .map_err(err)?;
        Ok(Pixmap::from_backend(&pix))
    }

    /// Dibuja solo la región `region` (en píxeles, ya con `ctm` aplicado) sobre fondo blanco.
    pub fn render_region(&self, ctm: &Matrix, region: IRect, cs: Colorspace, show_extras: bool) -> Result<Pixmap> {
        if region.is_empty() {
            return Err(Error::InvalidInput("la región a dibujar está vacía".into()));
        }
        let mut pix = mupdf::Pixmap::new_with_rect(&cs.to_backend(), irect_to(region), false).map_err(err)?;
        pix.clear_with(255).map_err(err)?;
        {
            let dev = mupdf::Device::from_pixmap(&pix).map_err(err)?;
            let m = matrix_to(ctm);
            if show_extras {
                self.base().run(&dev, &m).map_err(err)?;
            } else {
                self.base().run_contents(&dev, &m).map_err(err)?;
            }
        }
        Ok(Pixmap::from_backend(&pix))
    }

    /// Página como SVG. `ctm` es normalmente `Matrix::IDENTITY`.
    pub fn to_svg(&self, ctm: &Matrix) -> Result<String> {
        self.base().to_svg(&matrix_to(ctm)).map_err(err)
    }

    pub fn to_text_page(&self, flags: TextFlags) -> Result<TextPage> {
        self.base()
            .to_text_page(flags.to_backend())
            .map(|raw| TextPage { raw })
            .map_err(err)
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
        Ok(self
            .base()
            .search(needle, hit_max)
            .map_err(err)?
            .iter()
            .map(|q| {
                crate::geom::Quad::new(
                    crate::backend::point_from(q.ul),
                    crate::backend::point_from(q.ur),
                    crate::backend::point_from(q.ll),
                    crate::backend::point_from(q.lr),
                )
            })
            .collect())
    }

    /// Enlaces de la página. El iterador es un `vec::IntoIter` para poder usar `.collect()` directamente.
    pub fn links(&self) -> Result<std::vec::IntoIter<Link>> {
        let mut out = Vec::new();
        for l in self.base().links().map_err(err)? {
            out.push(Link {
                bounds: rect_from(l.bounds),
                dest: l.dest.map(|d| LinkDestination {
                    page: d.loc.page_number,
                    kind: kind_from(d.kind),
                }),
                uri: l.uri,
            });
        }
        Ok(out.into_iter())
    }
}

pub(crate) fn kind_from(k: mupdf::DestinationKind) -> DestinationKind {
    use mupdf::DestinationKind as K;
    match k {
        K::Fit => DestinationKind::Fit,
        K::FitH { top } => DestinationKind::FitH { top },
        K::FitV { left } => DestinationKind::FitV { left },
        K::XYZ { left, top, zoom } => DestinationKind::XYZ { left, top, zoom },
        K::FitR {
            left,
            bottom,
            right,
            top,
        } => DestinationKind::FitR {
            left,
            bottom,
            right,
            top,
        },
        K::FitB => DestinationKind::FitB,
        K::FitBH { top } => DestinationKind::FitBH { top },
        K::FitBV { left } => DestinationKind::FitBV { left },
    }
}

pub(crate) fn kind_to(k: DestinationKind) -> mupdf::DestinationKind {
    use mupdf::DestinationKind as K;
    match k {
        DestinationKind::Fit => K::Fit,
        DestinationKind::FitH { top } => K::FitH { top },
        DestinationKind::FitV { left } => K::FitV { left },
        DestinationKind::XYZ { left, top, zoom } => K::XYZ { left, top, zoom },
        DestinationKind::FitR {
            left,
            bottom,
            right,
            top,
        } => K::FitR {
            left,
            bottom,
            right,
            top,
        },
        DestinationKind::FitB => K::FitB,
        DestinationKind::FitBH { top } => K::FitBH { top },
        DestinationKind::FitBV { left } => K::FitBV { left },
    }
}

/// Página de un PDF. Deriva a [`Page`].
pub struct PdfPage {
    page: Page,
}

impl Deref for PdfPage {
    type Target = Page;
    fn deref(&self) -> &Page {
        &self.page
    }
}

impl DerefMut for PdfPage {
    fn deref_mut(&mut self) -> &mut Page {
        &mut self.page
    }
}

impl PdfPage {
    pub(crate) fn from_raw(raw: mupdf::pdf::PdfPage) -> Self {
        Self {
            page: Page { raw: PageRaw::Pdf(raw) },
        }
    }

    pub(crate) fn pdf(&self) -> &mupdf::pdf::PdfPage {
        match &self.page.raw {
            PageRaw::Pdf(p) => p,
            PageRaw::Plain(_) => unreachable!("PdfPage siempre contiene una página PDF"),
        }
    }

    pub(crate) fn pdf_mut(&mut self) -> &mut mupdf::pdf::PdfPage {
        match &mut self.page.raw {
            PageRaw::Pdf(p) => p,
            PageRaw::Plain(_) => unreachable!("PdfPage siempre contiene una página PDF"),
        }
    }

    /// Diccionario `/Page`.
    pub fn object(&self) -> Object {
        Object::wrap(self.pdf().object())
    }

    /// Valor de `/Contents` tal cual (un flujo, una matriz de flujos o `None`).
    pub fn contents(&self) -> Result<Option<Object>> {
        self.pdf().contents().map(|o| o.map(Object::wrap)).map_err(err)
    }

    /// `/Resources` propio de la página; si solo los hereda, los copia a la página antes de devolverlos.
    pub fn resources(&self) -> Result<Object> {
        self.pdf().resources().map(Object::wrap).map_err(err)
    }

    /// Giro de la página en grados (0, 90, 180 o 270), heredado si hace falta.
    pub fn rotation(&self) -> Result<i32> {
        self.pdf().rotation().map_err(err)
    }

    pub fn set_rotation(&mut self, rotate: i32) -> Result<()> {
        self.pdf_mut().set_rotation(rotate).map_err(err)
    }

    /// `/MediaBox` en el espacio de usuario, sin girar.
    pub fn media_box(&self) -> Result<Rect> {
        self.pdf().media_box().map(rect_from).map_err(err)
    }

    pub fn crop_box(&self) -> Result<Rect> {
        self.pdf().crop_box().map(rect_from).map_err(err)
    }

    pub fn set_crop_box(&mut self, crop_box: Rect) -> Result<()> {
        self.pdf_mut().set_crop_box(rect_to(crop_box)).map_err(err)
    }

    /// Matriz del espacio de usuario de la página al espacio de página (con giro y recorte aplicados).
    pub fn ctm(&self) -> Result<Matrix> {
        self.pdf().ctm().map(matrix_from).map_err(err)
    }

    /// Añade un flujo de contenido a `/Contents`: al final (`overlay`) o al principio. Devuelve su número de objeto.
    pub fn insert_contents(&mut self, doc: &mut PdfDocument, bytes: &[u8], overlay: bool) -> Result<i32> {
        self.pdf_mut()
            .insert_contents(doc.pdf_mut(), bytes, overlay)
            .map_err(err)
    }

    /// Envuelve el contenido existente en `q … Q` para que lo que se añada después parta de un estado gráfico limpio.
    pub fn wrap_contents(&mut self, doc: &mut PdfDocument) -> Result<()> {
        self.pdf_mut().wrap_contents(doc.pdf_mut()).map_err(err)
    }

    /// Registra (o reutiliza) un `ExtGState` con opacidades y devuelve su nombre de recurso con barra
    /// (`"/GS0"`); `None` si ambas son `None`.
    pub fn register_ext_gstate(
        &mut self,
        doc: &mut PdfDocument,
        stroke_opacity: Option<f32>,
        fill_opacity: Option<f32>,
    ) -> Result<Option<String>> {
        self.pdf_mut()
            .register_ext_gstate(doc.pdf_mut(), stroke_opacity, fill_opacity)
            .map_err(err)
    }

    /// Regenera las apariencias de las anotaciones modificadas. `true` si cambió algo.
    pub fn update(&mut self) -> Result<bool> {
        self.pdf_mut().update().map_err(err)
    }

    /// Crea enlaces en la página (`bounds` en el espacio de página, como los que devuelve [`Page::links`]).
    pub fn add_links(&mut self, doc: &mut PdfDocument, links: &[NewLink]) -> Result<()> {
        use mupdf::pdf::{LinkAction, PdfAction, PdfDestination, PdfLink};
        let conv: Vec<PdfLink> = links
            .iter()
            .map(|l| PdfLink {
                bounds: rect_to(l.bounds),
                action: LinkAction::Action(match &l.target {
                    LinkTarget::Uri(u) => PdfAction::Uri(u.clone()),
                    LinkTarget::Page { page, kind } => PdfAction::GoTo(PdfDestination::Page {
                        page: *page,
                        kind: kind_to(*kind),
                    }),
                }),
            })
            .collect();
        self.pdf_mut().add_links(doc.pdf_mut(), &conv).map_err(err)
    }
}

/// Punto del espacio de página (para uso interno de otros módulos).
#[allow(dead_code)]
pub(crate) fn pt(p: crate::geom::Point) -> mupdf::Point {
    point_to(p)
}
