//! Anotaciones de página: crear, leer y modificar. También aplicar las de redacción.
//!
//! Las coordenadas de las anotaciones están en el **espacio de página** (el que devuelven `Page::bounds`, el texto
//! y los enlaces), no en el espacio de usuario del PDF; la capa hace la conversión al escribir `/Rect`.

use std::ops::{BitOr, BitOrAssign};

use pliego_core::{Error, Result};

use crate::backend::{err, point_from, point_to, quad_from, quad_to, rect_from, rect_to};
use crate::geom::{Point, Quad, Rect};
use crate::object::Object;
use crate::page::PdfPage;

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

macro_rules! map_enum {
    ($to:ident, $from:ident, $mine:ident => $theirs:path : $($v:ident),+ $(,)?) => {
        fn $to(v: $mine) -> $theirs {
            match v { $( $mine::$v => <$theirs>::$v, )+ }
        }
        fn $from(v: $theirs) -> $mine {
            match v { $( <$theirs>::$v => $mine::$v, )+ #[allow(unreachable_patterns)] _ => $mine::Unknown }
        }
    };
}

map_enum!(
    annot_type_to, annot_type_from, PdfAnnotationType => mupdf::pdf::PdfAnnotationType:
    Text, Link, FreeText, Line, Square, Circle, Polygon, PolyLine, Highlight, Underline, Squiggly, StrikeOut,
    Redact, Stamp, Caret, Ink, Popup, FileAttachment, Sound, Movie, RichMedia, Widget, Screen, PrinterMark,
    TrapNet, Watermark, ThreeD, Projection, Unknown,
);

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

fn le_to(v: LineEndingStyle) -> mupdf::pdf::LineEndingStyle {
    use mupdf::pdf::LineEndingStyle as L;
    match v {
        LineEndingStyle::None => L::None,
        LineEndingStyle::Square => L::Square,
        LineEndingStyle::Circle => L::Circle,
        LineEndingStyle::Diamond => L::Diamond,
        LineEndingStyle::OpenArrow => L::OpenArrow,
        LineEndingStyle::ClosedArrow => L::ClosedArrow,
        LineEndingStyle::Butt => L::Butt,
        LineEndingStyle::ROpenArrow => L::ROpenArrow,
        LineEndingStyle::RClosedArrow => L::RClosedArrow,
        LineEndingStyle::Slash => L::Slash,
    }
}

fn le_from(v: mupdf::pdf::LineEndingStyle) -> LineEndingStyle {
    use mupdf::pdf::LineEndingStyle as L;
    match v {
        L::None => LineEndingStyle::None,
        L::Square => LineEndingStyle::Square,
        L::Circle => LineEndingStyle::Circle,
        L::Diamond => LineEndingStyle::Diamond,
        L::OpenArrow => LineEndingStyle::OpenArrow,
        L::ClosedArrow => LineEndingStyle::ClosedArrow,
        L::Butt => LineEndingStyle::Butt,
        L::ROpenArrow => LineEndingStyle::ROpenArrow,
        L::RClosedArrow => LineEndingStyle::RClosedArrow,
        L::Slash => LineEndingStyle::Slash,
    }
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

fn color_to(c: AnnotationColor) -> mupdf::color::AnnotationColor {
    use mupdf::color::AnnotationColor as C;
    match c {
        AnnotationColor::Gray(g) => C::Gray(g),
        AnnotationColor::Rgb { red, green, blue } => C::Rgb { red, green, blue },
        AnnotationColor::Cmyk {
            cyan,
            magenta,
            yellow,
            key,
        } => C::Cmyk {
            cyan,
            magenta,
            yellow,
            key,
        },
    }
}

fn color_from(c: mupdf::color::AnnotationColor) -> AnnotationColor {
    use mupdf::color::AnnotationColor as C;
    match c {
        C::Gray(g) => AnnotationColor::Gray(g),
        C::Rgb { red, green, blue } => AnnotationColor::Rgb { red, green, blue },
        C::Cmyk {
            cyan,
            magenta,
            yellow,
            key,
        } => AnnotationColor::Cmyk {
            cyan,
            magenta,
            yellow,
            key,
        },
    }
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

/// Cómo se aplican las redacciones. `Default` = comportamiento por defecto de MuPDF (sin cajas negras, píxeles de
/// imagen, sin gráficos vectoriales, quitando texto).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RedactOptions {
    pub black_boxes: bool,
    pub image_method: RedactImageMethod,
    pub line_art: RedactLineArtMethod,
    pub text: RedactTextMethod,
}

fn redact_to(o: RedactOptions) -> mupdf::pdf::PdfRedactOptions {
    use mupdf::pdf as p;
    mupdf::pdf::PdfRedactOptions {
        black_boxes: o.black_boxes,
        image_method: match o.image_method {
            RedactImageMethod::None => p::PdfRedactImageMethod::None,
            RedactImageMethod::Remove => p::PdfRedactImageMethod::Remove,
            RedactImageMethod::Pixels => p::PdfRedactImageMethod::Pixels,
            RedactImageMethod::RemoveUnlessInvisible => p::PdfRedactImageMethod::RemoveUnlessInvisible,
        },
        line_art: match o.line_art {
            RedactLineArtMethod::None => p::PdfRedactLineArtMethod::None,
            RedactLineArtMethod::RemoveIfCovered => p::PdfRedactLineArtMethod::RemoveIfCovered,
            RedactLineArtMethod::RemoveIfTouched => p::PdfRedactLineArtMethod::RemoveIfTouched,
        },
        text: match o.text {
            RedactTextMethod::Remove => p::PdfRedactTextMethod::Remove,
            RedactTextMethod::None => p::PdfRedactTextMethod::None,
            RedactTextMethod::RemoveInvisible => p::PdfRedactTextMethod::RemoveInvisible,
        },
    }
}

/// Anotación de una página. Mientras exista, su página debe seguir viva y la anotación adjunta.
pub struct PdfAnnotation {
    raw: AnnotRaw,
}

enum AnnotRaw {
    Own(mupdf::pdf::PdfAnnotation),
    /// Vista de solo lectura de la anotación de un campo (`PdfWidget::annotation`).
    OfWidget(mupdf::pdf::PdfWidget),
}

impl PdfAnnotation {
    pub(crate) fn wrap(raw: mupdf::pdf::PdfAnnotation) -> Self {
        Self {
            raw: AnnotRaw::Own(raw),
        }
    }

    pub(crate) fn of_widget(raw: mupdf::pdf::PdfWidget) -> Self {
        Self {
            raw: AnnotRaw::OfWidget(raw),
        }
    }

    pub(crate) fn into_inner(self) -> Result<mupdf::pdf::PdfAnnotation> {
        match self.raw {
            AnnotRaw::Own(a) => Ok(a),
            AnnotRaw::OfWidget(w) => Ok(w.into_annotation()),
        }
    }

    pub(crate) fn base(&self) -> &mupdf::pdf::PdfAnnotation {
        match &self.raw {
            AnnotRaw::Own(a) => a,
            AnnotRaw::OfWidget(w) => w.annotation(),
        }
    }

    pub(crate) fn widget(&self) -> Result<&mupdf::pdf::PdfWidget> {
        match &self.raw {
            AnnotRaw::OfWidget(w) => Ok(w),
            AnnotRaw::Own(_) => Err(Error::Unsupported("la anotación no es un campo de formulario".into())),
        }
    }

    pub(crate) fn widget_mut(&mut self) -> Result<&mut mupdf::pdf::PdfWidget> {
        match &mut self.raw {
            AnnotRaw::OfWidget(w) => Ok(w),
            AnnotRaw::Own(_) => Err(Error::Unsupported("la anotación no es un campo de formulario".into())),
        }
    }

    fn base_mut(&mut self) -> Result<&mut mupdf::pdf::PdfAnnotation> {
        match &mut self.raw {
            AnnotRaw::Own(a) => Ok(a),
            AnnotRaw::OfWidget(_) => Err(Error::Unsupported(
                "esta anotación es una vista de solo lectura de un campo; usa PdfWidget::into_annotation".into(),
            )),
        }
    }

    pub fn r#type(&self) -> Result<PdfAnnotationType> {
        self.base().r#type().map(annot_type_from).map_err(err)
    }

    /// Diccionario de la anotación.
    pub fn object(&self) -> Object {
        Object::wrap(self.base().object())
    }

    pub fn xref(&self) -> Result<i32> {
        self.base().xref().map_err(err)
    }

    pub fn bounds(&self) -> Result<Rect> {
        self.base().bounds().map(rect_from).map_err(err)
    }

    /// Rectángulo en el espacio de página.
    pub fn rect(&self) -> Result<Rect> {
        self.base().rect().map(rect_from).map_err(err)
    }

    pub fn set_rect(&mut self, rect: Rect) -> Result<()> {
        self.base_mut()?.set_rect(rect_to(rect)).map_err(err)
    }

    /// Regenera la apariencia si hace falta. `true` si cambió.
    pub fn update(&mut self) -> Result<bool> {
        self.base_mut()?.update().map_err(err)
    }

    /// Aplica solo esta redacción (consume la anotación).
    pub fn apply_redaction(self, options: RedactOptions) -> Result<bool> {
        self.into_inner()?
            .apply_redaction_with_options(redact_to(options))
            .map_err(err)
    }

    pub fn flags(&self) -> Result<AnnotationFlags> {
        self.base().flags().map(|f| AnnotationFlags(f.bits())).map_err(err)
    }

    pub fn set_flags(&mut self, flags: AnnotationFlags) -> Result<()> {
        self.base_mut()?
            .set_flags(mupdf::pdf::AnnotationFlags::from_bits_retain(flags.bits()))
            .map_err(err)
    }

    pub fn color(&self) -> Result<Option<AnnotationColor>> {
        self.base().color().map(|c| c.map(color_from)).map_err(err)
    }

    pub fn set_color(&mut self, color: AnnotationColor) -> Result<()> {
        self.base_mut()?.set_color(color_to(color)).map_err(err)
    }

    pub fn interior_color(&self) -> Result<Option<AnnotationColor>> {
        self.base().interior_color().map(|c| c.map(color_from)).map_err(err)
    }

    pub fn set_interior_color(&mut self, color: AnnotationColor) -> Result<()> {
        self.base_mut()?.set_interior_color(color_to(color)).map_err(err)
    }

    pub fn opacity(&self) -> Result<f32> {
        self.base().opacity().map_err(err)
    }

    pub fn set_opacity(&mut self, opacity: f32) -> Result<()> {
        self.base_mut()?.set_opacity(opacity).map_err(err)
    }

    pub fn border_width(&self) -> Result<f32> {
        self.base().border_width().map_err(err)
    }

    pub fn set_border_width(&mut self, width: f32) -> Result<()> {
        self.base_mut()?.set_border_width(width).map_err(err)
    }

    pub fn border_style(&self) -> Result<AnnotationBorderStyle> {
        use mupdf::pdf::AnnotationBorderStyle as B;
        self.base()
            .border_style()
            .map(|s| match s {
                B::Solid => AnnotationBorderStyle::Solid,
                B::Dashed => AnnotationBorderStyle::Dashed,
                B::Beveled => AnnotationBorderStyle::Beveled,
                B::Inset => AnnotationBorderStyle::Inset,
                B::Underline => AnnotationBorderStyle::Underline,
            })
            .map_err(err)
    }

    pub fn set_border_style(&mut self, style: AnnotationBorderStyle) -> Result<()> {
        use mupdf::pdf::AnnotationBorderStyle as B;
        self.base_mut()?
            .set_border_style(match style {
                AnnotationBorderStyle::Solid => B::Solid,
                AnnotationBorderStyle::Dashed => B::Dashed,
                AnnotationBorderStyle::Beveled => B::Beveled,
                AnnotationBorderStyle::Inset => B::Inset,
                AnnotationBorderStyle::Underline => B::Underline,
            })
            .map_err(err)
    }

    pub fn border_dash_pattern(&self) -> Result<Vec<f32>> {
        self.base().border_dash_pattern().map_err(err)
    }

    pub fn set_border_dash_pattern(&mut self, lengths: &[f32]) -> Result<()> {
        self.base_mut()?.set_border_dash_pattern(lengths).map_err(err)
    }

    pub fn author(&self) -> Result<Option<String>> {
        self.base().author().map(|a| a.map(str::to_owned)).map_err(err)
    }

    pub fn set_author(&mut self, author: &str) -> Result<()> {
        self.base_mut()?.set_author(author).map_err(err)
    }

    pub fn contents(&self) -> Result<Option<String>> {
        self.base().contents().map(|c| c.map(str::to_owned)).map_err(err)
    }

    pub fn set_contents(&mut self, contents: &str) -> Result<()> {
        self.base_mut()?.set_contents(contents).map_err(err)
    }

    pub fn icon_name(&self) -> Result<Option<String>> {
        self.base().icon_name().map(|c| c.map(str::to_owned)).map_err(err)
    }

    pub fn set_icon_name(&mut self, name: &str) -> Result<()> {
        self.base_mut()?.set_icon_name(name).map_err(err)
    }

    pub fn is_open(&self) -> Result<bool> {
        self.base().is_open().map_err(err)
    }

    pub fn set_open(&mut self, open: bool) -> Result<()> {
        self.base_mut()?.set_open(open).map_err(err)
    }

    pub fn set_popup(&mut self, rect: Rect) -> Result<()> {
        self.base_mut()?.set_popup(rect_to(rect)).map_err(err)
    }

    pub fn quadding(&self) -> Result<AnnotationTextAlign> {
        use mupdf::pdf::AnnotationTextAlign as A;
        self.base()
            .quadding()
            .map(|a| match a {
                A::Left => AnnotationTextAlign::Left,
                A::Center => AnnotationTextAlign::Center,
                A::Right => AnnotationTextAlign::Right,
            })
            .map_err(err)
    }

    pub fn set_quadding(&mut self, align: AnnotationTextAlign) -> Result<()> {
        use mupdf::pdf::AnnotationTextAlign as A;
        self.base_mut()?
            .set_quadding(match align {
                AnnotationTextAlign::Left => A::Left,
                AnnotationTextAlign::Center => A::Center,
                AnnotationTextAlign::Right => A::Right,
            })
            .map_err(err)
    }

    // --- Geometría específica de cada tipo ---

    pub fn quad_points(&self) -> Result<Vec<Quad>> {
        Ok(self
            .base()
            .quad_points()
            .map_err(err)?
            .into_iter()
            .map(quad_from)
            .collect())
    }

    pub fn set_quad_points(&mut self, quads: &[Quad]) -> Result<()> {
        let q: Vec<mupdf::Quad> = quads.iter().copied().map(quad_to).collect();
        self.base_mut()?.set_quad_points(q).map_err(err)
    }

    pub fn line(&self) -> Result<(Point, Point)> {
        self.base()
            .line()
            .map(|(a, b)| (point_from(a), point_from(b)))
            .map_err(err)
    }

    pub fn set_line(&mut self, start: Point, end: Point) -> Result<()> {
        self.base_mut()?.set_line(point_to(start), point_to(end)).map_err(err)
    }

    pub fn line_ending_styles(&self) -> Result<(LineEndingStyle, LineEndingStyle)> {
        self.base()
            .line_ending_styles()
            .map(|(a, b)| (le_from(a), le_from(b)))
            .map_err(err)
    }

    pub fn set_line_ending_styles(&mut self, start: LineEndingStyle, end: LineEndingStyle) -> Result<()> {
        self.base_mut()?
            .set_line_ending_styles(le_to(start), le_to(end))
            .map_err(err)
    }

    pub fn vertices(&self) -> Result<Vec<Point>> {
        Ok(self
            .base()
            .vertices()
            .map_err(err)?
            .into_iter()
            .map(point_from)
            .collect())
    }

    pub fn set_vertices(&mut self, points: impl IntoIterator<Item = Point>) -> Result<()> {
        self.base_mut()?
            .set_vertices(points.into_iter().map(point_to))
            .map_err(err)
    }

    pub fn ink_list(&self) -> Result<Vec<Vec<Point>>> {
        Ok(self
            .base()
            .ink_list()
            .map_err(err)?
            .into_iter()
            .map(|s| s.into_iter().map(point_from).collect())
            .collect())
    }

    pub fn set_ink_list<I, S>(&mut self, strokes: I) -> Result<()>
    where
        I: IntoIterator<Item = S>,
        S: IntoIterator<Item = Point>,
    {
        self.base_mut()?
            .set_ink_list(strokes.into_iter().map(|s| s.into_iter().map(point_to)))
            .map_err(err)
    }

    pub fn default_appearance(&self) -> Result<Option<AnnotationDefaultAppearance>> {
        Ok(self
            .base()
            .default_appearance()
            .map_err(err)?
            .map(|d| AnnotationDefaultAppearance {
                font_name: d.font_name,
                size: d.size,
                color: d.color.map(color_from),
            }))
    }

    pub fn set_default_appearance(&mut self, font_name: &str, size: f32, color: Option<AnnotationColor>) -> Result<()> {
        self.base_mut()?
            .set_default_appearance(font_name, size, color.map(color_to))
            .map_err(err)
    }
}

impl PdfPage {
    /// Crea una anotación vacía del tipo dado (sin rectángulo ni contenido).
    pub fn create_annotation(&mut self, subtype: PdfAnnotationType) -> Result<PdfAnnotation> {
        self.pdf_mut()
            .create_annotation(annot_type_to(subtype))
            .map(PdfAnnotation::wrap)
            .map_err(err)
    }

    pub fn add_text_annotation(&mut self, rect: Rect, contents: &str) -> Result<PdfAnnotation> {
        self.pdf_mut()
            .add_text_annotation(rect_to(rect), contents)
            .map(PdfAnnotation::wrap)
            .map_err(err)
    }

    pub fn add_free_text_annotation(&mut self, rect: Rect, contents: &str) -> Result<PdfAnnotation> {
        self.pdf_mut()
            .add_free_text_annotation(rect_to(rect), contents)
            .map(PdfAnnotation::wrap)
            .map_err(err)
    }

    pub fn add_caret_annotation(&mut self, rect: Rect) -> Result<PdfAnnotation> {
        self.pdf_mut()
            .add_caret_annotation(rect_to(rect))
            .map(PdfAnnotation::wrap)
            .map_err(err)
    }

    pub fn add_line_annotation(&mut self, start: Point, end: Point) -> Result<PdfAnnotation> {
        self.pdf_mut()
            .add_line_annotation(point_to(start), point_to(end))
            .map(PdfAnnotation::wrap)
            .map_err(err)
    }

    pub fn add_square_annotation(&mut self, rect: Rect) -> Result<PdfAnnotation> {
        self.pdf_mut()
            .add_square_annotation(rect_to(rect))
            .map(PdfAnnotation::wrap)
            .map_err(err)
    }

    pub fn add_circle_annotation(&mut self, rect: Rect) -> Result<PdfAnnotation> {
        self.pdf_mut()
            .add_circle_annotation(rect_to(rect))
            .map(PdfAnnotation::wrap)
            .map_err(err)
    }

    pub fn add_polygon_annotation(&mut self, points: &[Point]) -> Result<PdfAnnotation> {
        self.pdf_mut()
            .add_polygon_annotation(points.iter().copied().map(point_to).collect::<Vec<_>>())
            .map(PdfAnnotation::wrap)
            .map_err(err)
    }

    pub fn add_polyline_annotation(&mut self, points: &[Point]) -> Result<PdfAnnotation> {
        self.pdf_mut()
            .add_polyline_annotation(points.iter().copied().map(point_to).collect::<Vec<_>>())
            .map(PdfAnnotation::wrap)
            .map_err(err)
    }

    pub fn add_ink_annotation(&mut self, strokes: &[Vec<Point>]) -> Result<PdfAnnotation> {
        self.pdf_mut()
            .add_ink_annotation(
                strokes
                    .iter()
                    .map(|s| s.iter().copied().map(point_to).collect::<Vec<_>>()),
            )
            .map(PdfAnnotation::wrap)
            .map_err(err)
    }

    pub fn add_highlight_annotation(&mut self, quads: &[Quad]) -> Result<PdfAnnotation> {
        self.pdf_mut()
            .add_highlight_annotation(quads.iter().copied().map(quad_to).collect::<Vec<_>>())
            .map(PdfAnnotation::wrap)
            .map_err(err)
    }

    pub fn add_underline_annotation(&mut self, quads: &[Quad]) -> Result<PdfAnnotation> {
        self.pdf_mut()
            .add_underline_annotation(quads.iter().copied().map(quad_to).collect::<Vec<_>>())
            .map(PdfAnnotation::wrap)
            .map_err(err)
    }

    pub fn add_squiggly_annotation(&mut self, quads: &[Quad]) -> Result<PdfAnnotation> {
        self.pdf_mut()
            .add_squiggly_annotation(quads.iter().copied().map(quad_to).collect::<Vec<_>>())
            .map(PdfAnnotation::wrap)
            .map_err(err)
    }

    pub fn add_strikeout_annotation(&mut self, quads: &[Quad]) -> Result<PdfAnnotation> {
        self.pdf_mut()
            .add_strikeout_annotation(quads.iter().copied().map(quad_to).collect::<Vec<_>>())
            .map(PdfAnnotation::wrap)
            .map_err(err)
    }

    pub fn add_stamp_annotation(&mut self, rect: Rect, icon_name: &str) -> Result<PdfAnnotation> {
        self.pdf_mut()
            .add_stamp_annotation(rect_to(rect), icon_name)
            .map(PdfAnnotation::wrap)
            .map_err(err)
    }

    /// Marca un área para redactar (se aplica con [`PdfPage::redact`]).
    pub fn add_redact_annotation(&mut self, rect: Rect) -> Result<PdfAnnotation> {
        self.pdf_mut()
            .add_redact_annotation(rect_to(rect))
            .map(PdfAnnotation::wrap)
            .map_err(err)
    }

    pub fn add_redact_annotation_quads(&mut self, quads: &[Quad]) -> Result<PdfAnnotation> {
        self.pdf_mut()
            .add_redact_annotation(quads.iter().copied().map(quad_to).collect::<Vec<_>>())
            .map(PdfAnnotation::wrap)
            .map_err(err)
    }

    pub fn delete_annotation(&mut self, annot: PdfAnnotation) -> Result<()> {
        self.pdf_mut().delete_annotation(annot.into_inner()?).map_err(err)
    }

    /// Anotaciones de la página (los enlaces y los campos de formulario también son anotaciones).
    pub fn annotations(&self) -> std::vec::IntoIter<PdfAnnotation> {
        self.pdf()
            .annotations()
            .map(PdfAnnotation::wrap)
            .collect::<Vec<_>>()
            .into_iter()
    }

    /// Aplica todas las redacciones de la página: **elimina** del archivo el contenido bajo las áreas marcadas,
    /// según `options`. `true` si cambió algo.
    pub fn redact(&mut self, options: RedactOptions) -> Result<bool> {
        self.pdf_mut().redact_with_options(redact_to(options)).map_err(err)
    }
}

/// Comprueba que una anotación pertenece a la página dada (por su número de objeto).
#[allow(dead_code)]
pub(crate) fn require_attached(a: &PdfAnnotation) -> Result<()> {
    a.xref()
        .map(|_| ())
        .map_err(|_| Error::InvalidInput("la anotación ya no está en la página".into()))
}
