//! Detección de bloques de texto editables.

use pliego_core::Result;
use pliego_document::Document;
use pliego_pdf::{CharFont, PdfPage, TextBlockKind, TextChar, TextFlags};

use crate::fonts::family_from_name;
use crate::load_page;
use crate::types::{FontFamily, TextBlock};

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

fn color_of(c: &TextChar) -> [f32; 3] {
    let a = c.argb;
    [
        ((a >> 16) & 0xff) as f32 / 255.0,
        ((a >> 8) & 0xff) as f32 / 255.0,
        (a & 0xff) as f32 / 255.0,
    ]
}

pub(crate) fn extract(page: &PdfPage, index: usize) -> Result<Vec<RawBlock>> {
    let tp = page.to_text_page(TextFlags::PRESERVE_LIGATURES)?;
    let mut out = Vec::new();
    for (id, b) in tp.blocks().iter().enumerate() {
        if b.kind != TextBlockKind::Text {
            continue;
        }
        // Nombre, negrita, cursiva, con remate y monoespaciada: los de la fuente del primer carácter.
        let font: Option<FontInfo> = b
            .lines
            .first()
            .and_then(|l| l.chars.first())
            .and_then(|c| c.font.as_deref())
            .map(|f: &CharFont| (f.name.clone(), f.bold, f.italic, f.serif, f.monospaced));
        let texts: Vec<String> = b.lines.iter().map(|l| l.text()).collect();
        let lines: Vec<(&pliego_pdf::TextLine, &String)> = b
            .lines
            .iter()
            .zip(&texts)
            .filter(|(_, t)| !t.trim().is_empty())
            .collect();
        let Some((first, _)) = lines.first() else { continue };
        let Some(c0) = first.chars.first() else { continue };
        let (font_name, bold, italic, family) = match font {
            Some((name, bold, italic, serif, mono)) => {
                let fam = family_from_name(&name, serif, mono);
                (Some(name), bold, italic, fam)
            }
            None => (None, false, false, FontFamily::Sans),
        };
        let leading = lines.get(1).and_then(|(l2, _)| {
            let y2 = l2.chars.first()?.origin.y;
            let d = y2 - c0.origin.y;
            (d > 0.0).then_some(d)
        });
        out.push(RawBlock {
            block: TextBlock {
                page: index,
                id,
                bbox: [b.bounds.x0, b.bounds.y0, b.bounds.x1, b.bounds.y1],
                text: lines.iter().map(|(_, t)| t.trim_end()).collect::<Vec<_>>().join("\n"),
                lines: lines.iter().map(|(_, t)| t.trim_end().to_owned()).collect(),
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
