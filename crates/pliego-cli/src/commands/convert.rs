use std::path::PathBuf;

use anyhow::{Context, bail};
use clap::{Args, ValueEnum};
use pliego_convert::{
    ExportImageFormat, ExtractImagesOptions, ImagesToPdfOptions, Orientation, PageSizeSpec, PdfToImagesOptions,
    TextExportOptions, TextToPdfOptions, cbz_to_pdf, epub_to_pdf, extract_images, html_to_pdf, images_to_pdf,
    markdown_to_pdf, office_to_pdf, pdf_to_html, pdf_to_images, pdf_to_text, structured_text_pages, text_to_pdf,
    xps_to_pdf,
};
use pliego_core::{JobContext, PageRange, Report};

use super::view::OpenArgs;

#[derive(Clone, Copy, ValueEnum)]
pub enum OrientationArg {
    Auto,
    Portrait,
    Landscape,
}

#[derive(Args)]
pub struct CreateArgs {
    /// Archivos de entrada: imágenes (JPG, PNG, TIFF, WebP, GIF, BMP) o un único .txt, .md, .html o documento de Office (Word, Excel, PowerPoint, OpenDocument).
    #[arg(required = true)]
    pub inputs: Vec<PathBuf>,
    /// PDF de salida.
    #[arg(short, long)]
    pub output: PathBuf,
    /// Tamaño de página: fit, a3, a4, a5, letter, legal o 100x150mm.
    #[arg(long, default_value = "fit")]
    pub page_size: PageSizeSpec,
    /// Margen en milímetros.
    #[arg(long, default_value_t = 0.0)]
    pub margin_mm: f32,
    /// Resolución con la que se interpreta el tamaño de las imágenes.
    #[arg(long, default_value_t = 150.0)]
    pub dpi: f32,
    #[arg(long, value_enum, default_value = "auto")]
    pub orientation: OrientationArg,
    /// Amplía las imágenes pequeñas hasta llenar la página.
    #[arg(long)]
    pub upscale: bool,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum ExportTarget {
    Png,
    Jpg,
    Tiff,
    Txt,
    Html,
    /// Imágenes incrustadas, en su formato original.
    Images,
    /// Texto estructurado (bloques, líneas, fuentes) en JSON.
    Json,
}

#[derive(Args)]
pub struct ExportArgs {
    #[command(flatten)]
    pub open: OpenArgs,
    /// Formato de salida.
    #[arg(long, value_enum)]
    pub to: ExportTarget,
    /// Carpeta de salida (imágenes) o archivo (texto).
    #[arg(short, long)]
    pub output: PathBuf,
    /// Páginas (desde 1): "1-3,5", "all", "last".
    #[arg(short, long, default_value = "all")]
    pub pages: PageRange,
    /// Resolución en ppp para imágenes.
    #[arg(long, default_value_t = 150.0)]
    pub dpi: f32,
    /// Calidad JPEG (1-100).
    #[arg(long, default_value_t = 90)]
    pub quality: u8,
}

fn show(report: &Report, json: bool) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
    } else {
        for o in &report.outputs {
            println!("{}", o.display());
        }
        for w in &report.warnings {
            eprintln!("aviso: {w}");
        }
    }
    Ok(())
}

