//! Motor de render de Pliego sobre `pliego-pdf` (AGPL-3.0-or-later).
//!
//! `DefaultDocument` no es `Send`: abrir y usar cada documento en un único hilo.

use std::path::Path;

use pliego_pdf::{Colorspace, Document, IRect, Matrix, MetadataName, Outline, PdfDocument, Pixmap};
pub use pliego_pdf::{RenderBackend, configure_from_environment, init_fonts};

use pliego_core::{
    DocInfo, EngineDocument, Error, Layer, OutlineItem, PageSize, PdfEngine, Result, RgbaImage, SearchHit, TileRequest,
};

/// Límite de seguridad para no reservar mapas de bits enormes por error (≈ 400 MB RGBA).
const MAX_PIXELS: u64 = 100_000_000;

#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultEngine;

impl DefaultEngine {
    /// Abre un documento con un motor de render y texto concreto, sin tocar el valor por defecto del proceso
    /// (lo usa el arnés de comparación para tener MuPDF y PDFium en el mismo proceso).
    pub fn open_with_backend(
        &self,
        path: &Path,
        password: Option<&str>,
        backend: RenderBackend,
    ) -> Result<DefaultDocument> {
        let mut doc = Document::open_with_password(path, password)?;
        doc.set_render_backend(backend)?;
        DefaultDocument::from_document(doc)
    }
}

/// Documento abierto: los PDF se guardan como `PdfDocument` para poder recorrer sus objetos.
enum Doc {
    Pdf(PdfDocument),
    Other(Document),
}

impl std::ops::Deref for Doc {
    type Target = Document;
    fn deref(&self) -> &Document {
        match self {
            Doc::Pdf(p) => p,
            Doc::Other(d) => d,
        }
    }
}

pub struct DefaultDocument {
    doc: Doc,
}

impl PdfEngine for DefaultEngine {
    type Doc = DefaultDocument;

    fn open(&self, path: &Path, password: Option<&str>) -> Result<DefaultDocument> {
        // Un PDF de tamaño normal se lee a memoria y no se deja abierto: en Windows un archivo abierto por el worker
        // impide a la app guardar encima («Access is denied», error 5) al comentar, rellenar o editar.
        if let Some(bytes) = read_unlocked(path)? {
            return self.open_bytes(&bytes, password);
        }
        DefaultDocument::from_document(Document::open_with_password(path, password)?)
    }

    fn open_bytes(&self, bytes: &[u8], password: Option<&str>) -> Result<DefaultDocument> {
        DefaultDocument::from_document(Document::from_bytes_with_password(bytes, "application/pdf", password)?)
    }
}

/// Tamaño máximo de un PDF que se abre desde memoria en vez de por ruta (por encima, el archivo queda abierto).
const MAX_OPEN_FROM_MEMORY: u64 = 256 * 1024 * 1024;

/// Contenido del archivo si es un PDF que cabe en memoria; `None` si hay que abrirlo por ruta (otros formatos o
/// archivos enormes). Un archivo que no existe da el error de E/S habitual.
fn read_unlocked(path: &Path) -> Result<Option<Vec<u8>>> {
    let is_pdf = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"));
    if !is_pdf {
        return Ok(None);
    }
    let len = std::fs::metadata(path).map_err(|e| Error::io_at(path, e))?.len();
    if len > MAX_OPEN_FROM_MEMORY {
        return Ok(None);
    }
    std::fs::read(path).map(Some).map_err(|e| Error::io_at(path, e))
}

impl DefaultDocument {
    fn from_document(doc: Document) -> Result<Self> {
        let doc = if doc.is_pdf() {
            Doc::Pdf(doc.into_pdf()?)
        } else {
            Doc::Other(doc)
        };
        Ok(Self { doc })
    }

    /// Muestra u oculta una capa en memoria (el archivo no cambia). Solo PDF.
    pub fn set_layer_visible_mut(&mut self, id: i32, visible: bool) -> Result<()> {
        match &mut self.doc {
            Doc::Pdf(pdf) => {
                let known = pdf.optional_content_groups()?.iter().any(|g| g.xref == id);
                if !known {
                    return Err(Error::InvalidInput(format!("la capa {id} no existe")));
                }
                pdf.set_optional_content_enabled(id, visible)?;
                // El motor lee la configuración de capas al abrir: para que el render la respete se reabre el
                // documento desde su copia en memoria (sin cifrar, así que no vuelve a pedir contraseña).
                let mut bytes = Vec::new();
                pdf.write_to(&mut bytes)?;
                *self = Self::from_document(Document::from_bytes_with_password(&bytes, "application/pdf", None)?)?;
                Ok(())
            }
            Doc::Other(_) => Err(Error::Unsupported("solo los PDF tienen capas".into())),
        }
    }

    /// Acceso al documento de `pliego-pdf` para módulos que necesiten más de lo que expone el trait.
    pub fn document(&self) -> &Document {
        &self.doc
    }

    fn check_page(&self, page: u32) -> Result<pliego_pdf::Page> {
        let count = self.page_count()?;
        if page >= count {
            return Err(Error::InvalidInput(format!(
                "página {} fuera de rango (hay {count})",
                page + 1
            )));
        }
        self.doc.load_page(page as i32)
    }

    fn meta(&self, name: MetadataName) -> Option<String> {
        self.doc.metadata(name).ok().filter(|s| !s.is_empty())
    }
}

