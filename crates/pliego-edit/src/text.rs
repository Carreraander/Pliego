//! Reemplazo e inserción de texto con reflujo.

use std::fmt::Write as _;

use mupdf::Rect;
use mupdf::pdf::{
    InsertFontOptions, PdfPage, PdfRedactImageMethod, PdfRedactLineArtMethod, PdfRedactOptions, PdfRedactTextMethod,
};
use pliego_core::{Error, Report, Result};
use pliego_document::Document;

use crate::blocks::extract;
use crate::fonts::{Metrics, base14_name, encode};
use crate::types::{Align, FontFamily, RectPts, Style};
use crate::{load_page, pe};

const DEFAULT_SIZE: f32 = 12.0;

/// Texto ya resuelto: fuente, tamaño y color finales.
struct Resolved {
    font: &'static str,
    size: f32,
    color: [f32; 3],
    align: Align,
}

fn resolve(style: &Style, base: Option<(&crate::TextBlock, ())>) -> Resolved {
    let (b_family, b_bold, b_italic, b_size, b_color) = match base {
        Some((b, _)) => (b.family, b.bold, b.italic, b.font_size, b.color),
        None => (FontFamily::Sans, false, false, DEFAULT_SIZE, [0.0; 3]),
    };
    Resolved {
        font: base14_name(
            style.family.unwrap_or(b_family),
            style.bold.unwrap_or(b_bold),
            style.italic.unwrap_or(b_italic),
        ),
        size: style.size.filter(|s| *s > 0.0).unwrap_or(b_size),
        color: style.color.unwrap_or(b_color),
        align: style.align,
    }
}

/// Reparte el texto en líneas que caben en `max_width` (sin límite si es `None`).
fn wrap(text: &str, metrics: &Metrics, size: f32, max_width: Option<f32>) -> Vec<String> {
    let mut lines = Vec::new();
    for para in text.split('\n') {
        let Some(max) = max_width else {
            lines.push(para.to_owned());
            continue;
        };
        let mut cur = String::new();
        for word in para.split_whitespace() {
            let candidate = if cur.is_empty() {
                word.to_owned()
            } else {
                format!("{cur} {word}")
            };
            if cur.is_empty() || metrics.width(&candidate, size) <= max {
                cur = candidate;
            } else {
                lines.push(std::mem::take(&mut cur));
                cur = word.to_owned();
            }
        }
        lines.push(cur);
    }
    lines
}

