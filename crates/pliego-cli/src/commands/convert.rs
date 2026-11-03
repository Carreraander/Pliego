use std::path::PathBuf;

use anyhow::{Context, bail};
use clap::{Args, ValueEnum};
use pliego_convert::{
    ExportImageFormat, ImagesToPdfOptions, Orientation, PageSizeSpec, PdfToImagesOptions, TextExportOptions,
    images_to_pdf, pdf_to_images, pdf_to_text,
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
    /// Archivos de entrada (imágenes: JPG, PNG, TIFF, WebP, GIF, BMP).
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
    let report = images_to_pdf(&a.inputs, &a.output, &opts, &JobContext::silent())?;
    show(&report, json)
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
