//! Texto estructurado de una página: bloques → líneas → caracteres, con geometría, tamaño, color y fuente.
//!
//! [`TextPage`] se extrae una vez por página ([`crate::Page::to_text_page`]) y se puede recorrer varias veces.
//! Las coordenadas están en puntos del **espacio de página** (origen arriba a la izquierda, con la rotación aplicada).

use std::ops::{BitOr, BitOrAssign};
use std::sync::Arc;

use pliego_core::Result;

use crate::geom::{Matrix, Point, Quad, Rect};

/// Opciones de extracción de texto (combinables con `|`). `TextFlags::empty()` es el comportamiento por defecto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TextFlags(u32);

impl TextFlags {
    /// Conserva las ligaduras (ﬁ, ﬂ) como un solo carácter en lugar de separarlas.
    pub const PRESERVE_LIGATURES: TextFlags = TextFlags(1 << 0);
    pub const PRESERVE_WHITESPACE: TextFlags = TextFlags(1 << 1);
    /// Incluye los bloques de imagen.
    pub const PRESERVE_IMAGES: TextFlags = TextFlags(1 << 2);
    pub const INHIBIT_SPACES: TextFlags = TextFlags(1 << 3);
    /// Une las palabras partidas con guion al final de línea.
    pub const DEHYPHENATE: TextFlags = TextFlags(1 << 4);
    pub const PRESERVE_SPANS: TextFlags = TextFlags(1 << 5);
    /// Descarta el texto fuera del recorte.
    pub const CLIP: TextFlags = TextFlags(1 << 6);
    pub const ACCURATE_BBOXES: TextFlags = TextFlags(1 << 7);

    pub const fn empty() -> Self {
        TextFlags(0)
    }

    pub const fn contains(self, other: TextFlags) -> bool {
        self.0 & other.0 == other.0
    }
}

impl BitOr for TextFlags {
    type Output = TextFlags;
    fn bitor(self, rhs: TextFlags) -> TextFlags {
        TextFlags(self.0 | rhs.0)
    }
}

impl BitOrAssign for TextFlags {
    fn bitor_assign(&mut self, rhs: TextFlags) {
        self.0 |= rhs.0;
    }
}

/// Atributos de un carácter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CharFlags(u16);

impl CharFlags {
    pub const STRIKEOUT: CharFlags = CharFlags(1 << 0);
    pub const UNDERLINE: CharFlags = CharFlags(1 << 1);
    /// Texto sintetizado por el motor (p. ej. un espacio inferido).
    pub const SYNTHETIC: CharFlags = CharFlags(1 << 2);
    /// Negrita real o simulada.
    pub const BOLD: CharFlags = CharFlags(1 << 3);

    pub const fn empty() -> Self {
        CharFlags(0)
    }

    pub const fn contains(self, other: CharFlags) -> bool {
        self.0 & other.0 == other.0
    }
}

impl BitOr for CharFlags {
    type Output = CharFlags;
    fn bitor(self, rhs: CharFlags) -> CharFlags {
        CharFlags(self.0 | rhs.0)
    }
}

impl BitOrAssign for CharFlags {
    fn bitor_assign(&mut self, rhs: CharFlags) {
        self.0 |= rhs.0;
    }
}

/// Dirección de escritura.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WriteMode {
    #[default]
    Horizontal,
    Vertical,
}

/// Datos de la fuente con la que se pintó un carácter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharFont {
    /// Nombre PostScript, con el prefijo de subconjunto (`ABCDEF+`) si lo tiene.
    pub name: String,
    pub bold: bool,
    pub italic: bool,
    pub serif: bool,
    pub monospaced: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextChar {
    /// `None` si el carácter no tiene equivalente Unicode.
    pub ch: Option<char>,
    /// Origen (línea base) del carácter.
    pub origin: Point,
    pub quad: Quad,
    pub size: f32,
    /// Color de relleno `0xAARRGGBB`.
    pub argb: u32,
    pub flags: CharFlags,
    pub font: Option<Arc<CharFont>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextLine {
    pub bounds: Rect,
    pub wmode: WriteMode,
    pub chars: Vec<TextChar>,
}

impl TextLine {
    /// Texto de la línea (los caracteres sin Unicode se omiten).
    pub fn text(&self) -> String {
        self.chars.iter().filter_map(|c| c.ch).collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TextBlockKind {
    Text,
    Image,
    /// Estructura, vectores u otros bloques que Pliego no interpreta.
    Other,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextBlock {
    pub kind: TextBlockKind,
    pub bounds: Rect,
    /// Líneas (vacío salvo en bloques de texto).
    pub lines: Vec<TextLine>,
    /// Matriz de colocación de la imagen, en bloques de imagen.
    pub image_transform: Option<Matrix>,
}

/// Palabra con su caja.
#[derive(Debug, Clone, PartialEq)]
pub struct TextWord {
    pub text: String,
    pub bounds: Rect,
    pub block: usize,
    pub line: usize,
    pub word: usize,
}

/// Texto de una página, extraído con PDFium y estructurado en bloques → líneas → caracteres.
pub struct TextPage {
    blocks: Vec<TextBlock>,
}

impl TextPage {
    pub(crate) fn from_blocks(blocks: Vec<TextBlock>) -> Self {
        Self { blocks }
    }

    /// Texto plano de toda la página (líneas separadas por salto de línea).
    pub fn to_text(&self) -> Result<String> {
        Ok(crate::pdfium::blocks_to_text(&self.blocks))
    }

    /// HTML sencillo de la página (un párrafo por línea, colocado con coordenadas) con `id` como número de página.
    pub fn to_html(&self, id: i32, full: bool) -> Result<String> {
        Ok(crate::pdfium::blocks_to_html(&self.blocks, id, full))
    }

    /// Cuadriláteros de las apariciones de `needle` (sin distinguir mayúsculas).
    pub fn search(&self, needle: &str) -> Result<Vec<Quad>> {
        Ok(crate::pdfium::search_blocks(&self.blocks, needle))
    }

    /// Bloques → líneas → caracteres. Se calcula en cada llamada: guarda el resultado si lo vas a reutilizar.
    pub fn blocks(&self) -> Vec<TextBlock> {
        self.blocks.clone()
    }

    /// Palabras con su caja.
    pub fn words(&self) -> Vec<TextWord> {
        crate::pdfium::words_of(&self.blocks)
    }
}
