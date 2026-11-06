//! Motor de render de Pliego sobre `pliego-pdf` (AGPL-3.0-or-later).
//!
//! `DefaultDocument` no es `Send`: abrir y usar cada documento en un único hilo.

use std::path::{Path, PathBuf};

pub use pliego_pdf::configure_from_environment;
use pliego_pdf::{Colorspace, IRect, Matrix, MetadataName, Outline, PdfDocument, Pixmap};

use pliego_core::{
    DocInfo, EngineDocument, Error, Layer, OutlineItem, PageSize, PdfEngine, Result, RgbaImage, SearchHit, TileRequest,
};

/// Límite de seguridad para no reservar mapas de bits enormes por error (≈ 400 MB RGBA).
const MAX_PIXELS: u64 = 100_000_000;

#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultEngine;

/// Archivo de PDF desde el que el worker abre «solo para ver» (véase [`DefaultEngine::open_for_viewing`]).
pub const VIEW_ONLY_MIN_BYTES: u64 = 8 * 1024 * 1024;

/// Mayor archivo que se abre solo para ver: al abrirlo hay que leerlo entero una vez para construir el modelo (el búfer se
/// suelta después), y esa lectura cabe en el límite de memoria del worker (2 GB, `WorkerConfig::memory_limit`).
pub const VIEW_ONLY_MAX_BYTES: u64 = 1700 * 1024 * 1024;

/// Mayor archivo cuyas capas se pueden mostrar u ocultar: hay que abrirlo entero (con sus datos en memoria).
pub const LAYER_EDIT_MAX_BYTES: u64 = 256 * 1024 * 1024;

/// Documento abierto: un PDF con su modelo de objetos (se pueden recorrer sus objetos).
pub struct DefaultDocument {
    doc: PdfDocument,
    /// `Some` si se abrió solo para ver: de dónde (para pasar a documento completo si se cambian capas).
    view_only: Option<ViewOnly>,
}

struct ViewOnly {
    path: PathBuf,
    password: Option<String>,
}

impl PdfEngine for DefaultEngine {
    type Doc = DefaultDocument;

    fn open(&self, path: &Path, password: Option<&str>) -> Result<DefaultDocument> {
        Ok(DefaultDocument {
            doc: PdfDocument::open_with_password(path, password)?,
            view_only: None,
        })
    }

    fn open_bytes(&self, bytes: &[u8], password: Option<&str>) -> Result<DefaultDocument> {
        Ok(DefaultDocument {
            doc: PdfDocument::from_bytes_with_password(bytes, password)?,
            view_only: None,
        })
    }
}

impl DefaultEngine {
    /// Abre `path` solo para verlo (leer, dibujar, buscar): el proceso no guarda el archivo ni sus imágenes en memoria,
    /// así que un PDF de 1 GB cabe en el límite del worker. No admite cambios salvo las capas (que lo abren entero).
    pub fn open_for_viewing(&self, path: &Path, password: Option<&str>) -> Result<DefaultDocument> {
        let size = std::fs::metadata(path).map_err(|e| Error::io_at(path, e))?.len();
        check_view_only_size(path, size)?;
        Ok(DefaultDocument {
            doc: PdfDocument::open_for_viewing(path, password)?,
            view_only: Some(ViewOnly {
                path: path.to_path_buf(),
                password: password.map(str::to_owned),
            }),
        })
    }

    /// Como [`PdfEngine::open`], pero los archivos de [`VIEW_ONLY_MIN_BYTES`] o más se abren solo para ver.
    pub fn open_auto(&self, path: &Path, password: Option<&str>) -> Result<DefaultDocument> {
        let big = std::fs::metadata(path).is_ok_and(|m| m.len() >= view_only_min_bytes());
        if big {
            self.open_for_viewing(path, password)
        } else {
            self.open(path, password)
        }
    }
}

fn check_view_only_size(path: &Path, size: u64) -> Result<()> {
    if size > VIEW_ONLY_MAX_BYTES {
        return Err(Error::Unsupported(format!(
            "{} pesa {} MB: Pliego abre PDF de hasta {} MB",
            path.display(),
            size / 1_048_576,
            VIEW_ONLY_MAX_BYTES / 1_048_576
        )));
    }
    Ok(())
}

/// Umbral efectivo: `PLIEGO_VIEW_ONLY_MB` (para pruebas y diagnóstico) o [`VIEW_ONLY_MIN_BYTES`].
fn view_only_min_bytes() -> u64 {
    std::env::var("PLIEGO_VIEW_ONLY_MB")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map_or(VIEW_ONLY_MIN_BYTES, |mb| mb * 1024 * 1024)
}

impl DefaultDocument {
    /// Muestra u oculta una capa en memoria (el archivo no cambia).
    pub fn set_layer_visible_mut(&mut self, id: i32, visible: bool) -> Result<()> {
        if let Some(v) = &self.view_only {
            // Cambiar una capa modifica el modelo y PDFium debe leer una copia nueva: hace falta el documento entero.
            let size = std::fs::metadata(&v.path).map_err(|e| Error::io_at(&v.path, e))?.len();
            if size > LAYER_EDIT_MAX_BYTES {
                return Err(Error::Unsupported(format!(
                    "este PDF pesa {} MB: mostrar u ocultar capas necesita abrirlo entero y solo se admite hasta {} MB",
                    size / 1_048_576,
                    LAYER_EDIT_MAX_BYTES / 1_048_576
                )));
            }
            self.doc = PdfDocument::open_with_password(&v.path, v.password.as_deref())?;
            self.view_only = None;
        }
        let known = self.doc.optional_content_groups()?.iter().any(|g| g.xref == id);
        if !known {
            return Err(Error::InvalidInput(format!("la capa {id} no existe")));
        }
        // El dibujo lee una copia al día del documento, así que el cambio se ve en el siguiente render.
        self.doc.set_optional_content_enabled(id, visible)
    }

    /// Acceso al documento de `pliego-pdf` para módulos que necesiten más de lo que expone el trait.
    pub fn document(&self) -> &PdfDocument {
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
            is_pdf: true,
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
        Ok(self
            .doc
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
            .collect())
    }

    fn outline(&self) -> Result<Vec<OutlineItem>> {
        // `PdfDocument::outlines` recorre el árbol con cuidado (marcadores sin /Title, ciclos…).
        Ok(convert_outline(self.doc.outlines()?))
    }
}

/// Codifica una imagen RGBA a PNG.
pub fn encode_png(img: &RgbaImage) -> Result<Vec<u8>> {
    Pixmap::from_samples(Colorspace::Rgb, img.width, img.height, true, img.data.clone())?.encode_png()
}

#[cfg(test)]
mod tests;
