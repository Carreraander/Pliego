//! Exportar un PDF a HTML y SVG, y extraer las imágenes incrustadas.

use std::collections::HashSet;
use std::path::Path;

use pliego_core::{Error, JobContext, PageRange, Report, Result};
use pliego_pdf::{
    Colorspace, ImageFormat, ImageSource, InsertImageOptions, Matrix, PdfDocument, Rect, Size, TextFlags,
};
use serde::{Deserialize, Serialize};

use crate::util::{ensure_not_same, file_stem, open_document, write_atomic};

/// Escapa texto para meterlo en HTML.
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Exporta las páginas a un único HTML con el texto estructurado (una `<div>` por página).
pub fn pdf_to_html(
    input: &Path,
    password: Option<&str>,
    pages: &PageRange,
    output: &Path,
    ctx: &JobContext,
) -> Result<Report> {
    ensure_not_same(input, output)?;
    let doc = open_document(input, password)?;
    let count = doc.page_count()? as u32;
    let selected = pages.resolve(count)?;
    let title = file_stem(input, "documento");
    let mut html = format!(
        "<!DOCTYPE html>\n<html lang=\"es\">\n<head>\n<meta charset=\"utf-8\">\n<title>{}</title>\n</head>\n<body>\n",
        escape(&title)
    );
    for (n, &p) in selected.iter().enumerate() {
        ctx.step(n as u64, selected.len() as u64, &format!("página {}", p + 1))?;
        let page = doc.load_page(p as i32)?;
        let tp = page.to_text_page(TextFlags::PRESERVE_LIGATURES)?;
        html.push_str(&tp.to_html(p as i32, false)?);
        html.push('\n');
    }
    html.push_str("</body>\n</html>\n");
    write_atomic(output, html.as_bytes())?;
    ctx.step(selected.len() as u64, selected.len() as u64, "listo")?;
    Ok(Report::with_output(
        output,
        format!("HTML de {} página(s)", selected.len()),
    ))
}

/// Resolución de la imagen de página de un SVG (ppp). El SVG mide lo mismo que la página en puntos.
const SVG_DPI: f32 = 150.0;

/// Exporta cada página a `<nombre>-NNN.svg` dentro de `out_dir`.
///
/// Cada SVG lleva la página dibujada como imagen PNG incrustada (a 150 ppp, fiel al original) y encima su texto,
/// invisible pero seleccionable y buscable (`<text>` con `fill-opacity="0"`). No es un SVG vectorial: los trazados no
/// se pueden editar.
pub fn pdf_to_svg(
    input: &Path,
    password: Option<&str>,
    pages: &PageRange,
    out_dir: &Path,
    ctx: &JobContext,
) -> Result<Report> {
    let doc = open_document(input, password)?;
    let count = doc.page_count()? as u32;
    let selected = pages.resolve(count)?;
    let stem = file_stem(input, "pagina");
    let mut report = Report::default();
    for (n, &p) in selected.iter().enumerate() {
        ctx.step(n as u64, selected.len() as u64, &format!("página {}", p + 1))?;
        let page = doc.load_page(p as i32)?;
        let svg = page_to_svg(&page)?;
        let path = out_dir.join(format!("{stem}-{:03}.svg", p + 1));
        write_atomic(&path, svg.as_bytes())?;
        report.outputs.push(path);
    }
    ctx.step(selected.len() as u64, selected.len() as u64, "listo")?;
    report.summary = format!("{} SVG exportados", report.outputs.len());
    Ok(report)
}

/// SVG de una página: imagen de fondo más capa de texto invisible.
fn page_to_svg(page: &pliego_pdf::Page) -> Result<String> {
    let bounds = page.bounds()?;
    let (w, h) = (bounds.x1 - bounds.x0, bounds.y1 - bounds.y0);
    let scale = SVG_DPI / 72.0;
    let pix = page.to_pixmap(&Matrix::new_scale(scale, scale), Colorspace::Rgb, false, true)?;
    let png = crate::export::encode(
        &pix,
        &crate::export::PdfToImagesOptions {
            format: crate::export::ExportImageFormat::Png,
            ..Default::default()
        },
    )?;
    let mut svg = String::with_capacity(png.len() * 4 / 3 + 4096);
    svg.push_str(&format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\" \
xmlns:xlink=\"http://www.w3.org/1999/xlink\" width=\"{w:.2}pt\" height=\"{h:.2}pt\" viewBox=\"0 0 {w:.2} {h:.2}\">\n"
    ));
    svg.push_str(&format!(
        "<image x=\"0\" y=\"0\" width=\"{w:.2}\" height=\"{h:.2}\" preserveAspectRatio=\"none\" \
xlink:href=\"data:image/png;base64,{}\"/>\n",
        base64(&png)
    ));
    svg.push_str("<g fill=\"#000\" fill-opacity=\"0\" font-family=\"sans-serif\" xml:space=\"preserve\">\n");
    let tp = page.to_text_page(TextFlags::PRESERVE_LIGATURES)?;
    for block in tp.blocks() {
        for line in block.lines {
            let text = line.text();
            let (Some(first), true) = (line.chars.first(), !text.trim().is_empty()) else {
                continue;
            };
            let width = line.bounds.x1 - line.bounds.x0;
            svg.push_str(&format!(
                "<text x=\"{:.2}\" y=\"{:.2}\" font-size=\"{:.2}\" textLength=\"{:.2}\" lengthAdjust=\"spacingAndGlyphs\">{}</text>\n",
                first.origin.x - bounds.x0,
                first.origin.y - bounds.y0,
                first.size.max(1.0),
                width.max(0.1),
                escape_xml(&text)
            ));
        }
    }
    svg.push_str("</g>\n</svg>\n");
    Ok(svg)
}

