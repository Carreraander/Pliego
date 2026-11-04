//! Fuentes: cargar, medir y obtener contornos; e incrustarlas en un documento.

use pliego_core::Result;

use crate::backend::{err, matrix_to};
use crate::document::PdfDocument;
use crate::geom::Matrix;
use crate::object::Object;
use crate::page::PdfPage;

/// Recorrido de un contorno de glifo o de un trazado.
pub trait PathWalker {
    fn move_to(&mut self, x: f32, y: f32);
    fn line_to(&mut self, x: f32, y: f32);
    fn curve_to(&mut self, cx1: f32, cy1: f32, cx2: f32, cy2: f32, ex: f32, ey: f32);
    fn close(&mut self);
}

struct Bridge<'a>(&'a mut dyn PathWalker);

impl mupdf::PathWalker for Bridge<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.move_to(x, y)
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.0.line_to(x, y)
    }
    fn curve_to(&mut self, cx1: f32, cy1: f32, cx2: f32, cy2: f32, ex: f32, ey: f32) {
        self.0.curve_to(cx1, cy1, cx2, cy2, ex, ey)
    }
    fn close(&mut self) {
        self.0.close()
    }
}

/// Codificación de una fuente simple incrustada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimpleFontEncoding {
    Latin,
    Greek,
    Cyrillic,
}

/// Fuente cargada. No es `Send`.
#[derive(Debug)]
pub struct Font {
    raw: mupdf::Font,
}

impl Font {
    /// Fuente por nombre. Los 14 nombres estándar (`Helvetica`, `Times-Roman`, `Courier-Bold`…) están siempre
    /// disponibles; otros nombres se buscan entre las fuentes del sistema.
    pub fn new(name: &str) -> Result<Font> {
        crate::fonts::init_fonts();
        mupdf::Font::new(name).map(|raw| Font { raw }).map_err(err)
    }

    /// Fuente a partir de los bytes de un archivo TrueType/OpenType/Type1.
    pub fn from_bytes(name: &str, data: &[u8]) -> Result<Font> {
        crate::fonts::init_fonts();
        mupdf::Font::from_bytes(name, data).map(|raw| Font { raw }).map_err(err)
    }

    pub fn name(&self) -> &str {
        self.raw.name()
    }
    pub fn is_bold(&self) -> bool {
        self.raw.is_bold()
    }
    pub fn is_italic(&self) -> bool {
        self.raw.is_italic()
    }
    pub fn is_serif(&self) -> bool {
        self.raw.is_serif()
    }
    pub fn is_monospaced(&self) -> bool {
        self.raw.is_monospaced()
    }
    /// Ascendente en unidades de tamaño (1.0 = el tamaño de la fuente).
    pub fn ascender(&self) -> f32 {
        self.raw.ascender()
    }
    pub fn descender(&self) -> f32 {
        self.raw.descender()
    }

    /// Glifo de un carácter Unicode (0 = `.notdef`).
    pub fn encode_character(&self, unicode: i32) -> Result<i32> {
        self.raw.encode_character(unicode).map_err(err)
    }

    /// Avance horizontal del glifo en unidades de tamaño.
    pub fn advance_glyph(&self, glyph: i32) -> Result<f32> {
        self.raw.advance_glyph(glyph).map_err(err)
    }

    /// Recorre el contorno del glifo transformado por `ctm`. Devuelve `false` si el glifo no tiene contorno.
    pub fn outline_glyph(&self, glyph: i32, ctm: &Matrix, walker: &mut dyn PathWalker) -> Result<bool> {
        match self.raw.outline_glyph_with_ctm(glyph, &matrix_to(ctm)).map_err(err)? {
            Some(path) => {
                path.walk(Bridge(walker)).map_err(err)?;
                Ok(true)
            }
            None => Ok(false),
        }
    }
}

impl PdfDocument {
    /// Incrusta `font` como fuente simple (un byte por carácter) y devuelve el diccionario de la fuente
    /// (referencia indirecta), listo para ponerlo en `/Resources /Font`.
    pub fn add_simple_font(&mut self, font: &Font, encoding: SimpleFontEncoding) -> Result<Object> {
        let enc = match encoding {
            SimpleFontEncoding::Latin => mupdf::SimpleFontEncoding::Latin,
            SimpleFontEncoding::Greek => mupdf::SimpleFontEncoding::Greek,
            SimpleFontEncoding::Cyrillic => mupdf::SimpleFontEncoding::Cyrillic,
        };
        self.pdf_mut()
            .add_simple_font(&font.raw, enc)
            .map(Object::wrap)
            .map_err(err)
    }

    /// Incrusta `font` como fuente Unicode (CID, Identity-H).
    pub fn add_font(&mut self, font: &Font) -> Result<Object> {
        self.pdf_mut().add_font(&font.raw).map(Object::wrap).map_err(err)
    }
}

impl PdfPage {
    /// Añade a los recursos de la página una fuente estándar por nombre y devuelve `(nombre de recurso con barra,
    /// número de objeto)`.
    pub fn insert_font(&mut self, doc: &mut PdfDocument, name: &str) -> Result<(String, i32)> {
        let (res, xref, _) = self
            .pdf_mut()
            .insert_font(doc.pdf_mut(), &mupdf::pdf::InsertFontOptions::new(name))
            .map_err(err)?;
        Ok((res, xref))
    }
}
