//! Texto estructurado de una página: bloques → líneas → caracteres, con geometría, tamaño, color y fuente.
//!
//! [`TextPage`] se extrae una vez por página ([`crate::Page::to_text_page`]) y se puede recorrer varias veces.
//! Las coordenadas están en puntos del **espacio de página** (origen arriba a la izquierda, con la rotación aplicada).

use std::collections::HashMap;
use std::ops::{BitOr, BitOrAssign};
use std::sync::Arc;

use pliego_core::Result;

use crate::backend::{err, matrix_from, point_from, quad_from, quad_to, rect_from};
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

    pub(crate) fn to_backend(self) -> mupdf::TextPageFlags {
        use mupdf::TextPageFlags as F;
        let table = [
            (Self::PRESERVE_LIGATURES, F::PRESERVE_LIGATURES),
            (Self::PRESERVE_WHITESPACE, F::PRESERVE_WHITESPACE),
            (Self::PRESERVE_IMAGES, F::PRESERVE_IMAGES),
            (Self::INHIBIT_SPACES, F::INHIBIT_SPACES),
            (Self::DEHYPHENATE, F::DEHYPHENATE),
            (Self::PRESERVE_SPANS, F::PRESERVE_SPANS),
            (Self::CLIP, F::CLIP),
            (Self::ACCURATE_BBOXES, F::ACCURATE_BBOXES),
        ];
        let mut out = F::empty();
        for (mine, theirs) in table {
            if self.contains(mine) {
                out |= theirs;
            }
        }
        out
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

    fn from_backend(f: mupdf::text_page::TextCharFlags) -> Self {
        use mupdf::text_page::TextCharFlags as F;
        let mut out = 0;
        for (theirs, mine) in [
            (F::STRIKEOUT, Self::STRIKEOUT),
            (F::UNDERLINE, Self::UNDERLINE),
            (F::SYNTHETIC, Self::SYNTHETIC),
            (F::BOLD, Self::BOLD),
        ] {
            if f.contains(theirs) {
                out |= mine.0;
            }
        }
        CharFlags(out)
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

/// Texto de una página. Lo produce MuPDF o, si el documento usa [`crate::RenderBackend::Pdfium`], PDFium.
pub struct TextPage {
    inner: Inner,
}

enum Inner {
    Mupdf(mupdf::TextPage),
    /// Texto ya estructurado (bloques → líneas → caracteres) a partir de los caracteres de PDFium.
    #[cfg(feature = "pdfium")]
    Pdfium(Vec<TextBlock>),
}

impl TextPage {
    pub(crate) fn from_mupdf(raw: mupdf::TextPage) -> Self {
        Self {
            inner: Inner::Mupdf(raw),
        }
    }

    #[cfg(feature = "pdfium")]
    pub(crate) fn from_pdfium(blocks: Vec<TextBlock>) -> Self {
        Self {
            inner: Inner::Pdfium(blocks),
        }
    }

    fn mupdf(&self) -> Option<&mupdf::TextPage> {
        match &self.inner {
            Inner::Mupdf(raw) => Some(raw),
            #[cfg(feature = "pdfium")]
            Inner::Pdfium(_) => None,
        }
    }

    /// Texto plano de toda la página (líneas separadas por salto de línea).
    pub fn to_text(&self) -> Result<String> {
        match &self.inner {
            Inner::Mupdf(raw) => raw.to_text().map_err(err),
            #[cfg(feature = "pdfium")]
            Inner::Pdfium(blocks) => Ok(crate::pdfium::blocks_to_text(blocks)),
        }
    }

    /// HTML de la página con `id` como número de página. (Con PDFium se genera un HTML sencillo de párrafos.)
    pub fn to_html(&self, id: i32, full: bool) -> Result<String> {
        match &self.inner {
            Inner::Mupdf(raw) => raw.to_html(id, full).map_err(err),
            #[cfg(feature = "pdfium")]
            Inner::Pdfium(blocks) => Ok(crate::pdfium::blocks_to_html(blocks, id, full)),
        }
    }

    /// Cuadriláteros de las apariciones de `needle` (sin distinguir mayúsculas).
    pub fn search(&self, needle: &str) -> Result<Vec<Quad>> {
        match &self.inner {
            Inner::Mupdf(raw) => Ok(raw.search(needle).map_err(err)?.into_iter().map(quad_from).collect()),
            #[cfg(feature = "pdfium")]
            Inner::Pdfium(blocks) => Ok(crate::pdfium::search_blocks(blocks, needle)),
        }
    }

    /// Bloques → líneas → caracteres. Se calcula en cada llamada: guarda el resultado si lo vas a reutilizar.
    pub fn blocks(&self) -> Vec<TextBlock> {
        #[cfg(feature = "pdfium")]
        if let Inner::Pdfium(blocks) = &self.inner {
            return blocks.clone();
        }
        let Some(raw) = self.mupdf() else { return Vec::new() };
        let mut fonts: HashMap<(String, u8), Arc<CharFont>> = HashMap::new();
        let mut out = Vec::new();
        for block in raw.blocks() {
            let bounds = rect_from(block.bounds());
            match block.r#type() {
                mupdf::text_page::TextBlockType::Text => {
                    let mut lines = Vec::new();
                    for line in block.lines() {
                        let mut chars = Vec::new();
                        for ch in line.chars() {
                            let font = ch.font().map(|f| {
                                let key = (
                                    f.name().to_owned(),
                                    u8::from(f.is_bold())
                                        | u8::from(f.is_italic()) << 1
                                        | u8::from(f.is_serif()) << 2
                                        | u8::from(f.is_monospaced()) << 3,
                                );
                                fonts
                                    .entry(key)
                                    .or_insert_with(|| {
                                        Arc::new(CharFont {
                                            name: f.name().to_owned(),
                                            bold: f.is_bold(),
                                            italic: f.is_italic(),
                                            serif: f.is_serif(),
                                            monospaced: f.is_monospaced(),
                                        })
                                    })
                                    .clone()
                            });
                            chars.push(TextChar {
                                ch: ch.char(),
                                origin: point_from(ch.origin()),
                                quad: quad_from(ch.quad()),
                                size: ch.size(),
                                argb: ch.argb(),
                                flags: CharFlags::from_backend(ch.flags()),
                                font,
                            });
                        }
                        lines.push(TextLine {
                            bounds: rect_from(line.bounds()),
                            wmode: match line.wmode() {
                                mupdf::WriteMode::Horizontal => WriteMode::Horizontal,
                                mupdf::WriteMode::Vertical => WriteMode::Vertical,
                            },
                            chars,
                        });
                    }
                    out.push(TextBlock {
                        kind: TextBlockKind::Text,
                        bounds,
                        lines,
                        image_transform: None,
                    });
                }
                mupdf::text_page::TextBlockType::Image => out.push(TextBlock {
                    kind: TextBlockKind::Image,
                    bounds,
                    lines: Vec::new(),
                    image_transform: block.ctm().map(matrix_from),
                }),
                _ => out.push(TextBlock {
                    kind: TextBlockKind::Other,
                    bounds,
                    lines: Vec::new(),
                    image_transform: None,
                }),
            }
        }
        out
    }

    /// Palabras con su caja.
    pub fn words(&self) -> Vec<TextWord> {
        #[cfg(feature = "pdfium")]
        if let Inner::Pdfium(blocks) = &self.inner {
            return crate::pdfium::words_of(blocks);
        }
        let Some(raw) = self.mupdf() else { return Vec::new() };
        raw.words()
            .into_iter()
            .map(|w| TextWord {
                text: w.text,
                bounds: rect_from(w.bounds),
                block: w.block,
                line: w.line,
                word: w.word,
            })
            .collect()
    }
}

/// Convierte cuadriláteros propios a los del motor (para `highlight_selection` y similares).
#[allow(dead_code)]
pub(crate) fn quads_to(q: &[Quad]) -> Vec<mupdf::Quad> {
    q.iter().copied().map(quad_to).collect()
}
