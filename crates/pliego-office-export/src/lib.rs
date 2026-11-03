//! Exportar PDF a Word (DOCX), Excel (XLSX) y PowerPoint (PPTX) conservando la maquetación tanto como se
//! pueda (flujo W14).
//!
//! El análisis parte del texto estructurado de W09 (`pliego-convert`): líneas con fuente, tamaño y caja. De ahí
//! reconstruye columnas, párrafos, listas, tablas (por alineación), encabezados y pies, e imágenes, y escribe
//! OOXML propio sin dependencias de Office. El algoritmo es original; `pdf2docx` solo sirvió de referencia
//! conceptual.
//!
//! Biblioteca primero: funciones sobre rutas, sin estado global. Las páginas se numeran desde 0 en la API.

mod analyze;
mod docx;
mod extract;
mod lines;
mod model;
mod paragraphs;
mod pptx;
mod tables;
mod util;
mod xlsx;

use std::path::Path;

use pliego_core::{Error, JobContext, PageRange, Report, Result};
use serde::{Deserialize, Serialize};

pub use model::{
    Align, Cell, ColumnSpec, Element, ImageData, Layout, ListItem, ListKind, PageLayout, Paragraph, Run, Section, Table,
};

use analyze::AnalyzeOptions;

/// Qué hacer cuando el PDF tiene páginas escaneadas (sin texto).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ScannedPolicy {
    /// Exportar esas páginas como imagen y avisar en el informe (valor por defecto).
    #[default]
    Warn,
    /// No exportar: devolver un error que sugiere aplicar OCR (W06) antes.
    Fail,
}

/// Opciones comunes a los tres formatos.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CommonOptions {
    /// Contraseña del PDF, si la tiene.
    pub password: Option<String>,
    /// Páginas (desde 1): `"all"`, `"1-3,5"`, `"last"`.
    pub pages: String,
    pub scanned: ScannedPolicy,
}

impl Default for CommonOptions {
    fn default() -> Self {
        Self {
            password: None,
            pages: "all".into(),
            scanned: ScannedPolicy::Warn,
        }
    }
}

impl CommonOptions {
    fn page_range(&self) -> Result<PageRange> {
        self.pages
            .parse()
            .map_err(|e| Error::InvalidInput(format!("rango de páginas no válido ({:?}): {e}", self.pages)))
    }
}

/// Opciones de [`to_docx`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DocxOptions {
    #[serde(flatten)]
    pub common: CommonOptions,
    /// Resolución (ppp) de las imágenes incrustadas.
    pub image_dpi: f32,
    /// Separar los encabezados y pies repetidos del cuerpo (con número de página dinámico).
    pub headers_footers: bool,
}

impl Default for DocxOptions {
    fn default() -> Self {
        Self {
            common: CommonOptions::default(),
            image_dpi: 150.0,
            headers_footers: true,
        }
    }
}

fn check_dpi(dpi: f32) -> Result<()> {
    if dpi.is_finite() && (36.0..=600.0).contains(&dpi) {
        Ok(())
    } else {
        Err(Error::InvalidInput(
            "la resolución de imagen debe estar entre 36 y 600 ppp".into(),
        ))
    }
}

fn title_of(input: &Path) -> String {
    input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Documento")
        .to_string()
}

/// Falla con un mensaje útil si la política lo pide y hay páginas escaneadas.
fn enforce_scanned(common: &CommonOptions, layout: &Layout) -> Result<()> {
    if common.scanned == ScannedPolicy::Fail
        && let Some(w) = layout.warnings.first()
    {
        return Err(Error::Unsupported(w.clone()));
    }
    Ok(())
}

/// Páginas (desde 1) del PDF que son solo una imagen sin texto: conviene aplicar OCR antes de exportar.
pub fn scanned_pages(input: &Path, password: Option<&str>) -> Result<Vec<u32>> {
    analyze::scanned_pages(input, password)
}

