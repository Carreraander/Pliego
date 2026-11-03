//! Exportar un PDF a imágenes por página y a texto.

use std::io::Cursor;
use std::path::Path;
use std::str::FromStr;

use image::{ExtendedColorType, ImageEncoder, codecs};
use mupdf::{Colorspace, Matrix, Pixmap, TextPageFlags};
use pliego_core::{Error, JobContext, PageRange, Report, Result};
use serde::{Deserialize, Serialize};

use crate::util::{ensure_not_same, file_stem, open_document, pdf_err, write_atomic};

/// Límite de seguridad por imagen (≈ 300 MB RGB), igual de prudente que el visor.
const MAX_PIXELS: u64 = 100_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportImageFormat {
    #[default]
    Png,
    Jpeg,
    Tiff,
}

impl ExportImageFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Tiff => "tiff",
        }
    }
}

impl FromStr for ExportImageFormat {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "png" => Ok(Self::Png),
            "jpg" | "jpeg" => Ok(Self::Jpeg),
            "tif" | "tiff" => Ok(Self::Tiff),
            other => Err(Error::InvalidInput(format!(
                "formato de imagen no válido: {other:?} (usa png, jpg o tiff)"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PdfToImagesOptions {
    pub format: ExportImageFormat,
    /// Puntos por pulgada.
    pub dpi: f32,
    /// Calidad JPEG (1-100).
    pub jpeg_quality: u8,
}

impl Default for PdfToImagesOptions {
    fn default() -> Self {
        Self {
            format: ExportImageFormat::Png,
            dpi: 150.0,
            jpeg_quality: 90,
        }
    }
}

fn pixmap_rgb(pix: &Pixmap) -> Result<Vec<u8>> {
    let (w, h, n, stride) = (
        pix.width() as usize,
        pix.height() as usize,
        pix.n() as usize,
        pix.stride() as usize,
    );
    if n != 3 {
        return Err(Error::Pdf(format!("mapa de bits inesperado de {n} canales")));
    }
    let samples = pix.samples();
    let mut data = Vec::with_capacity(w * h * 3);
    for y in 0..h {
        data.extend_from_slice(&samples[y * stride..y * stride + w * 3]);
    }
    Ok(data)
}

fn encode(pix: &Pixmap, o: &PdfToImagesOptions) -> Result<Vec<u8>> {
    let (w, h) = (pix.width(), pix.height());
    let rgb = pixmap_rgb(pix)?;
    let mut out = Vec::new();
    let fail = |e: image::ImageError| Error::Other(format!("no se pudo codificar la imagen: {e}"));
    match o.format {
        ExportImageFormat::Png => codecs::png::PngEncoder::new(&mut out)
            .write_image(&rgb, w, h, ExtendedColorType::Rgb8)
            .map_err(fail)?,
        ExportImageFormat::Jpeg => codecs::jpeg::JpegEncoder::new_with_quality(&mut out, o.jpeg_quality.clamp(1, 100))
            .write_image(&rgb, w, h, ExtendedColorType::Rgb8)
            .map_err(fail)?,
        ExportImageFormat::Tiff => {
            let mut cur = Cursor::new(&mut out);
            codecs::tiff::TiffEncoder::new(&mut cur)
                .write_image(&rgb, w, h, ExtendedColorType::Rgb8)
                .map_err(fail)?
        }
    }
    Ok(out)
}

/// Exporta cada página a un archivo `<nombre>-NNN.<ext>` dentro de `out_dir`.
///
/// `pages` usa el rango habitual (desde 1 en texto). Devuelve los archivos creados en `Report::outputs`.
pub fn pdf_to_images(
    input: &Path,
    password: Option<&str>,
    pages: &PageRange,
    out_dir: &Path,
    options: &PdfToImagesOptions,
    ctx: &JobContext,
) -> Result<Report> {
    if !options.dpi.is_finite() || options.dpi <= 0.0 {
        return Err(Error::InvalidInput("la resolución debe ser mayor que 0".into()));
    }
    let doc = open_document(input, password)?;
    let count = doc.page_count().map_err(pdf_err)? as u32;
    let selected = pages.resolve(count)?;
    let stem = file_stem(input, "pagina");
    let scale = options.dpi / 72.0;
    let mut report = Report::default();
    for (n, &p) in selected.iter().enumerate() {
        ctx.step(n as u64, selected.len() as u64, &format!("página {}", p + 1))?;
        let page = doc.load_page(p as i32).map_err(pdf_err)?;
        let b = page.bounds().map_err(pdf_err)?;
        let (w, h) = (
            (b.width() * scale).ceil().max(1.0) as u64,
            (b.height() * scale).ceil().max(1.0) as u64,
        );
        if w * h > MAX_PIXELS {
            return Err(Error::InvalidInput(format!(
                "la página {} a {} ppp daría {w}x{h} píxeles; baja la resolución",
                p + 1,
                options.dpi
            )));
        }
        let pix = page
            .to_pixmap(&Matrix::new_scale(scale, scale), &Colorspace::device_rgb(), false, true)
            .map_err(pdf_err)?;
        let path = out_dir.join(format!("{stem}-{:03}.{}", p + 1, options.format.extension()));
        ensure_not_same(input, &path)?;
        write_atomic(&path, &encode(&pix, options)?)?;
        report.outputs.push(path);
    }
    ctx.step(selected.len() as u64, selected.len() as u64, "listo")?;
    report.summary = format!("{} imagen(es) exportadas", report.outputs.len());
    Ok(report)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextExportOptions {
    /// Une las palabras partidas por guion al final de línea.
    pub dehyphenate: bool,
    /// Separa las páginas con un salto de página (U+000C), como hace `pdftotext`.
    pub page_separator: bool,
}

impl Default for TextExportOptions {
    fn default() -> Self {
        Self {
            dehyphenate: false,
            page_separator: true,
        }
    }
}

/// Extrae el texto de las páginas indicadas a un archivo UTF-8.
pub fn pdf_to_text(
    input: &Path,
    password: Option<&str>,
    pages: &PageRange,
    output: &Path,
    options: &TextExportOptions,
    ctx: &JobContext,
) -> Result<Report> {
    ensure_not_same(input, output)?;
    let doc = open_document(input, password)?;
    let count = doc.page_count().map_err(pdf_err)? as u32;
    let selected = pages.resolve(count)?;
    let mut flags = TextPageFlags::empty();
    if options.dehyphenate {
        flags |= TextPageFlags::DEHYPHENATE;
    }
    let mut text = String::new();
    let mut empty_pages = 0;
    for (n, &p) in selected.iter().enumerate() {
        ctx.step(n as u64, selected.len() as u64, &format!("página {}", p + 1))?;
        let page = doc.load_page(p as i32).map_err(pdf_err)?;
        let t = page.to_text_page(flags).map_err(pdf_err)?.to_text().map_err(pdf_err)?;
        if t.trim().is_empty() {
            empty_pages += 1;
        }
        text.push_str(&t);
        if options.page_separator && n + 1 < selected.len() {
            text.push('\u{c}');
        }
    }
    write_atomic(output, text.as_bytes())?;
    ctx.step(selected.len() as u64, selected.len() as u64, "listo")?;
    let mut report = Report::with_output(output, format!("texto de {} página(s)", selected.len()));
    if empty_pages > 0 {
        report.warnings.push(format!(
            "{empty_pages} página(s) sin texto seleccionable (¿escaneadas? el reconocimiento de texto (OCR) puede extraerlo)"
        ));
    }
    Ok(report)
}
