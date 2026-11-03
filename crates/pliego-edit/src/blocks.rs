//! Detección de bloques de texto editables.

use mupdf::TextPageFlags;
use mupdf::pdf::PdfPage;
use mupdf::text_page::{StructuredTextChar, TextBlockContent};
use pliego_core::Result;
use pliego_document::Document;

use crate::fonts::family_from_name;
use crate::types::{FontFamily, TextBlock};
use crate::{load_page, pe};

/// Nombre, negrita, cursiva, con remate y monoespaciada.
type FontInfo = (String, bool, bool, bool, bool);

/// Primer carácter con su fuente, necesario para conocer nombre y estilo.
pub(crate) struct RawBlock {
    pub block: TextBlock,
    /// Origen (x, y) de la primera línea en el espacio de MuPDF.
    pub first_origin: (f32, f32),
    /// Distancia entre líneas base si el bloque tiene más de una línea.
    pub leading: Option<f32>,
}

fn color_of(c: &StructuredTextChar) -> [f32; 3] {
    let a = c.argb;
    [
        ((a >> 16) & 0xff) as f32 / 255.0,
        ((a >> 8) & 0xff) as f32 / 255.0,
        (a & 0xff) as f32 / 255.0,
    ]
}

pub(crate) fn extract(page: &PdfPage, index: usize) -> Result<Vec<RawBlock>> {
    let tp = page.to_text_page(TextPageFlags::PRESERVE_LIGATURES).map_err(pe)?;
    // Las fuentes solo se pueden consultar por el iterador de bajo nivel.
    let mut fonts: Vec<Option<FontInfo>> = Vec::new();
    for block in tp.blocks() {
        let info = block
            .lines()
            .next()
            .and_then(|l| l.chars().next().and_then(|c| c.font()))
            .map(|f| {
                (
                    f.name().to_owned(),
                    f.is_bold(),
                    f.is_italic(),
                    f.is_serif(),
                    f.is_monospaced(),
                )
            });
        fonts.push(info);
    }
    let st = tp.structured();
    let mut out = Vec::new();
    for (id, (b, font)) in st.blocks.iter().zip(fonts).enumerate() {
        let TextBlockContent::Text { lines } = &b.content else {
            continue;
        };
        let lines: Vec<_> = lines.iter().filter(|l| !l.text.trim().is_empty()).collect();
        let Some(first) = lines.first() else { continue };
        let Some(c0) = first.chars.first() else { continue };
        let (font_name, bold, italic, family) = match font {
            Some((name, bold, italic, serif, mono)) => {
                let fam = family_from_name(&name, serif, mono);
                (Some(name), bold, italic, fam)
            }
            None => (None, false, false, FontFamily::Sans),
        };
        let leading = lines.get(1).and_then(|l2| {
            let y2 = l2.chars.first()?.origin.y;
            let d = y2 - c0.origin.y;
            (d > 0.0).then_some(d)
        });
        out.push(RawBlock {
            block: TextBlock {
                page: index,
                id,
                bbox: [b.bounds.x0, b.bounds.y0, b.bounds.x1, b.bounds.y1],
                text: lines.iter().map(|l| l.text.trim_end()).collect::<Vec<_>>().join("\n"),
                lines: lines.iter().map(|l| l.text.trim_end().to_owned()).collect(),
                font_name,
                font_size: c0.size,
                color: color_of(c0),
                bold,
                italic,
                family,
            },
            first_origin: (c0.origin.x, c0.origin.y),
            leading,
        });
    }
    Ok(out)
}

/// Bloques de texto editables de una página (desde 0), en orden de lectura.
pub fn text_blocks(doc: &Document, page: usize) -> Result<Vec<TextBlock>> {
    let p = load_page(doc.pdf(), page)?;
    Ok(extract(&p, page)?.into_iter().map(|r| r.block).collect())
}