fn pdf_num(v: f32) -> String {
    let s = format!("{v:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// Escribe las líneas como un flujo de contenido nuevo sobre la página. `origin` es la línea base de la
/// primera línea (espacio MuPDF) y `x0`/`x1` los límites horizontales para la alineación.
#[allow(clippy::too_many_arguments)]
fn draw(
    page: &mut PdfPage,
    doc: &mut mupdf::pdf::PdfDocument,
    r: &Resolved,
    lines: &[String],
    x0: f32,
    x1: f32,
    baseline: f32,
    leading: f32,
    metrics: &Metrics,
) -> Result<()> {
    let (font_res, _, _) = page.insert_font(doc, &InsertFontOptions::new(r.font)).map_err(pe)?;
    let inv = page
        .object()
        .page_ctm()
        .map_err(pe)?
        .invert()
        .ok_or_else(|| Error::Pdf("la matriz de la página no es invertible".into()))?;
    let mut s = String::new();
    let [cr, cg, cb] = r.color;
    let _ = writeln!(s, "q\n{} {} {} rg\nBT", pdf_num(cr), pdf_num(cg), pdf_num(cb));
    let _ = writeln!(s, "{font_res} {} Tf", pdf_num(r.size));
    for (i, line) in lines.iter().enumerate() {
        if line.is_empty() {
            continue;
        }
        let w = metrics.width(line, r.size);
        let x = match r.align {
            Align::Left => x0,
            Align::Center => x0 + ((x1 - x0) - w) / 2.0,
            Align::Right => x1 - w,
        };
        let (ux, uy) = inv.transform_xy(x, baseline + leading * i as f32);
        let hex: String = encode(line)?.iter().map(|b| format!("{b:02X}")).collect();
        let _ = writeln!(s, "1 0 0 1 {} {} Tm <{hex}> Tj", pdf_num(ux), pdf_num(uy));
    }
    s.push_str("ET\nQ\n");
    page.insert_contents(doc, s.as_bytes(), true).map_err(pe)?;
    Ok(())
}

fn check_text(text: &str) -> Result<()> {
    encode(text).map(|_| ())
}

fn rect_of(r: RectPts) -> Rect {
    Rect::new(r[0], r[1], r[2], r[3])
}

/// Reemplaza el texto del bloque `block` (id devuelto por [`crate::text_blocks`]) de la página `page`.
///
/// Se retira el texto original dentro del bloque (sin tocar imágenes ni gráficos) y se compone `new_text`
/// con reflujo dentro del ancho del bloque, empezando donde empezaba el original. Lo que queda fuera del
/// bloque no cambia. La fuente se sustituye por una estándar parecida y se avisa en el informe.
pub fn replace_text(doc: &mut Document, page: usize, block: usize, new_text: &str, style: &Style) -> Result<Report> {
    check_text(new_text)?;
    let mut report = Report::default();
    doc.with_pdf_mut(|pdf| {
        let mut p = load_page(pdf, page)?;
        let raw = extract(&p, page)?
            .into_iter()
            .find(|r| r.block.id == block)
            .ok_or_else(|| {
                Error::InvalidInput(format!("la página {} no tiene el bloque de texto {block}", page + 1))
            })?;
        let b = &raw.block;
        let res = resolve(style, Some((b, ())));
        let metrics = Metrics::new(res.font)?;

        // Retirar el texto original. El recuadro se encoge un poco para no arrastrar líneas vecinas.
        let inset_y = (b.font_size * 0.15).min((b.bbox[3] - b.bbox[1]) / 4.0);
        let area = rect_of([
            b.bbox[0] + 0.5,
            b.bbox[1] + inset_y,
            b.bbox[2] - 0.5,
            b.bbox[3] - inset_y,
        ]);
        p.add_redact_annotation(area).map_err(pe)?;
        let opts = PdfRedactOptions {
            black_boxes: false,
            image_method: PdfRedactImageMethod::None,
            line_art: PdfRedactLineArtMethod::None,
            text: PdfRedactTextMethod::Remove,
        };
        p.redact_with_options(opts).map_err(pe)?;
        let mut p = load_page(pdf, page)?;

        let max_width = (b.bbox[2] - raw.first_origin.0).max(res.size);
        let lines = wrap(new_text, &metrics, res.size, Some(max_width));
        let leading = raw.leading.unwrap_or(res.size * 1.2);
        draw(
            &mut p,
            pdf,
            &res,
            &lines,
            raw.first_origin.0,
            b.bbox[2],
            raw.first_origin.1,
            leading,
            &metrics,
        )?;

        let new_bottom = raw.first_origin.1 + leading * (lines.len().saturating_sub(1)) as f32;
        if new_bottom > b.bbox[3] + 0.5 {
            report.warnings.push(
                "el texto nuevo ocupa más líneas que el original y puede invadir el contenido que hay debajo".into(),
            );
        }
        let original = b.font_name.clone().unwrap_or_else(|| "desconocida".into());
        report.warnings.push(format!(
            "se usó la fuente estándar {} en lugar de «{original}»: el aspecto del texto puede variar ligeramente",
            res.font
        ));
        report.summary = format!("Texto del bloque {block} reemplazado en la página {}", page + 1);
        Ok(())
    })?;
    Ok(report)
}

/// Añade texto nuevo dentro de `rect` (espacio MuPDF) de la página `page`, con reflujo al ancho del recuadro.
pub fn insert_text(doc: &mut Document, page: usize, rect: RectPts, text: &str, style: &Style) -> Result<Report> {
    check_text(text)?;
    if rect[2] <= rect[0] || rect[3] <= rect[1] {
        return Err(Error::InvalidInput("el recuadro del texto está vacío".into()));
    }
    let mut report = Report::default();
    doc.with_pdf_mut(|pdf| {
        let mut p = load_page(pdf, page)?;
        let res = resolve(style, None);
        let metrics = Metrics::new(res.font)?;
        let lines = wrap(text, &metrics, res.size, Some(rect[2] - rect[0]));
        let font = mupdf::Font::new(res.font).map_err(pe)?;
        let ascent = font.ascender().clamp(0.5, 1.2) * res.size;
        let leading = res.size * 1.2;
        draw(
            &mut p,
            pdf,
            &res,
            &lines,
            rect[0],
            rect[2],
            rect[1] + ascent,
            leading,
            &metrics,
        )?;
        let bottom = rect[1] + ascent + leading * (lines.len().saturating_sub(1)) as f32;
        if bottom > rect[3] + 0.5 {
            report
                .warnings
                .push("el texto no cabe en el recuadro y se sale por abajo".into());
        }
        report.summary = format!("Texto añadido en la página {}", page + 1);
        Ok(())
    })?;
    Ok(report)
}