pub fn create(a: CreateArgs, json: bool) -> anyhow::Result<()> {
    if a.inputs.is_empty() {
        bail!("indica al menos un archivo de entrada");
    }
    let kind = a.inputs[0]
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase)
        .unwrap_or_default();
    const OFFICE: &[&str] = &[
        "doc", "docx", "docm", "dot", "dotx", "rtf", "odt", "xls", "xlsx", "xlsm", "xlsb", "csv", "ods", "ppt", "pptx",
        "pptm", "pps", "ppsx", "odp",
    ];
    if OFFICE.contains(&kind.as_str()) {
        if a.inputs.len() != 1 {
            bail!("los documentos de Office se convierten de uno en uno");
        }
        let report = office_to_pdf(&a.inputs[0], &a.output, &JobContext::silent())?;
        return show(&report, json);
    }
    if kind == "xps" || kind == "oxps" {
        if a.inputs.len() != 1 {
            bail!("los documentos XPS se convierten de uno en uno");
        }
        let report = xps_to_pdf(&a.inputs[0], &a.output, &JobContext::silent())?;
        return show(&report, json);
    }
    let doc_fn: Option<fn(&_, &_, &TextToPdfOptions, &JobContext) -> pliego_core::Result<Report>> = match kind.as_str()
    {
        "txt" => Some(text_to_pdf),
        "md" | "markdown" => Some(markdown_to_pdf),
        "html" | "htm" => Some(html_to_pdf),
        "epub" => Some(epub_to_pdf),
        _ => None,
    };
    if let Some(convert) = doc_fn {
        if a.inputs.len() != 1 {
            bail!("los archivos de texto, Markdown, HTML y EPUB se convierten de uno en uno");
        }
        let opts = TextToPdfOptions {
            page_size: if a.page_size == PageSizeSpec::Fit {
                PageSizeSpec::A4
            } else {
                a.page_size
            },
            margin_pt: if a.margin_mm > 0.0 {
                a.margin_mm * 72.0 / 25.4
            } else {
                TextToPdfOptions::default().margin_pt
            },
            ..Default::default()
        };
        let report = convert(&a.inputs[0], &a.output, &opts, &JobContext::silent())?;
        return show(&report, json);
    }
    let opts = ImagesToPdfOptions {
        page_size: a.page_size,
        margin_pt: a.margin_mm * 72.0 / 25.4,
        dpi: a.dpi,
        orientation: match a.orientation {
            OrientationArg::Auto => Orientation::Auto,
            OrientationArg::Portrait => Orientation::Portrait,
            OrientationArg::Landscape => Orientation::Landscape,
        },
        upscale: a.upscale,
    };
    if kind == "cbz" {
        if a.inputs.len() != 1 {
            bail!("los cómics CBZ se convierten de uno en uno");
        }
        let report = cbz_to_pdf(&a.inputs[0], &a.output, &opts, &JobContext::silent())?;
        return show(&report, json);
    }
    let report = images_to_pdf(&a.inputs, &a.output, &opts, &JobContext::silent())?;
    show(&report, json)
}

fn structured_json(a: &ExportArgs) -> pliego_core::Result<Report> {
    let pages = structured_text_pages(&a.open.input, a.open.password.as_deref(), &a.pages)?;
    let json = serde_json::to_vec_pretty(&pages).map_err(|e| pliego_core::Error::Other(e.to_string()))?;
    if let Some(dir) = a.output.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| pliego_core::Error::io_at(dir, e))?;
    }
    std::fs::write(&a.output, json).map_err(|e| pliego_core::Error::io_at(&a.output, e))?;
    Ok(Report::with_output(
        &a.output,
        format!("{} página(s) en JSON", pages.len()),
    ))
}

pub fn export(a: ExportArgs, json: bool) -> anyhow::Result<()> {
    let ctx = JobContext::silent();
    let report = match a.to {
        ExportTarget::Txt => pdf_to_text(
            &a.open.input,
            a.open.password.as_deref(),
            &a.pages,
            &a.output,
            &TextExportOptions::default(),
            &ctx,
        ),
        ExportTarget::Html => pdf_to_html(&a.open.input, a.open.password.as_deref(), &a.pages, &a.output, &ctx),
        ExportTarget::Images => extract_images(
            &a.open.input,
            a.open.password.as_deref(),
            &a.pages,
            &a.output,
            &ExtractImagesOptions::default(),
            &ctx,
        ),
        ExportTarget::Json => structured_json(&a),
        ExportTarget::Png | ExportTarget::Jpg | ExportTarget::Tiff => {
            let format = match a.to {
                ExportTarget::Png => ExportImageFormat::Png,
                ExportTarget::Jpg => ExportImageFormat::Jpeg,
                _ => ExportImageFormat::Tiff,
            };
            let opts = PdfToImagesOptions {
                format,
                dpi: a.dpi,
                jpeg_quality: a.quality,
            };
            pdf_to_images(
                &a.open.input,
                a.open.password.as_deref(),
                &a.pages,
                &a.output,
                &opts,
                &ctx,
            )
        }
    }
    .with_context(|| format!("no se pudo exportar {}", a.open.input.display()))?;
    show(&report, json)
}