/// Exporta un PDF a Word.
pub fn to_docx(input: &Path, out: &Path, options: &DocxOptions, ctx: &JobContext) -> Result<Report> {
    check_dpi(options.image_dpi)?;
    util::ensure_not_same(input, out)?;
    let opts = AnalyzeOptions {
        headers: options.headers_footers,
        images: true,
        image_dpi: options.image_dpi,
    };
    let layout = analyze::analyze(
        input,
        options.common.password.as_deref(),
        &options.common.page_range()?,
        &opts,
        ctx,
    )?;
    enforce_scanned(&options.common, &layout)?;
    let bytes = docx::write_docx(&layout, &title_of(input))?;
    util::write_atomic(out, &bytes)?;
    let tables: usize = layout.pages.iter().map(|p| p.tables().count()).sum();
    Ok(Report {
        outputs: vec![out.to_path_buf()],
        summary: format!(
            "{} página(s) exportadas a Word; {tables} tabla(s) detectada(s)",
            layout.pages.len()
        ),
        warnings: layout.warnings,
    })
}

/// Maquetación reconstruida de las páginas indicadas (útil para pruebas y vistas previas).
pub fn analyze_layout(input: &Path, common: &CommonOptions, ctx: &JobContext) -> Result<Layout> {
    let opts = AnalyzeOptions {
        headers: true,
        images: false,
        image_dpi: 150.0,
    };
    analyze::analyze(input, common.password.as_deref(), &common.page_range()?, &opts, ctx)
}

/// Opciones de [`to_xlsx`].
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct XlsxOptions {
    #[serde(flatten)]
    pub common: CommonOptions,
    /// Una hoja por página (con todas sus tablas) en lugar de una hoja por tabla.
    pub sheet_per_page: bool,
}

/// Exporta las tablas de un PDF a Excel (una hoja por tabla o por página). Si no se detecta ninguna tabla,
/// cada página va a una hoja con su texto, una línea por fila, y se avisa.
pub fn to_xlsx(input: &Path, out: &Path, options: &XlsxOptions, ctx: &JobContext) -> Result<Report> {
    util::ensure_not_same(input, out)?;
    let opts = AnalyzeOptions {
        headers: false,
        images: false,
        image_dpi: 150.0,
    };
    let layout = analyze::analyze(
        input,
        options.common.password.as_deref(),
        &options.common.page_range()?,
        &opts,
        ctx,
    )?;
    enforce_scanned(&options.common, &layout)?;
    let (bytes, sheets, any_table) = xlsx::write_xlsx(&layout, options.sheet_per_page)?;
    util::write_atomic(out, &bytes)?;
    let mut warnings = layout.warnings;
    if !any_table {
        warnings.push("No se detectó ninguna tabla: se exporta el texto de cada página, una línea por fila.".into());
    }
    Ok(Report {
        outputs: vec![out.to_path_buf()],
        summary: format!("{sheets} hoja(s) exportadas a Excel"),
        warnings,
    })
}

/// Opciones de [`to_pptx`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PptxOptions {
    #[serde(flatten)]
    pub common: CommonOptions,
    /// Resolución (ppp) de las imágenes incrustadas.
    pub image_dpi: f32,
}

impl Default for PptxOptions {
    fn default() -> Self {
        Self {
            common: CommonOptions::default(),
            image_dpi: 150.0,
        }
    }
}

/// Exporta un PDF a PowerPoint: una diapositiva por página con texto editable, tablas e imágenes.
pub fn to_pptx(input: &Path, out: &Path, options: &PptxOptions, ctx: &JobContext) -> Result<Report> {
    check_dpi(options.image_dpi)?;
    util::ensure_not_same(input, out)?;
    let opts = AnalyzeOptions {
        headers: false,
        images: true,
        image_dpi: options.image_dpi,
    };
    let layout = analyze::analyze(
        input,
        options.common.password.as_deref(),
        &options.common.page_range()?,
        &opts,
        ctx,
    )?;
    enforce_scanned(&options.common, &layout)?;
    let bytes = pptx::write_pptx(&layout)?;
    util::write_atomic(out, &bytes)?;
    Ok(Report {
        outputs: vec![out.to_path_buf()],
        summary: format!("{} diapositiva(s) exportadas a PowerPoint", layout.pages.len()),
        warnings: layout.warnings,
    })
}
