//! Tipos de las anotaciones de página (la anotación en sí es [`crate::PdfAnnotation`], en `native`).
//!
//! Las coordenadas de las anotaciones están en el **espacio de página** (el que devuelven `Page::bounds`, el texto
//! y los enlaces), no en el espacio de usuario del PDF; la capa hace la conversión al escribir `/Rect`.

use std::ops::{BitOr, BitOrAssign};

/// Tipo (`/Subtype`) de una anotación.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfAnnotationType {
    Text,
    Link,
    FreeText,
    Line,
    Square,
    Circle,
    Polygon,
    PolyLine,
    Highlight,
    Underline,
    Squiggly,
    StrikeOut,
    Redact,
    Stamp,
    Caret,
    Ink,
    Popup,
    FileAttachment,
    Sound,
    Movie,
    RichMedia,
    Widget,
    Screen,
    PrinterMark,
    TrapNet,
    Watermark,
    ThreeD,
    Projection,
    Unknown,
}

/// Terminación de línea (`/LE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEndingStyle {
    None,
    Square,
    Circle,
    Diamond,
    OpenArrow,
    ClosedArrow,
    Butt,
    ROpenArrow,
    RClosedArrow,
    Slash,
}

/// Color de una anotación; componentes entre 0 y 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnnotationColor {
    Gray(f32),
    Rgb {
        red: f32,
        green: f32,
        blue: f32,
    },
    Cmyk {
        cyan: f32,
        magenta: f32,
        yellow: f32,
        key: f32,
    },
}

/// Indicadores `/F` de una anotación (combinables con `|`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AnnotationFlags(pub i32);

impl AnnotationFlags {
    pub const IS_INVISIBLE: AnnotationFlags = AnnotationFlags(1 << 0);
    pub const IS_HIDDEN: AnnotationFlags = AnnotationFlags(1 << 1);
    pub const IS_PRINT: AnnotationFlags = AnnotationFlags(1 << 2);
    pub const NO_ZOOM: AnnotationFlags = AnnotationFlags(1 << 3);
    pub const NO_ROTATE: AnnotationFlags = AnnotationFlags(1 << 4);
    pub const NO_VIEW: AnnotationFlags = AnnotationFlags(1 << 5);
    pub const IS_READ_ONLY: AnnotationFlags = AnnotationFlags(1 << 6);
    pub const IS_LOCKED: AnnotationFlags = AnnotationFlags(1 << 7);

    pub const fn empty() -> Self {
        Self(0)
    }
    pub const fn bits(self) -> i32 {
        self.0
    }
    pub const fn contains(self, o: AnnotationFlags) -> bool {
        self.0 & o.0 == o.0
    }
}

impl BitOr for AnnotationFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for AnnotationFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnotationBorderStyle {
    Solid,
    Dashed,
    Beveled,
    Inset,
    Underline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnnotationTextAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// `/DA` de una anotación de texto libre.
#[derive(Debug, Clone, PartialEq)]
pub struct AnnotationDefaultAppearance {
    pub font_name: String,
    pub size: f32,
    pub color: Option<AnnotationColor>,
}

/// Qué hacer con cada tipo de contenido al aplicar una redacción.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RedactImageMethod {
    /// No tocar las imágenes.
    None,
    /// Quitar la imagen entera si toca el área.
    Remove,
    /// Borrar solo los píxeles bajo el área (por defecto).
    #[default]
    Pixels,
    RemoveUnlessInvisible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RedactLineArtMethod {
    #[default]
    None,
    RemoveIfCovered,
    RemoveIfTouched,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RedactTextMethod {
    /// Quitar el texto bajo el área (por defecto).
    #[default]
    Remove,
    None,
    RemoveInvisible,
}

/// Cómo se aplican las redacciones. `Default` = sin cajas negras, píxeles de imagen, sin gráficos vectoriales,
/// quitando texto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RedactOptions {
    pub black_boxes: bool,
    pub image_method: RedactImageMethod,
    pub line_art: RedactLineArtMethod,
    pub text: RedactTextMethod,
}
