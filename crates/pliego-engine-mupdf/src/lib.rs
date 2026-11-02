//! Motor de render de Pliego sobre MuPDF (AGPL-3.0-or-later).
//!
//! `MupdfDocument` no es `Send`: abrir y usar cada documento en un único hilo.

use std::path::Path;

use mupdf::{Colorspace, Device, Document, IRect, Matrix, MetadataName, Pixmap, TextPageFlags};
use pliego_core::{
    DocInfo, EngineDocument, Error, OutlineItem, PageSize, PdfEngine, Result, RgbaImage, SearchHit, TileRequest,
};

/// Límite de seguridad para no reservar mapas de bits enormes por error (≈ 400 MB RGBA).
const MAX_PIXELS: u64 = 100_000_000;

#[derive(Debug, Default, Clone, Copy)]
pub struct MupdfEngine;

pub struct MupdfDocument {
    doc: Document,
}

fn pdf_err(e: mupdf::Error) -> Error {
    Error::Pdf(e.to_string())
}

impl PdfEngine for MupdfEngine {
    type Doc = MupdfDocument;

    fn open(&self, path: &Path, password: Option<&str>) -> Result<MupdfDocument> {
        if !path.exists() {
            return Err(Error::io_at(path, std::io::Error::from(std::io::ErrorKind::NotFound)));
        }
        let doc = Document::open(path).map_err(pdf_err)?;
        MupdfDocument::unlock(doc, password)
    }

    fn open_bytes(&self, bytes: &[u8], password: Option<&str>) -> Result<MupdfDocument> {
        let doc = Document::from_bytes(bytes, "application/pdf").map_err(pdf_err)?;
        MupdfDocument::unlock(doc, password)
    }
}

impl MupdfDocument {
    fn unlock(mut doc: Document, password: Option<&str>) -> Result<Self> {
        if doc.needs_password().map_err(pdf_err)? {
            let ok = match password {
                Some(p) => doc.authenticate(p).map_err(pdf_err)?,
                None => false,
            };
            if !ok {
                return Err(Error::Password);
            }
        }
        Ok(Self { doc })
    }

    /// Acceso al documento MuPDF para módulos que necesiten más de lo que expone el trait.
    pub fn raw(&self) -> &Document {
        &self.doc
    }

    fn check_page(&self, page: u32) -> Result<mupdf::Page> {
        let count = self.page_count()?;
        if page >= count {
            return Err(Error::InvalidInput(format!(
                "página {} fuera de rango (hay {count})",
                page + 1
            )));
        }
        self.doc.load_page(page as i32).map_err(pdf_err)
    }

    fn meta(&self, name: MetadataName) -> Option<String> {
        self.doc.metadata(name).ok().filter(|s| !s.is_empty())
    }

    fn convert_outline(&self, items: Vec<mupdf::Outline>) -> Vec<OutlineItem> {
        items
            .into_iter()
            .map(|o| OutlineItem {
                title: o.title,
                page: o.dest.map(|d| d.loc.page_number),
                uri: o.uri.filter(|u| !u.starts_with('#')),
                children: self.convert_outline(o.down),
            })
            .collect()
    }
}

fn to_rgba(pix: &Pixmap) -> RgbaImage {
    let (w, h) = (pix.width(), pix.height());
    let n = pix.n() as usize;
    let stride = pix.stride() as usize;
    let samples = pix.samples();
    let mut data = Vec::with_capacity(w as usize * h as usize * 4);
    for y in 0..h as usize {
        let row = &samples[y * stride..y * stride + w as usize * n];
        for px in row.chunks_exact(n) {
            match n {
                4 => data.extend_from_slice(px),
                3 => data.extend_from_slice(&[px[0], px[1], px[2], 255]),
                1 => data.extend_from_slice(&[px[0], px[0], px[0], 255]),
                _ => data.extend_from_slice(&[px[0], px[1.min(n - 1)], px[2.min(n - 1)], 255]),
            }
        }
    }
    RgbaImage {
        width: w,
        height: h,
        data,
    }
}

