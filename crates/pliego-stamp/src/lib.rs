//! Sellos de documento: encabezados y pies con variables, marcas de agua (texto o imagen), fondos y
//! numeración Bates. Operan sobre `&mut PdfDocument` y un conjunto de páginas (desde 0).
//!
//! Todo se dibuja en el sistema de coordenadas *visual* de la página (como la ve la persona usuaria),
//! de modo que funciona igual con páginas giradas o con CropBox desplazado.

mod draw;
mod options;
mod text;

pub use draw::{background, bates, header_footer, watermark};
pub use options::{
    Background, BackgroundFill, BatesOptions, Corner, HeaderFooter, Slots, StampContext, TextStyle, Watermark,
    WatermarkContent,
};
