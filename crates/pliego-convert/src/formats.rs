//! Exportar un PDF a HTML y extraer las imágenes incrustadas.

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
