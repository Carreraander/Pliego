use std::path::PathBuf;

use anyhow::Context;
use clap::{Args, Subcommand};
use pliego_core::{JobContext, PageRange, Report};
use pliego_office_export::{CommonOptions, DocxOptions, PptxOptions, ScannedPolicy, XlsxOptions};

use super::view::OpenArgs;

#[derive(Args)]
pub struct OfficeArgs {
    #[command(flatten)]
    pub open: OpenArgs,
    /// Archivo de salida.
    #[arg(short, long)]
    pub output: PathBuf,
    /// Páginas (desde 1): "1-3,5", "all", "last".
    #[arg(short, long, default_value = "all")]
    pub pages: PageRange,
    /// Falla si hay páginas escaneadas (sin texto) en lugar de exportarlas como imagen.
    #[arg(long)]
    pub fail_on_scanned: bool,
}

#[derive(Args)]
pub struct DocxArgs {
    #[command(flatten)]
    pub common: OfficeArgs,
    /// Resolución (ppp) de las imágenes incrustadas.
    #[arg(long, default_value_t = 150.0)]
    pub dpi: f32,
    /// No separar encabezados y pies repetidos del cuerpo.
    #[arg(long)]
    pub no_headers: bool,
}

#[derive(Args)]
pub struct XlsxArgs {
    #[command(flatten)]
    pub common: OfficeArgs,
    /// Una hoja por página (con todas sus tablas) en lugar de una por tabla.
    #[arg(long)]
    pub sheet_per_page: bool,
}

#[derive(Args)]
pub struct PptxArgs {
    #[command(flatten)]
    pub common: OfficeArgs,
    /// Resolución (ppp) de las imágenes incrustadas.
    #[arg(long, default_value_t = 150.0)]
    pub dpi: f32,
}

#[derive(Subcommand)]
pub enum OfficeCommand {
    /// Exporta a Word (.docx): párrafos, columnas, listas, tablas, imágenes, encabezados y pies.
    Docx(DocxArgs),
    /// Exporta las tablas a Excel (.xlsx): una hoja por tabla o por página.
    Xlsx(XlsxArgs),
    /// Exporta a PowerPoint (.pptx): una diapositiva por página con texto editable.
    Pptx(PptxArgs),
}

fn common(a: &OfficeArgs) -> CommonOptions {
    CommonOptions {
        password: a.open.password.clone(),
        pages: a.pages.to_string(),
        scanned: if a.fail_on_scanned {
            ScannedPolicy::Fail
        } else {
            ScannedPolicy::Warn
        },
    }
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

pub fn run(cmd: OfficeCommand, json: bool) -> anyhow::Result<()> {
    let ctx = JobContext::silent();
    let report = match cmd {
        OfficeCommand::Docx(a) => {
            let opts = DocxOptions {
                common: common(&a.common),
                image_dpi: a.dpi,
                headers_footers: !a.no_headers,
            };
            pliego_office_export::to_docx(&a.common.open.input, &a.common.output, &opts, &ctx)
        }
        OfficeCommand::Xlsx(a) => {
            let opts = XlsxOptions {
                common: common(&a.common),
                sheet_per_page: a.sheet_per_page,
            };
            pliego_office_export::to_xlsx(&a.common.open.input, &a.common.output, &opts, &ctx)
        }
        OfficeCommand::Pptx(a) => {
            let opts = PptxOptions {
                common: common(&a.common),
                image_dpi: a.dpi,
            };
            pliego_office_export::to_pptx(&a.common.open.input, &a.common.output, &opts, &ctx)
        }
    }
    .with_context(|| "no se pudo exportar")?;
    show(&report, json)
}
