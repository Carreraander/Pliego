//! Texto plano, Markdown y HTML a PDF con el motor HTML de MuPDF.
//!
//! El motor HTML de MuPDF es multiplataforma y no necesita navegador, pero su CSS es limitado (sin flex ni grid)
//! y no ejecuta JavaScript. Para páginas web reales hay que usar la impresión a PDF de WebView2
//! ([`HtmlBackend::WebView2`]), que corre en la app de escritorio y no desde esta biblioteca.

use std::path::Path;

use mupdf::{Document, DocumentWriter, Matrix, Rect};
use pliego_core::{Error, JobContext, Report, Result};
use pulldown_cmark::{Options, Parser, html};
use serde::{Deserialize, Serialize};

use crate::images::PageSizeSpec;
use crate::util::{ensure_not_same, path_utf8, pdf_err, write_atomic};

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

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn read_utf8(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path).map_err(|e| Error::io_at(path, e))?;
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes);
    String::from_utf8(bytes.to_vec())
        .map_err(|_| Error::InvalidInput(format!("{}: el archivo no es texto UTF-8", path.display())))
}

const MARKDOWN_CSS: &str = "body{margin:0;font-family:serif;line-height:1.35}h1{font-size:1.8em}h2{font-size:1.45em}\
h3{font-size:1.2em}code,pre{font-family:monospace}pre{white-space:pre-wrap}\
blockquote{margin-left:1.2em;font-style:italic}table{border-collapse:collapse}td,th{border:1px solid #888;padding:2px 6px}";

/// Convierte una cadena HTML en un PDF. Es la función común de las demás.
pub fn html_string_to_pdf(html: &str, output: &Path, options: &TextToPdfOptions, ctx: &JobContext) -> Result<Report> {
    if options.backend == HtmlBackend::WebView2 {
        return Err(Error::Unsupported(
            "la impresión con WebView2 solo está disponible desde la app de escritorio en Windows".into(),
        ));
    }
    let (w, h) = options.page_size.size_pt().unwrap_or((595.28, 841.89));
    let m = options.margin_pt;
    if !(options.font_size_pt.is_finite() && options.font_size_pt > 0.0) || !m.is_finite() || m < 0.0 {
        return Err(Error::InvalidInput("tamaño de letra o margen no válidos".into()));
    }
    if w - 2.0 * m < 36.0 || h - 2.0 * m < 36.0 {
        return Err(Error::InvalidInput(
            "los márgenes dejan una página sin espacio útil".into(),
        ));
    }
    ctx.step(0, 2, "maquetando")?;
    let mut doc = Document::from_bytes(html.as_bytes(), "text/html").map_err(pdf_err)?;
    doc.layout(w - 2.0 * m, h - 2.0 * m, options.font_size_pt)
        .map_err(pdf_err)?;
    let pages = doc.page_count().map_err(pdf_err)?;

    // Se escribe con el `DocumentWriter` de MuPDF (página completa y contenido desplazado por el margen).
    // `Document::convert_to_pdf` se descartó: libera memoria dos veces en documentos de varias páginas.
    let tmp = tempfile::tempdir().map_err(Error::from)?;
    let tmp_pdf = tmp.path().join("salida.pdf");
    {
        let mut writer = DocumentWriter::new(path_utf8(&tmp_pdf)?, "pdf", "").map_err(pdf_err)?;
        for i in 0..pages {
            let page = doc.load_page(i).map_err(pdf_err)?;
            let dev = writer
                .begin_page(Rect {
                    x0: 0.0,
                    y0: 0.0,
                    x1: w,
                    y1: h,
                })
                .map_err(pdf_err)?;
            page.run(&dev, &Matrix::new_translate(m, m)).map_err(pdf_err)?;
            writer.end_page(dev).map_err(pdf_err)?;
        }
        // Al salir de este bloque se cierra el escritor y el PDF queda completo.
    }
    ctx.step(1, 2, "guardando")?;
    let bytes = std::fs::read(&tmp_pdf).map_err(|e| Error::io_at(&tmp_pdf, e))?;
    write_atomic(output, &bytes)?;
    ctx.step(2, 2, "listo")?;
    Ok(Report::with_output(output, format!("{pages} página(s) creadas")))
}

/// Texto plano UTF-8 a PDF (fuente monoespaciada, ajuste de línea).
pub fn text_to_pdf(input: &Path, output: &Path, options: &TextToPdfOptions, ctx: &JobContext) -> Result<Report> {
    ensure_not_same(input, output)?;
    let text = read_utf8(input)?;
    let html = format!(
        "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><style>body{{margin:0}}pre{{font-family:monospace;white-space:pre-wrap}}</style></head><body><pre>{}</pre></body></html>",
        escape(&text)
    );
    html_string_to_pdf(&html, output, options, ctx)
}

/// Markdown (CommonMark con tablas, tachado y listas de tareas) a PDF.
pub fn markdown_to_pdf(input: &Path, output: &Path, options: &TextToPdfOptions, ctx: &JobContext) -> Result<Report> {
    ensure_not_same(input, output)?;
    let md = read_utf8(input)?;
    let parser = Parser::new_ext(
        &md,
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS,
    );
    let mut body = String::new();
    html::push_html(&mut body, parser);
    let html = format!(
        "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><style>{MARKDOWN_CSS}</style></head><body>{body}</body></html>"
    );
    html_string_to_pdf(&html, output, options, ctx)
}

/// Archivo HTML (UTF-8) a PDF. Con [`HtmlBackend::Mupdf`] no se ejecuta JavaScript ni se admite CSS moderno.
pub fn html_to_pdf(input: &Path, output: &Path, options: &TextToPdfOptions, ctx: &JobContext) -> Result<Report> {
    ensure_not_same(input, output)?;
    let html = read_utf8(input)?;
    html_string_to_pdf(&html, output, options, ctx)
}