fn convert_outline(items: Vec<Outline>) -> Vec<OutlineItem> {
    items
        .into_iter()
        .map(|o| OutlineItem {
            title: o.title,
            page: o.dest.map(|d| d.page),
            uri: o.uri.filter(|u| !u.starts_with('#')),
            children: convert_outline(o.down),
        })
        .collect()
}

fn to_rgba(pix: &Pixmap) -> RgbaImage {
    RgbaImage {
        width: pix.width(),
        height: pix.height(),
        data: pix.to_rgba(),
    }
}

fn check_size(w: u64, h: u64) -> Result<()> {
    if w == 0 || h == 0 || w * h > MAX_PIXELS {
        return Err(Error::InvalidInput(format!("tamaño de render no válido: {w}x{h}")));
    }
    Ok(())
}

impl EngineDocument for DefaultDocument {
    fn info(&self) -> Result<DocInfo> {
        Ok(DocInfo {
            page_count: self.page_count()?,
            title: self.meta(MetadataName::Title),
            author: self.meta(MetadataName::Author),
            subject: self.meta(MetadataName::Subject),
            keywords: self.meta(MetadataName::Keywords),
            creator: self.meta(MetadataName::Creator),
            producer: self.meta(MetadataName::Producer),
            format: self.meta(MetadataName::Format),
            encryption: self.meta(MetadataName::Encryption).filter(|e| e != "None"),
            is_pdf: self.doc.is_pdf(),
        })
    }

    fn page_count(&self) -> Result<u32> {
        Ok(self.doc.page_count()?.max(0) as u32)
    }

    fn page_size(&self, page: u32) -> Result<PageSize> {
        let b = self.check_page(page)?.bounds()?;
        Ok(PageSize {
            width: b.x1 - b.x0,
            height: b.y1 - b.y0,
        })
    }

    fn render_page(&self, page: u32, zoom: f32) -> Result<RgbaImage> {
        let p = self.check_page(page)?;
        let b = p.bounds()?;
        check_size(
            ((b.x1 - b.x0) * zoom).ceil() as u64,
            ((b.y1 - b.y0) * zoom).ceil() as u64,
        )?;
        let pix = p.to_pixmap(&Matrix::new_scale(zoom, zoom), Colorspace::Rgb, false, true)?;
        Ok(to_rgba(&pix))
    }

    fn render_tile(&self, req: TileRequest) -> Result<RgbaImage> {
        if req.size == 0 || req.zoom.is_nan() || req.zoom <= 0.0 {
            return Err(Error::InvalidInput("tesela con tamaño o zoom no válidos".into()));
        }
        let p = self.check_page(req.page)?;
        let b = p.bounds()?;
        let full_w = ((b.x1 - b.x0) * req.zoom).ceil() as i64;
        let full_h = ((b.y1 - b.y0) * req.zoom).ceil() as i64;
        let x0 = req.col as i64 * req.size as i64;
        let y0 = req.row as i64 * req.size as i64;
        if x0 >= full_w || y0 >= full_h {
            return Err(Error::InvalidInput("tesela fuera de la página".into()));
        }
        let x1 = (x0 + req.size as i64).min(full_w);
        let y1 = (y0 + req.size as i64).min(full_h);
        check_size((x1 - x0) as u64, (y1 - y0) as u64)?;
        let rect = IRect {
            x0: x0 as i32,
            y0: y0 as i32,
            x1: x1 as i32,
            y1: y1 as i32,
        };
        // Origen de la página en (0,0) y escala al zoom pedido.
        let mut ctm = Matrix::new_translate(-b.x0, -b.y0);
        ctm.concat(Matrix::new_scale(req.zoom, req.zoom));
        let pix = p.render_region(&ctm, rect, Colorspace::Rgb, true)?;
        Ok(to_rgba(&pix))
    }

    fn page_text(&self, page: u32) -> Result<String> {
        self.check_page(page)?.text()
    }

    fn search_page(&self, page: u32, needle: &str, max_hits: u32) -> Result<SearchHit> {
        let p = self.check_page(page)?;
        let quads = p.search(needle, max_hits.max(1))?;
        Ok(SearchHit {
            page,
            quads: quads
                .iter()
                .map(|q| [q.ul.x, q.ul.y, q.ur.x, q.ur.y, q.ll.x, q.ll.y, q.lr.x, q.lr.y])
                .collect(),
        })
    }

    fn layers(&self) -> Result<Vec<Layer>> {
        match &self.doc {
            Doc::Pdf(pdf) => Ok(pdf
                .optional_content_groups()?
                .into_iter()
                .enumerate()
                .map(|(i, g)| Layer {
                    id: g.xref,
                    name: g
                        .name
                        .filter(|n| !n.trim().is_empty())
                        .unwrap_or_else(|| format!("Capa {}", i + 1)),
                    visible: g.enabled,
                })
                .collect()),
            Doc::Other(_) => Ok(Vec::new()),
        }
    }

    fn outline(&self) -> Result<Vec<OutlineItem>> {
        match &self.doc {
            // `PdfDocument::outlines` recorre el árbol con cuidado (marcadores sin /Title, ciclos…).
            Doc::Pdf(pdf) => Ok(convert_outline(pdf.outlines()?)),
            Doc::Other(d) => Ok(convert_outline(d.outlines()?)),
        }
    }
}

/// Codifica una imagen RGBA a PNG.
pub fn encode_png(img: &RgbaImage) -> Result<Vec<u8>> {
    Pixmap::from_samples(Colorspace::Rgb, img.width, img.height, true, img.data.clone())?.encode_png()
}

#[cfg(test)]
mod tests;
