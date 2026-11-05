//! Texto plano, Markdown y HTML a PDF.
//!
//! Texto plano y Markdown los maqueta Pliego (`typeset`, sin MuPDF). El HTML sigue con el motor HTML de MuPDF
//! (multiplataforma y sin navegador, pero con CSS limitado —sin flex ni grid— y sin JavaScript) hasta que MuPDF salga
//! del binario (W41). Para páginas web reales hay que usar la impresión a PDF de WebView2
//! ([`HtmlBackend::WebView2`]), que corre en la app de escritorio y no desde esta biblioteca.

use std::path::Path;

use pliego_core::{Error, JobContext, Report, Result};
use pliego_pdf::{Document, Matrix, PdfWriter, Rect};

use crate::typeset::{self, PageSpec};
use serde::{Deserialize, Serialize};

use crate::images::PageSizeSpec;
use crate::util::{ensure_not_same, write_atomic};

/// Quién renderiza el HTML.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HtmlBackend {
    /// Motor HTML de MuPDF: en cualquier sistema, sin JS, CSS básico. Es el único que implementa esta biblioteca.
    #[default]
    Mupdf,
    /// Impresión a PDF de WebView2 (solo Windows, la ejecuta la app). Aquí devuelve `Unsupported`.
    WebView2,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextToPdfOptions {
    /// `Fit` equivale a A4.
    pub page_size: PageSizeSpec,
    /// Margen en puntos en los cuatro lados.
    pub margin_pt: f32,
    /// Tamaño de letra en puntos.
    pub font_size_pt: f32,
    pub backend: HtmlBackend,
}

impl Default for TextToPdfOptions {
    fn default() -> Self {
        Self {
            page_size: PageSizeSpec::A4,
            margin_pt: 54.0,
            font_size_pt: 11.0,
            backend: HtmlBackend::Mupdf,
        }
    }
}

fn read_utf8(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path).map_err(|e| Error::io_at(path, e))?;
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes);
    String::from_utf8(bytes.to_vec())
        .map_err(|_| Error::InvalidInput(format!("{}: el archivo no es texto UTF-8", path.display())))
}

/// Comprueba las opciones y devuelve las medidas de la página.
fn page_spec(options: &TextToPdfOptions) -> Result<PageSpec> {
    if options.backend == HtmlBackend::WebView2 {
        return Err(Error::Unsupported(
            "la impresión con WebView2 solo está disponible desde la app de escritorio en Windows".into(),
        ));
    }
    let (width, height) = options.page_size.size_pt().unwrap_or((595.28, 841.89));
    let m = options.margin_pt;
    if !options.font_size_pt.is_finite() || options.font_size_pt <= 0.0 || !m.is_finite() || m < 0.0 {
        return Err(Error::InvalidInput("tamaño de letra o margen no válidos".into()));
    }
    if width - 2.0 * m < 36.0 || height - 2.0 * m < 36.0 {
        return Err(Error::InvalidInput(
            "los márgenes dejan una página sin espacio útil".into(),
        ));
    }
    Ok(PageSpec {
        width,
        height,
        margin: m,
        font_size: options.font_size_pt,
    })
}

/// Guarda el resultado del maquetador propio y arma el informe.
fn finish_typeset(t: typeset::Typeset, output: &Path, ctx: &JobContext) -> Result<Report> {
    ctx.step(1, 2, "guardando")?;
    let bytes = typeset::save_bytes(&t.pdf)?;
    write_atomic(output, &bytes)?;
    ctx.step(2, 2, "listo")?;
    let mut report = Report::with_output(output, format!("{} página(s) creadas", t.pages));
    if t.substituted > 0 {
        report.warnings.push(format!(
            "{} carácter(es) fuera de Windows-1252 (cirílico, griego, CJK, emojis…) se sustituyeron por «?»",
            t.substituted
        ));
    }
    Ok(report)
}

/// Convierte una cadena HTML en un PDF con el motor HTML de MuPDF.
pub fn html_string_to_pdf(html: &str, output: &Path, options: &TextToPdfOptions, ctx: &JobContext) -> Result<Report> {
    let spec = page_spec(options)?;
    let (w, h, m) = (spec.width, spec.height, spec.margin);
    ctx.step(0, 2, "maquetando")?;
    let mut doc = Document::from_bytes(html.as_bytes(), "text/html")?;
    doc.layout(w - 2.0 * m, h - 2.0 * m, options.font_size_pt)?;
    let pages = doc.page_count()?;

    // Se escribe con `PdfWriter` (página completa y contenido desplazado por el margen).
    // `Document::convert_to_pdf` se descartó: libera memoria dos veces en documentos de varias páginas.
    let tmp = tempfile::tempdir().map_err(Error::from)?;
    let tmp_pdf = tmp.path().join("salida.pdf");
    {
        let mut writer = PdfWriter::new(&tmp_pdf, "")?;
        for i in 0..pages {
            let page = doc.load_page(i)?;
            writer.write_page(
                Rect {
                    x0: 0.0,
                    y0: 0.0,
                    x1: w,
                    y1: h,
                },
                &page,
                &Matrix::new_translate(m, m),
            )?;
        }
        // Al salir de este bloque se cierra el escritor y el PDF queda completo.
    }
    ctx.step(1, 2, "guardando")?;
    let bytes = std::fs::read(&tmp_pdf).map_err(|e| Error::io_at(&tmp_pdf, e))?;
    write_atomic(output, &bytes)?;
    ctx.step(2, 2, "listo")?;
    Ok(Report::with_output(output, format!("{pages} página(s) creadas")))
}

/// Texto plano UTF-8 a PDF (fuente monoespaciada, ajuste de línea). Lo maqueta Pliego, sin MuPDF.
pub fn text_to_pdf(input: &Path, output: &Path, options: &TextToPdfOptions, ctx: &JobContext) -> Result<Report> {
    ensure_not_same(input, output)?;
    let spec = page_spec(options)?;
    let text = read_utf8(input)?;
    ctx.step(0, 2, "maquetando")?;
    finish_typeset(typeset::plain(&text, &spec)?, output, ctx)
}

/// Markdown (CommonMark con tablas, tachado y listas de tareas) a PDF. Lo maqueta Pliego, sin MuPDF.
pub fn markdown_to_pdf(input: &Path, output: &Path, options: &TextToPdfOptions, ctx: &JobContext) -> Result<Report> {
    ensure_not_same(input, output)?;
    let spec = page_spec(options)?;
    let md = read_utf8(input)?;
    ctx.step(0, 2, "maquetando")?;
    finish_typeset(typeset::markdown(&md, &spec)?, output, ctx)
}

/// Archivo HTML (UTF-8) a PDF. Con [`HtmlBackend::Mupdf`] no se ejecuta JavaScript ni se admite CSS moderno.
pub fn html_to_pdf(input: &Path, output: &Path, options: &TextToPdfOptions, ctx: &JobContext) -> Result<Report> {
    ensure_not_same(input, output)?;
    let html = read_utf8(input)?;
    html_string_to_pdf(&html, output, options, ctx)
}
