//! `pliego-pdf`: la **única** capa de Pliego que habla con el motor PDF.
//!
//! Hoy el motor es MuPDF; la decisión del 2026-11-04 es sustituirlo por PDFium (render y texto) + lopdf
//! (objetos y guardado) detrás de esta misma API. Ningún otro crate del workspace debe importar `mupdf`,
//! `pdfium-render` ni `lopdf` para leer, dibujar o modificar un PDF (una prueba de CI lo comprobará al terminar
//! la fase A de la ola 5; `pliego-sign` ya usa `lopdf` para firmar y se revisará aparte).
//!
//! Reglas de la API (contrato; solo se **añade**, no se cambia):
//! - Errores: [`pliego_core::Error`]; los mensajes del motor entran como `Error::Pdf(String)`.
//! - Tipos propios: geometría ([`Point`], [`Rect`], [`Matrix`]…), [`Pixmap`], [`Object`]… Nada de `mupdf::*` público.
//! - Los nombres y la forma de las funciones siguen a MuPDF a propósito, para que migrar un crate sea mecánico
//!   (quitar `.map_err(pdf_err)` y cambiar `use mupdf::…` por `use pliego_pdf::…`).
//! - Nada es `Send`: un documento vive en un solo hilo (véase `docs/ARCHITECTURE.md`).
//! - Los índices de página son desde 0.
//!
//! Estructura: [`Document`]/[`PdfDocument`] (apertura, páginas, objetos, guardado), [`Page`]/[`PdfPage`]
//! (dibujo, texto, enlaces, contenido), [`Object`] (modelo de objetos), [`TextPage`] (texto estructurado),
//! [`Pixmap`] (imágenes de salida).

mod annot;
mod backend;
mod document;
mod font;
mod fonts;
mod geom;
mod image;
mod links;
pub mod native;
mod object;
mod options;
mod outline;
mod page;
mod pixmap;
mod text;
mod widget;
mod writer;

pub use annot::{
    AnnotationBorderStyle, AnnotationColor, AnnotationDefaultAppearance, AnnotationFlags, AnnotationTextAlign,
    LineEndingStyle, PdfAnnotation, PdfAnnotationType, RedactImageMethod, RedactLineArtMethod, RedactOptions,
    RedactTextMethod,
};
pub use document::{Document, GraftMap, PdfDocument};
pub use font::{Font, PathWalker, SimpleFontEncoding};
pub use fonts::{SystemFonts, init_fonts};
pub use geom::{IRect, Matrix, Point, Quad, Rect, Size};
pub use image::{ExtractedImage, Image, ImagePlacement, ImageSource, InsertImageOptions, PageImageInfo};
pub use links::{DestinationKind, Link, LinkDestination, LinkTarget, NewLink};
pub use object::{Object, Value};
pub use options::{
    EmbeddedFileInfo, EmbeddedFileOptions, Encryption, InsertPdfOptions, InsertPdfResult, InsertPosition, MetadataName,
    OptionalContentGroup, Outline, PageLabelRule, PageLabelStyle, PageRange, PageSelection, Permission, WriteOptions,
};
pub use page::{Page, PdfPage};
pub use pixmap::{Colorspace, ImageFormat, Pixmap};
pub use text::{
    CharFlags, CharFont, TextBlock, TextBlockKind, TextChar, TextFlags, TextLine, TextPage, TextWord, WriteMode,
};

pub use widget::{FieldFlags, PdfWidget, WidgetType};
pub use writer::PdfWriter;

#[cfg(test)]
mod tests;