/// Escapa texto para XML y descarta los caracteres de control que XML 1.0 no admite.
fn escape_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\t' | '\n' | '\r' => out.push(' '),
            c if (c as u32) < 0x20 || c == '\u{fffe}' || c == '\u{ffff}' || ('\u{7f}'..='\u{9f}').contains(&c) => {}
            c => out.push(c),
        }
    }
    out
}

/// Base64 estándar con relleno (RFC 4648).
fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ExtractImagesOptions {
    /// Ignora imágenes cuyo ancho o alto en píxeles sea menor.
    pub min_dimension: u32,
}

impl Default for ExtractImagesOptions {
    fn default() -> Self {
        Self { min_dimension: 1 }
    }
}

/// Extrae las imágenes incrustadas. JPEG (`DCTDecode`) y JPEG 2000 (`JPXDecode`) se guardan tal cual, en su
/// formato original; el resto se decodifica y se guarda como PNG. Cada imagen se extrae una sola vez aunque
/// aparezca en varias páginas. Nombre: `<nombre>-pNNN-MM.<ext>` (página donde aparece por primera vez).
pub fn extract_images(
    input: &Path,
    password: Option<&str>,
    pages: &PageRange,
    out_dir: &Path,
    options: &ExtractImagesOptions,
    ctx: &JobContext,
) -> Result<Report> {
    // Se valida con el abridor común (contraseña) y luego se reabre como PDF.
    let checked = open_document(input, password)?;
    if !checked.is_pdf() {
        return Err(Error::InvalidInput(
            "solo se pueden extraer imágenes de archivos PDF".into(),
        ));
    }
    let pdf = checked.into_pdf()?;
    let count = pdf.page_count()? as u32;
    let selected = pages.resolve(count)?;
    let stem = file_stem(input, "imagen");
    let mut seen = HashSet::new();
    let mut report = Report::default();
    for (n, &p) in selected.iter().enumerate() {
        ctx.step(n as u64, selected.len() as u64, &format!("página {}", p + 1))?;
        let page = pdf.load_pdf_page(p as i32)?;
        let mut idx = 0;
        for info in page.images()? {
            if !seen.insert(info.xref) {
                continue;
            }
            if info.width < options.min_dimension || info.height < options.min_dimension {
                continue;
            }
            idx += 1;
            let (bytes, ext) = match info.filter.as_deref() {
                Some(f) if f.contains("DCTDecode") && !f.contains(' ') => {
                    (pdf.extract_image(info.xref)?.encoded, "jpg")
                }
                Some(f) if f.contains("JPXDecode") && !f.contains(' ') => {
                    (pdf.extract_image(info.xref)?.encoded, "jp2")
                }
                _ => match decode_to_png(&pdf, info.xref, info.width, info.height) {
                    Ok(b) => (b, "png"),
                    Err(e) => {
                        report
                            .warnings
                            .push(format!("página {}: no se pudo extraer una imagen ({e})", p + 1));
                        continue;
                    }
                },
            };
            let path = out_dir.join(format!("{stem}-p{:03}-{idx:02}.{ext}", p + 1));
            write_atomic(&path, &bytes)?;
            report.outputs.push(path);
        }
    }
    ctx.step(selected.len() as u64, selected.len() as u64, "listo")?;
    report.summary = format!("{} imagen(es) extraídas", report.outputs.len());
    Ok(report)
}

/// Decodifica un XObject de imagen a PNG: lo injerta en un PDF temporal de una página del tamaño exacto
/// y lo renderiza a 1 píxel por punto, de modo que MuPDF resuelva espacios de color, máscaras y filtros.
fn decode_to_png(src: &PdfDocument, xref: i32, w: u32, h: u32) -> Result<Vec<u8>> {
    if w == 0 || h == 0 || u64::from(w) * u64::from(h) > 100_000_000 {
        return Err(Error::InvalidInput(format!("tamaño de imagen no válido: {w}x{h}")));
    }
    let obj = src.new_indirect(xref, 0)?;
    let mut tmp = PdfDocument::new();
    let grafted = tmp.graft_object(&obj)?;
    let mut page = tmp.new_page(Size {
        width: w as f32,
        height: h as f32,
    })?;
    page.insert_image(
        &mut tmp,
        Rect {
            x0: 0.0,
            y0: 0.0,
            x1: w as f32,
            y1: h as f32,
        },
        ImageSource::ExistingXref(grafted.as_indirect()?),
        InsertImageOptions::default(),
    )?;
    let pix = page.to_pixmap(&Matrix::IDENTITY, Colorspace::Rgb, true, false)?;
    let mut out = Vec::new();
    pix.write_to(&mut out, ImageFormat::Png)?;
    Ok(out)
}
