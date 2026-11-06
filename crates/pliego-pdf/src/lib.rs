//! `pliego-pdf`: la **única** capa de Pliego que habla con el motor PDF.
//!
//! El motor son lopdf (modelo de objetos, guardado, cifrado, anotaciones, formularios, fuentes, imágenes y redacción;
//! módulo [`native`]) y PDFium (dibujo, texto y búsqueda). MuPDF ya no está (W41). Ningún otro crate del workspace
//! debe importar `pdfium-render` ni `lopdf` para leer, dibujar o modificar un PDF (`pliego-sign` usa `lopdf` para
//! firmar y se revisa aparte).
//!
//! **PDFium** se carga en tiempo de ejecución: pon `pdfium.dll` (`libpdfium.so`) junto al ejecutable o define
//! `PLIEGO_PDFIUM_DLL` con su ruta absoluta (véase [`pdfium_init_from_environment`]). Sin la biblioteca, dibujar,
//! extraer texto y buscar dan error; el resto funciona.
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
//! [`Pixmap`] (imágenes de salida). [`Document`] envuelve a [`PdfDocument`] y [`Page`] es otro nombre de [`PdfPage`].

mod annot;
mod document;
mod engine;
mod font;
mod geom;
mod links;
pub mod native;
mod object;
mod options;
mod pdfium;
mod pixmap;
pub mod print;
mod text;
mod widget;

pub use annot::{
    AnnotationBorderStyle, AnnotationColor, AnnotationDefaultAppearance, AnnotationFlags, AnnotationTextAlign,
    LineEndingStyle, PdfAnnotationType, RedactImageMethod, RedactLineArtMethod, RedactOptions, RedactTextMethod,
};
pub use document::Document;
pub use engine::{
    configure_from_environment, pdfium_available, pdfium_default_path, pdfium_init, pdfium_init_from_environment,
    pdfium_library_name,
};
pub use font::{PathWalker, SimpleFontEncoding};
pub use geom::{IRect, Matrix, Point, Quad, Rect, Size};
pub use links::{DestinationKind, Link, LinkDestination, LinkTarget, NewLink};
pub use native::{
    ContentMove, CutContent, ExtractedImage, Font, GraftMap, Image, ImagePlacement, ImageSource, InsertImageOptions,
    MoveReport, Object, PageImageInfo, PdfAnnotation, PdfDocument, PdfPage, PdfWidget,
};
pub use object::Value;
pub use options::{
    EmbeddedFileInfo, EmbeddedFileOptions, Encryption, InsertPdfOptions, InsertPdfResult, InsertPosition, MetadataName,
    OptionalContentGroup, Outline, PageLabelRule, PageLabelStyle, PageRange, PageSelection, Permission, WriteOptions,
};
pub use pixmap::{Colorspace, ImageFormat, Pixmap};
pub use text::{
    CharFlags, CharFont, TextBlock, TextBlockKind, TextChar, TextFlags, TextLine, TextPage, TextWord, WriteMode,
};
pub use widget::{FieldFlags, WidgetType};

/// Página de un documento. Es otro nombre de [`PdfPage`] (la capa solo abre PDF).
pub type Page = PdfPage;

#[cfg(test)]
mod tests;
