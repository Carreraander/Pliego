//! Maquetador propio de texto plano y Markdown a PDF (W40, bloque 8). Sustituye al motor HTML de MuPDF para estos dos
//! formatos: usa las fuentes estándar de PDF (Times y Courier, codificación WinAnsi), no necesita navegador ni MuPDF
//! y escribe el PDF con el modelo propio (`pliego_pdf::native`).
//!
//! # Qué admite
//! - Texto plano: monoespaciado, tabuladores a 4 columnas, ajuste de línea (`pre-wrap`).
//! - Markdown (CommonMark + tablas, tachado y listas de tareas): párrafos, títulos 1-6, énfasis, negrita, código en
//!   línea y en bloque, citas, listas ordenadas y sin ordenar anidadas, tablas, reglas, enlaces (con anotación) y HTML
//!   en línea reducido a su texto (`<br>` es un salto).
//! - Salto de página sin dejar un título al final.
//!
//! # Qué no
//! - Caracteres fuera de Windows-1252 (CJK, cirílico, griego, emojis…): se sustituyen por «?» y el informe lo avisa.
//!   Algunos tipográficos habituales se transcriben (`→` por `->`, `−` por `-`…).
//! - Imágenes del Markdown (sale su texto alternativo entre corchetes), notas al pie, fórmulas, CSS.

mod layout;
mod markdown;
mod metrics;
mod winansi;

pub(crate) use layout::{PageSpec, save_bytes};

use pliego_core::Result;
use pliego_pdf::native::PdfDocument;

/// Resultado del maquetado: el PDF y cuántos caracteres no se pudieron representar.
pub(crate) struct Typeset {
    pub(crate) pdf: PdfDocument,
    pub(crate) pages: usize,
    pub(crate) substituted: usize,
}

/// Texto plano → PDF.
pub(crate) fn plain(text: &str, spec: &PageSpec) -> Result<Typeset> {
    layout::typeset_plain(text, spec)
}

/// Markdown → PDF.
pub(crate) fn markdown(md: &str, spec: &PageSpec) -> Result<Typeset> {
    markdown::typeset(md, spec)
}
