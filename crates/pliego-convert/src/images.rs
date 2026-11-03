//! Imágenes a PDF: una página por imagen (o por fotograma en TIFF multipágina).

use std::path::{Path, PathBuf};
use std::str::FromStr;

use mupdf::pdf::{InsertImageOptions, PageImageSource, PdfDocument};
use mupdf::{Colorspace, Document, Image, ImageFormat, Matrix, Rect, Size};
use pliego_core::{Error, JobContext, Report, Result};
use serde::{Deserialize, Serialize};

use crate::util::{path_utf8, pdf_err, write_atomic};

/// Tamaño de página al crear un PDF desde imágenes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PageSizeSpec {
    /// La página tiene el tamaño de la imagen (según `dpi`) más los márgenes.
    Fit,
    A3,
    A4,
    A5,
    Letter,
    Legal,
    /// Medidas en puntos (1 pt = 1/72 pulgada).
    Custom {
        width_pt: f32,
        height_pt: f32,
    },
}

impl PageSizeSpec {
    /// Tamaño en puntos en vertical, o `None` si depende de la imagen.
    pub fn size_pt(self) -> Option<(f32, f32)> {
        const MM: f32 = 72.0 / 25.4;
        match self {
            PageSizeSpec::Fit => None,
            PageSizeSpec::A3 => Some((297.0 * MM, 420.0 * MM)),
            PageSizeSpec::A4 => Some((210.0 * MM, 297.0 * MM)),
            PageSizeSpec::A5 => Some((148.0 * MM, 210.0 * MM)),
            PageSizeSpec::Letter => Some((612.0, 792.0)),
            PageSizeSpec::Legal => Some((612.0, 1008.0)),
            PageSizeSpec::Custom { width_pt, height_pt } => Some((width_pt, height_pt)),
        }
    }
}

/// Acepta `fit`, `a3`, `a4`, `a5`, `letter`, `legal` o `<ancho>x<alto>mm` / `in` / `pt` (p. ej. `100x150mm`).
impl FromStr for PageSizeSpec {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let t = s.trim().to_lowercase();
        match t.as_str() {
            "fit" | "ajustar" => return Ok(Self::Fit),
            "a3" => return Ok(Self::A3),
            "a4" => return Ok(Self::A4),
            "a5" => return Ok(Self::A5),
            "letter" | "carta" => return Ok(Self::Letter),
            "legal" => return Ok(Self::Legal),
            _ => {}
        }
        let bad = || {
            Error::InvalidInput(format!(
                "tamaño de página no válido: {s:?} (usa a4, letter, 100x150mm...)"
            ))
        };
        let (body, factor) = if let Some(b) = t.strip_suffix("mm") {
            (b, 72.0 / 25.4)
        } else if let Some(b) = t.strip_suffix("in") {
            (b, 72.0)
        } else if let Some(b) = t.strip_suffix("pt") {
            (b, 1.0)
        } else {
            return Err(bad());
        };
        let (w, h) = body.split_once('x').ok_or_else(bad)?;
        let w: f32 = w.trim().parse().map_err(|_| bad())?;
        let h: f32 = h.trim().parse().map_err(|_| bad())?;
        if w <= 0.0 || h <= 0.0 {
            return Err(bad());
        }
        Ok(Self::Custom {
            width_pt: w * factor,
            height_pt: h * factor,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Orientation {
    /// Horizontal si la imagen es más ancha que alta.
    #[default]
    Auto,
    Portrait,
    Landscape,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ImagesToPdfOptions {
    pub page_size: PageSizeSpec,
    /// Margen en puntos en los cuatro lados.
    pub margin_pt: f32,
    /// Resolución con la que se interpreta el tamaño de las imágenes.
    pub dpi: f32,
    pub orientation: Orientation,
    /// Permite ampliar imágenes pequeñas hasta llenar la página (por defecto solo se reducen).
    pub upscale: bool,
}

impl Default for ImagesToPdfOptions {
    fn default() -> Self {
        Self {
            page_size: PageSizeSpec::Fit,
            margin_pt: 0.0,
            dpi: 150.0,
            orientation: Orientation::Auto,
            upscale: false,
        }
    }
}

/// Un fotograma listo para incrustar.
struct Frame {
    data: Vec<u8>,
    /// "jpeg" o "png": cómo está codificado `data`.
    hint: &'static str,
    width: u32,
    height: u32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Jpeg,
    Png,
    Webp,
    /// TIFF, GIF, BMP... los lee MuPDF.
    Mupdf,
}

fn sniff(bytes: &[u8], path: &Path) -> Result<Kind> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Ok(Kind::Jpeg)
    } else if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        Ok(Kind::Png)
    } else if bytes.len() > 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Ok(Kind::Webp)
    } else if bytes.starts_with(b"II*\0")
        || bytes.starts_with(b"MM\0*")
        || bytes.starts_with(b"GIF8")
        || bytes.starts_with(b"BM")
    {
        Ok(Kind::Mupdf)
    } else if bytes.len() > 12
        && &bytes[4..8] == b"ftyp"
        && matches!(&bytes[8..12], b"heic" | b"heix" | b"mif1" | b"avif")
    {
        Err(Error::Unsupported(format!(
            "{}: HEIC/AVIF aún no es compatible; conviértelo antes a JPG o PNG",
            path.display()
        )))
    } else {
        Err(Error::InvalidInput(format!(
            "{}: formato de imagen no reconocido (admitidos: JPG, PNG, TIFF, WebP, GIF, BMP)",
            path.display()
        )))
    }
}

