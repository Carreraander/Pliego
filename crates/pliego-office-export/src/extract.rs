//! Extracción de texto con división de líneas por huecos.
//!
//! Nota (decisión de gestión, 2026-11-04): este extractor se mantiene separado de
//! `pliego_convert::structured_text` porque los módulos no dependen entre sí (CONVENTIONS). Unificarlos pasaría por
//! extraer el texto estructurado a `pliego-core` (con un parámetro de hueco máximo); queda como mejora futura.
//!
//! MuPDF junta en una sola línea las palabras que comparten línea base aunque haya un hueco grande entre ellas
//! (columnas de una tabla, nombre y fecha en una cabecera). Para reconstruir tablas hace falta separarlas, y el
//! texto estructurado de `pliego-convert` no conserva la posición de cada carácter. Esta variante parte las
//! líneas cuando el hueco entre dos caracteres visibles supera [`SPLIT_EM`] veces el tamaño de la fuente.

use mupdf::text_page::TextBlockType;
use mupdf::{Document, Quad, TextPageFlags};
use pliego_convert::{BBox, Block, Line, Span, StructuredPage};
use pliego_core::{Error, Result};

use crate::util::pdf_err;

/// Hueco, en múltiplos del tamaño de fuente, a partir del cual una línea se parte en fragmentos.
/// Un espacio normal mide ~0,28 em; el texto justificado rara vez pasa de 0,6 em.
const SPLIT_EM: f32 = 0.75;

fn strip_subset_prefix(name: &str) -> &str {
    match name.split_once('+') {
        Some((prefix, rest)) if prefix.len() == 6 && prefix.chars().all(|c| c.is_ascii_uppercase()) => rest,
        _ => name,
    }
}

fn quad_bbox(q: &Quad) -> BBox {
    let xs = [q.ul.x, q.ur.x, q.ll.x, q.lr.x];
    let ys = [q.ul.y, q.ur.y, q.ll.y, q.lr.y];
    BBox {
        x0: xs.iter().copied().fold(f32::INFINITY, f32::min),
        y0: ys.iter().copied().fold(f32::INFINITY, f32::min),
        x1: xs.iter().copied().fold(f32::NEG_INFINITY, f32::max),
        y1: ys.iter().copied().fold(f32::NEG_INFINITY, f32::max),
    }
}

fn union(a: BBox, b: BBox) -> BBox {
    BBox {
        x0: a.x0.min(b.x0),
        y0: a.y0.min(b.y0),
        x1: a.x1.max(b.x1),
        y1: a.y1.max(b.y1),
    }
}

fn finish_piece(spans: &mut Vec<Span>, lines: &mut Vec<Line>, vertical: bool) {
    // Los espacios finales no cuentan para la caja del fragmento.
    if let Some(last) = spans.last_mut() {
        last.text = last.text.trim_end().to_string();
    }
    spans.retain(|s| !s.text.is_empty());
    if spans.is_empty() {
        return;
    }
    let bbox = spans.iter().map(|s| s.bbox).reduce(union).expect("al menos un tramo");
    lines.push(Line {
        bbox,
        vertical,
        spans: std::mem::take(spans),
    });
}

/// Texto estructurado de la página `page` (desde 0) con las líneas partidas por huecos grandes.
pub(crate) fn structured_split(doc: &Document, page: u32) -> Result<StructuredPage> {
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
                let mut lines: Vec<Line> = Vec::new();
                for line in block.lines() {
                    let vertical = matches!(line.wmode(), mupdf::WriteMode::Vertical);
                    let mut spans: Vec<Span> = Vec::new();
                    let mut prev: Option<(BBox, f32)> = None; // último carácter visible y su tamaño
                    for ch in line.chars() {
                        let Some(c) = ch.char() else { continue };
                        let bbox = quad_bbox(&ch.quad());
                        let size = (ch.size() * 100.0).round() / 100.0;
                        if !c.is_whitespace() {
                            if let Some((pb, psize)) = prev
                                && !vertical
                                && bbox.x0 - pb.x1 > SPLIT_EM * psize.max(size)
                            {
                                finish_piece(&mut spans, &mut lines, vertical);
                            }
                            prev = Some((bbox, size));
                        }
                        if spans.is_empty() && c.is_whitespace() {
                            continue;
                        }
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
                        match spans.last_mut() {
                            Some(s) if s.font == font && s.size == size && s.color == color => {
                                s.text.push(c);
                                if !c.is_whitespace() {
                                    s.bbox = union(s.bbox, bbox);
                                }
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
                    finish_piece(&mut spans, &mut lines, vertical);
                }
                if !lines.is_empty() {
                    let bbox = lines.iter().map(|l| l.bbox).reduce(union).expect("al menos una línea");
                    blocks.push(Block::Text { bbox, lines });
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
