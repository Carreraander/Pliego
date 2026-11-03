//! Texto estructurado de una página: bloques, líneas y tramos (spans) con fuente, tamaño, color y caja.
//!
//! Es la base que usará la exportación a Office (W14) y cualquier módulo que necesite saber *cómo* está
//! maquetado el texto. Todo es serializable (`serde`); las coordenadas están en puntos, con origen arriba a
//! la izquierda y la y creciendo hacia abajo (el sistema de MuPDF para páginas ya rotadas).

use std::path::Path;

use mupdf::text_page::TextBlockType;
use mupdf::{Document, Rect, TextPageFlags};
use pliego_core::{Error, PageRange, Result};
use serde::{Deserialize, Serialize};

use crate::util::{open_document, pdf_err};

/// Rectángulo en puntos.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BBox {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

impl From<Rect> for BBox {
    fn from(r: Rect) -> Self {
        Self {
            x0: r.x0,
            y0: r.y0,
            x1: r.x1,
            y1: r.y1,
        }
    }
}

impl BBox {
    fn union(self, o: BBox) -> BBox {
        BBox {
            x0: self.x0.min(o.x0),
            y0: self.y0.min(o.y0),
            x1: self.x1.max(o.x1),
            y1: self.y1.max(o.y1),
        }
    }
}

/// Tramo de texto contiguo con el mismo estilo dentro de una línea.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Span {
    pub text: String,
    /// Nombre de la fuente sin el prefijo de subconjunto (`ABCDEF+`). Vacío si no se conoce.
    pub font: String,
    /// Tamaño en puntos.
    pub size: f32,
    /// Color de relleno como `#rrggbb`.
    pub color: String,
    pub bold: bool,
    pub italic: bool,
    pub monospace: bool,
    pub serif: bool,
    pub bbox: BBox,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Line {
    pub bbox: BBox,
    /// `true` si la línea es vertical.
    pub vertical: bool,
    pub spans: Vec<Span>,
}

impl Line {
    pub fn text(&self) -> String {
        self.spans.iter().map(|s| s.text.as_str()).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Block {
    Text { bbox: BBox, lines: Vec<Line> },
    Image { bbox: BBox },
}

impl Block {
    pub fn bbox(&self) -> BBox {
        match self {
            Block::Text { bbox, .. } | Block::Image { bbox } => *bbox,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructuredPage {
    /// Índice de página desde 0.
    pub page: u32,
    pub width: f32,
    pub height: f32,
    pub blocks: Vec<Block>,
}

impl StructuredPage {
    /// Texto plano: una línea por línea, un salto extra entre bloques.
    pub fn plain_text(&self) -> String {
        let mut out = String::new();
        for b in &self.blocks {
            if let Block::Text { lines, .. } = b {
                for l in lines {
                    out.push_str(&l.text());
                    out.push('\n');
                }
                out.push('\n');
            }
        }
        out
    }
}

fn strip_subset_prefix(name: &str) -> &str {
    match name.split_once('+') {
        Some((prefix, rest)) if prefix.len() == 6 && prefix.chars().all(|c| c.is_ascii_uppercase()) => rest,
        _ => name,
    }
}

fn quad_bbox(q: &mupdf::Quad) -> BBox {
    let xs = [q.ul.x, q.ur.x, q.ll.x, q.lr.x];
    let ys = [q.ul.y, q.ur.y, q.ll.y, q.lr.y];
    BBox {
        x0: xs.iter().copied().fold(f32::INFINITY, f32::min),
        y0: ys.iter().copied().fold(f32::INFINITY, f32::min),
        x1: xs.iter().copied().fold(f32::NEG_INFINITY, f32::max),
        y1: ys.iter().copied().fold(f32::NEG_INFINITY, f32::max),
    }
}

/// Devuelve el texto estructurado de la página `page` (desde 0) de un documento ya abierto.
pub fn structured_text(doc: &Document, page: u32) -> Result<StructuredPage> {
    let count = doc.page_count().map_err(pdf_err)? as u32;
    if page >= count {
        return Err(Error::InvalidInput(format!(
            "página {} fuera de rango (hay {count})",
            page + 1
        )));
    }
    let p = doc.load_page(page as i32).map_err(pdf_err)?;
    let b = p.bounds().map_err(pdf_err)?;
    let tp = p
        .to_text_page(TextPageFlags::PRESERVE_IMAGES | TextPageFlags::PRESERVE_LIGATURES)
        .map_err(pdf_err)?;

    let mut blocks = Vec::new();
    for block in tp.blocks() {
        match block.r#type() {
            TextBlockType::Image => blocks.push(Block::Image {
                bbox: block.bounds().into(),
            }),
            TextBlockType::Text => {
                let mut lines = Vec::new();
                for line in block.lines() {
                    let mut spans: Vec<Span> = Vec::new();
                    for ch in line.chars() {
                        let Some(c) = ch.char() else { continue };
                        let (font, bold, italic, mono, serif) = match ch.font() {
                            Some(f) => (
                                strip_subset_prefix(f.name()).to_string(),
                                f.is_bold(),
                                f.is_italic(),
                                f.is_monospaced(),
                                f.is_serif(),
                            ),
                            None => (String::new(), false, false, false, false),
                        };
                        let color = format!("#{:06x}", ch.argb() & 0x00ff_ffff);
                        let size = (ch.size() * 100.0).round() / 100.0;
                        let bbox = quad_bbox(&ch.quad());
                        match spans.last_mut() {
                            Some(s) if s.font == font && s.size == size && s.color == color => {
                                s.text.push(c);
                                s.bbox = s.bbox.union(bbox);
                            }
                            _ => spans.push(Span {
                                text: c.to_string(),
                                font,
                                size,
                                color,
                                bold,
                                italic,
                                monospace: mono,
                                serif,
                                bbox,
                            }),
                        }
                    }
                    if spans.is_empty() {
                        continue;
                    }
                    lines.push(Line {
                        bbox: line.bounds().into(),
                        vertical: matches!(line.wmode(), mupdf::WriteMode::Vertical),
                        spans,
                    });
                }
                if !lines.is_empty() {
                    blocks.push(Block::Text {
                        bbox: block.bounds().into(),
                        lines,
                    });
                }
            }
            _ => {}
        }
    }
    Ok(StructuredPage {
        page,
        width: b.width(),
        height: b.height(),
        blocks,
    })
}

/// Abre `input` y devuelve el texto estructurado de las páginas indicadas.
pub fn structured_text_pages(input: &Path, password: Option<&str>, pages: &PageRange) -> Result<Vec<StructuredPage>> {
    let doc = open_document(input, password)?;
    let count = doc.page_count().map_err(pdf_err)? as u32;
    pages
        .resolve(count)?
        .into_iter()
        .map(|p| structured_text(&doc, p))
        .collect()
}
