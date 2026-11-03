//! OCR de Pliego.
//!
//! - [`OcrEngine`]: contrato de un motor de reconocimiento (hoy [`TesseractEngine`]; más adelante PaddleOCR/ONNX).
//! - [`tessdata`]: búsqueda de modelos de idioma instalados.
//! - [`ocr_page_image`] / [`available_languages`]: funciones de conveniencia de la API pública.

mod image;
pub mod tessdata;
mod tesseract;

pub use image::{GrayImage, PixelRect, Word};
pub use tessdata::available_languages;
pub use tesseract::TesseractEngine;

use pliego_core::Result;

/// Motor de reconocimiento de texto sobre una imagen en escala de grises.
///
/// Un motor no es compartible entre hilos mientras trabaja (`&mut self`); para paralelizar se crea uno por hilo.
pub trait OcrEngine: Send {
    /// Reconoce las palabras de `img`. `dpi` es la resolución real de la imagen (ayuda a Tesseract a fijar tamaños).
    /// Las coordenadas devueltas están en píxeles de `img`, con el origen arriba a la izquierda.
    fn recognize(&mut self, img: &GrayImage, dpi: u32) -> Result<Vec<Word>>;

    /// Orientación del texto en grados a favor del reloj que hay que girar la imagen para dejarla derecha
    /// (0, 90, 180 o 270) y su confianza. `None` si el motor no sabe o falta el modelo `osd`.
    fn detect_orientation(&mut self, img: &GrayImage) -> Result<Option<(u32, f32)>>;
}

/// Reconoce una imagen con Tesseract. `langs` al estilo Tesseract: `"spa+eng"`.
pub fn ocr_page_image(img: &GrayImage, langs: &str) -> Result<Vec<Word>> {
    let mut engine = TesseractEngine::new(langs, None)?;
    engine.recognize(img, 300)
}
