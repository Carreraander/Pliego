//! Extracción de palabras con posición y formato, y render de páginas.

use pliego_core::{Error, Result};
use pliego_pdf::{Colorspace, Document, Matrix, Page, TextBlockKind, TextFlags};

use crate::model::Rect;

/// Abre un documento y lo desbloquea si hace falta.
pub(crate) fn open(path: &std::path::Path, password: Option<&str>) -> Result<Document> {
    if !path.exists() {
        return Err(Error::io_at(path, std::io::Error::from(std::io::ErrorKind::NotFound)));
    }
    let name = path
        .to_str()
        .ok_or_else(|| Error::InvalidInput(format!("ruta no válida: {}", path.display())))?;
    let mut doc = Document::open(name)?;
    if doc.needs_password()? {
        let ok = match password {
            Some(p) => doc.authenticate(p)?,
            None => false,
        };
        if !ok {
            return Err(Error::Password);
        }
    }
    Ok(doc)
}

#[derive(Debug, Clone)]
pub(crate) struct Word {
    pub text: String,
    pub rect: Rect,
    /// Nombre de la fuente sin el prefijo de subconjunto (`ABCDEF+`).
    pub font: String,
    pub size: f32,
}

pub(crate) struct PageData {
    pub words: Vec<Word>,
    /// Miniatura en niveles de oscuridad para emparejar páginas sin texto (escaneos).
    pub thumb: Option<Vec<u8>>,
}

const THUMB: usize = 48;

fn strip_subset(name: &str) -> &str {
    match name.split_once('+') {
        Some((tag, rest)) if tag.len() == 6 && tag.chars().all(|c| c.is_ascii_uppercase()) => rest,
        _ => name,
    }
}

/// Límites de la página: se usan para llevar el origen a (0, 0).
pub(crate) fn origin(page: &Page) -> Result<(f32, f32, f32, f32)> {
    let b = page.bounds()?;
    Ok((b.x0, b.y0, b.x1 - b.x0, b.y1 - b.y0))
}

pub(crate) fn page_data(doc: &Document, index: u32) -> Result<PageData> {
    let page = doc.load_page(index as i32)?;
    let (ox, oy, _, _) = origin(&page)?;
    let tp = page.to_text_page(TextFlags::empty())?;
    let mut words = Vec::new();
    for block in tp.blocks() {
        if block.kind != TextBlockKind::Text {
            continue;
        }
        for line in &block.lines {
            let mut cur = String::new();
            let mut rect: Option<Rect> = None;
            let mut font = String::new();
            let mut size = 0.0f32;
            let mut flush = |cur: &mut String, rect: &mut Option<Rect>, font: &str, size: f32| {
                if let Some(r) = rect.take() {
                    words.push(Word {
                        text: std::mem::take(cur),
                        rect: r,
                        font: font.to_string(),
                        size,
                    });
                }
                cur.clear();
            };
            for ch in &line.chars {
                let Some(c) = ch.ch else { continue };
                if c.is_whitespace() {
                    flush(&mut cur, &mut rect, &font, size);
                    continue;
                }
                let r = pliego_pdf::Rect::from(ch.quad);
                let r = Rect {
                    x0: r.x0 - ox,
                    y0: r.y0 - oy,
                    x1: r.x1 - ox,
                    y1: r.y1 - oy,
                };
                if rect.is_none() {
                    font = ch
                        .font
                        .as_ref()
                        .map(|f| strip_subset(&f.name).to_string())
                        .unwrap_or_default();
                    size = ch.size;
                }
                rect = Some(rect.map_or(r, |old| old.union(&r)));
                cur.push(c);
            }
            flush(&mut cur, &mut rect, &font, size);
        }
    }
    let thumb = if words.len() < 3 { Some(thumbnail(&page)?) } else { None };
    Ok(PageData { words, thumb })
}

fn thumbnail(page: &Page) -> Result<Vec<u8>> {
    let (ox, oy, w, h) = origin(page)?;
    let sx = THUMB as f32 / w.max(1.0);
    let sy = THUMB as f32 / h.max(1.0);
    let mut ctm = Matrix::new_translate(-ox, -oy);
    ctm.concat(Matrix::new_scale(sx, sy));
    let pix = page.to_pixmap(&ctm, Colorspace::Gray, false, true)?;
    let (pw, ph) = (pix.width() as usize, pix.height() as usize);
    let n = pix.n() as usize;
    let stride = pix.stride();
    let s = pix.samples();
    let mut out = vec![0u8; THUMB * THUMB];
    for y in 0..THUMB.min(ph) {
        for x in 0..THUMB.min(pw) {
            out[y * THUMB + x] = 255 - s[y * stride + x * n];
        }
    }
    Ok(out)
}

/// Página renderizada a RGB de 8 bits.
pub(crate) struct Raster {
    pub width: usize,
    pub height: usize,
    pub rgb: Vec<u8>,
}

pub(crate) fn render(doc: &Document, index: u32, scale: f32) -> Result<Raster> {
    let page = doc.load_page(index as i32)?;
    let (ox, oy, w, h) = origin(&page)?;
    let (pw, ph) = ((w * scale).ceil() as u64, (h * scale).ceil() as u64);
    if pw * ph > 60_000_000 {
        return Err(Error::InvalidInput(format!(
            "la página {} es demasiado grande para compararla a esta resolución",
            index + 1
        )));
    }
    let mut ctm = Matrix::new_translate(-ox, -oy);
    ctm.concat(Matrix::new_scale(scale, scale));
    let pix = page.to_pixmap(&ctm, Colorspace::Rgb, false, true)?;
    let (width, height) = (pix.width() as usize, pix.height() as usize);
    let n = pix.n() as usize;
    let stride = pix.stride();
    let s = pix.samples();
    let mut rgb = Vec::with_capacity(width * height * 3);
    for y in 0..height {
        for x in 0..width {
            let p = y * stride + x * n;
            rgb.extend_from_slice(&s[p..p + 3]);
        }
    }
    Ok(Raster { width, height, rgb })
}

/// Tamaño de la página en puntos.
pub(crate) fn page_size(doc: &Document, index: u32) -> Result<(f32, f32)> {
    let page = doc.load_page(index as i32)?;
    let (_, _, w, h) = origin(&page)?;
    Ok((w, h))
}