fn check_size(w: u64, h: u64) -> Result<()> {
    if w == 0 || h == 0 || w * h > MAX_PIXELS {
        return Err(Error::InvalidInput(format!("tamaño de render no válido: {w}x{h}")));
    }
    Ok(())
}

impl EngineDocument for MupdfDocument {
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
        Ok(self.doc.page_count().map_err(pdf_err)?.max(0) as u32)
    }

    fn page_size(&self, page: u32) -> Result<PageSize> {
        let b = self.check_page(page)?.bounds().map_err(pdf_err)?;
        Ok(PageSize {
            width: b.x1 - b.x0,
            height: b.y1 - b.y0,
        })
    }

    fn render_page(&self, page: u32, zoom: f32) -> Result<RgbaImage> {
        let p = self.check_page(page)?;
        let b = p.bounds().map_err(pdf_err)?;
        check_size(
            ((b.x1 - b.x0) * zoom).ceil() as u64,
            ((b.y1 - b.y0) * zoom).ceil() as u64,
        )?;
        let pix = p
            .to_pixmap(&Matrix::new_scale(zoom, zoom), &Colorspace::device_rgb(), false, true)
            .map_err(pdf_err)?;
        Ok(to_rgba(&pix))
    }

    fn render_tile(&self, req: TileRequest) -> Result<RgbaImage> {
        if req.size == 0 || req.zoom.is_nan() || req.zoom <= 0.0 {
            return Err(Error::InvalidInput("tesela con tamaño o zoom no válidos".into()));
        }
        let p = self.check_page(req.page)?;
        let b = p.bounds().map_err(pdf_err)?;
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
        let mut pix = Pixmap::new_with_rect(&Colorspace::device_rgb(), rect, false).map_err(pdf_err)?;
        pix.clear_with(255).map_err(pdf_err)?;
        {
            let dev = Device::from_pixmap(&pix).map_err(pdf_err)?;
            // Origen de la página en (0,0) y escala al zoom pedido.
            let mut ctm = Matrix::new_translate(-b.x0, -b.y0);
            ctm.concat(Matrix::new_scale(req.zoom, req.zoom));
            p.run(&dev, &ctm).map_err(pdf_err)?;
        }
        Ok(to_rgba(&pix))
    }

    fn page_text(&self, page: u32) -> Result<String> {
        let p = self.check_page(page)?;
        p.to_text_page(TextPageFlags::empty())
            .map_err(pdf_err)?
            .to_text()
            .map_err(pdf_err)
    }

    fn search_page(&self, page: u32, needle: &str, max_hits: u32) -> Result<SearchHit> {
        let p = self.check_page(page)?;
        let quads = p.search(needle, max_hits.max(1)).map_err(pdf_err)?;
        Ok(SearchHit {
            page,
            quads: quads
                .iter()
                .map(|q| [q.ul.x, q.ul.y, q.ur.x, q.ur.y, q.ll.x, q.ll.y, q.lr.x, q.lr.y])
                .collect(),
        })
    }

    fn outline(&self) -> Result<Vec<OutlineItem>> {
        Ok(self.convert_outline(self.doc.outlines().map_err(pdf_err)?))
    }
}

/// Codifica una imagen RGBA a PNG usando MuPDF (sin dependencias extra).
pub fn encode_png(img: &RgbaImage) -> Result<Vec<u8>> {
    let rect = IRect {
        x0: 0,
        y0: 0,
        x1: img.width as i32,
        y1: img.height as i32,
    };
    let mut pix = Pixmap::new_with_rect(&Colorspace::device_rgb(), rect, true).map_err(pdf_err)?;
    let stride = pix.stride() as usize;
    let row_len = img.width as usize * 4;
    {
        let samples = pix.samples_mut();
        for y in 0..img.height as usize {
            samples[y * stride..y * stride + row_len].copy_from_slice(&img.data[y * row_len..(y + 1) * row_len]);
        }
    }
    let mut out = Vec::new();
    pix.write_to(&mut out, mupdf::ImageFormat::PNG).map_err(pdf_err)?;
    Ok(out)
}

#[cfg(test)]
mod tests;