fn png_bytes(pix: &mupdf::Pixmap) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    pix.write_to(&mut out, ImageFormat::PNG).map_err(pdf_err)?;
    Ok(out)
}

fn load_frames(path: &Path) -> Result<Vec<Frame>> {
    let bytes = std::fs::read(path).map_err(|e| Error::io_at(path, e))?;
    match sniff(&bytes, path)? {
        kind @ (Kind::Jpeg | Kind::Png) => {
            let img = Image::from_bytes(&bytes)
                .map_err(|e| Error::InvalidInput(format!("{}: imagen dañada ({e})", path.display())))?;
            Ok(vec![Frame {
                width: img.width(),
                height: img.height(),
                hint: if kind == Kind::Jpeg { "jpeg" } else { "png" },
                data: bytes,
            }])
        }
        Kind::Webp => {
            let img = image::load_from_memory_with_format(&bytes, image::ImageFormat::WebP)
                .map_err(|e| Error::InvalidInput(format!("{}: imagen WebP dañada ({e})", path.display())))?
                .to_rgba8();
            let (width, height) = img.dimensions();
            let mut data = Vec::new();
            img.write_to(&mut std::io::Cursor::new(&mut data), image::ImageFormat::Png)
                .map_err(|e| Error::Other(format!("no se pudo recodificar {}: {e}", path.display())))?;
            Ok(vec![Frame {
                data,
                hint: "png",
                width,
                height,
            }])
        }
        Kind::Mupdf => mupdf_frames(path),
    }
}

/// TIFF (también multipágina), GIF y BMP: se renderiza cada fotograma a su tamaño de píxeles nativo.
fn mupdf_frames(path: &Path) -> Result<Vec<Frame>> {
    let broken = |e: mupdf::Error| Error::InvalidInput(format!("{}: imagen dañada ({e})", path.display()));
    let path_str = path_utf8(path)?;
    // Tamaño nativo del primer fotograma: sirve para deducir la escala píxeles/puntos.
    let first = Image::from_file(path_str).map_err(broken)?;
    let doc = Document::open(path_str).map_err(broken)?;
    let count = doc.page_count().map_err(pdf_err)?;
    let mut frames = Vec::new();
    let mut scale = None;
    for i in 0..count {
        let page = doc.load_page(i).map_err(pdf_err)?;
        let bounds = page.bounds().map_err(pdf_err)?;
        let s = *scale.get_or_insert_with(|| first.width() as f32 / bounds.width().max(0.01));
        let pix = page
            .to_pixmap(&Matrix::new_scale(s, s), &Colorspace::device_rgb(), false, false)
            .map_err(pdf_err)?;
        frames.push(Frame {
            width: pix.width(),
            height: pix.height(),
            hint: "png",
            data: png_bytes(&pix)?,
        });
    }
    Ok(frames)
}

/// Crea un PDF con una página por imagen. Los JPEG se incrustan sin recomprimir.
///
/// `inputs` admite JPG, PNG, TIFF (multipágina), WebP, GIF y BMP; el formato se detecta por contenido.
/// HEIC no está soportado todavía y devuelve un error claro.
pub fn images_to_pdf(
    inputs: &[PathBuf],
    output: &Path,
    options: &ImagesToPdfOptions,
    ctx: &JobContext,
) -> Result<Report> {
    if inputs.is_empty() {
        return Err(Error::InvalidInput("no se indicó ninguna imagen".into()));
    }
    if !(options.dpi.is_finite() && options.dpi > 0.0) || !options.margin_pt.is_finite() || options.margin_pt < 0.0 {
        return Err(Error::InvalidInput("resolución o margen no válidos".into()));
    }
    let mut pdf = PdfDocument::new();
    let mut pages = 0u32;
    for (n, path) in inputs.iter().enumerate() {
        ctx.step(n as u64, inputs.len() as u64, &path.display().to_string())?;
        for frame in load_frames(path)? {
            add_page(&mut pdf, &frame, options)?;
            pages += 1;
        }
    }
    let mut buf = Vec::new();
    pdf.write_to(&mut buf).map_err(pdf_err)?;
    write_atomic(output, &buf)?;
    ctx.step(inputs.len() as u64, inputs.len() as u64, "listo")?;
    Ok(Report::with_output(output, format!("{pages} página(s) creadas")))
}

fn add_page(pdf: &mut PdfDocument, frame: &Frame, o: &ImagesToPdfOptions) -> Result<()> {
    let to_pt = 72.0 / o.dpi;
    let (img_w, img_h) = (frame.width as f32 * to_pt, frame.height as f32 * to_pt);
    let m = o.margin_pt;

    let (page_w, page_h) = match o.page_size.size_pt() {
        None => (img_w + 2.0 * m, img_h + 2.0 * m),
        Some((w, h)) => {
            let landscape = match o.orientation {
                Orientation::Auto => frame.width > frame.height,
                Orientation::Landscape => true,
                Orientation::Portrait => false,
            };
            let (short, long) = (w.min(h), w.max(h));
            if landscape { (long, short) } else { (short, long) }
        }
    };
    let (avail_w, avail_h) = ((page_w - 2.0 * m).max(1.0), (page_h - 2.0 * m).max(1.0));
    let mut scale = (avail_w / img_w).min(avail_h / img_h);
    if !o.upscale {
        scale = scale.min(1.0);
    }
    let (w, h) = (img_w * scale, img_h * scale);
    let x0 = (page_w - w) / 2.0;
    let y0 = (page_h - h) / 2.0;

    let mut page = pdf
        .new_page(Size {
            width: page_w,
            height: page_h,
        })
        .map_err(pdf_err)?;
    let rect = Rect {
        x0,
        y0,
        x1: x0 + w,
        y1: y0 + h,
    };
    page.insert_image(
        pdf,
        rect,
        PageImageSource::Bytes {
            data: &frame.data,
            format_hint: Some(frame.hint),
        },
        InsertImageOptions::default(),
    )
    .map_err(pdf_err)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sizes() {
        assert_eq!("A4".parse::<PageSizeSpec>().unwrap(), PageSizeSpec::A4);
        assert_eq!("fit".parse::<PageSizeSpec>().unwrap(), PageSizeSpec::Fit);
        let PageSizeSpec::Custom { width_pt, height_pt } = "100x150mm".parse().unwrap() else {
            panic!()
        };
        assert!((width_pt - 283.46).abs() < 0.1 && (height_pt - 425.2).abs() < 0.1);
        assert!("10x".parse::<PageSizeSpec>().is_err());
        assert!("grande".parse::<PageSizeSpec>().is_err());
    }
}
