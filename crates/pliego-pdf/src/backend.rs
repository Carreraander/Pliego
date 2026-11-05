//! Conversiones entre los tipos propios y los de MuPDF. **Es el único sitio donde se mezclan** (junto con los
//! módulos que envuelven un tipo de MuPDF). Cuando cambie el motor, esto desaparece o se reduce.

use pliego_core::Error;

use crate::geom::{IRect, Matrix, Point, Quad, Rect, Size};

/// Convierte un error de MuPDF en el error común de Pliego.
pub(crate) fn err(e: mupdf::Error) -> Error {
    Error::Pdf(e.to_string())
}

pub(crate) fn rect_to(r: Rect) -> mupdf::Rect {
    mupdf::Rect::new(r.x0, r.y0, r.x1, r.y1)
}

pub(crate) fn rect_from(r: mupdf::Rect) -> Rect {
    Rect::new(r.x0, r.y0, r.x1, r.y1)
}

pub(crate) fn irect_to(r: IRect) -> mupdf::IRect {
    mupdf::IRect {
        x0: r.x0,
        y0: r.y0,
        x1: r.x1,
        y1: r.y1,
    }
}

pub(crate) fn point_to(p: Point) -> mupdf::Point {
    mupdf::Point::new(p.x, p.y)
}

pub(crate) fn point_from(p: mupdf::Point) -> Point {
    Point::new(p.x, p.y)
}

pub(crate) fn quad_to(q: Quad) -> mupdf::Quad {
    mupdf::Quad::new(point_to(q.ul), point_to(q.ur), point_to(q.ll), point_to(q.lr))
}

pub(crate) fn quad_from(q: mupdf::Quad) -> Quad {
    Quad::new(point_from(q.ul), point_from(q.ur), point_from(q.ll), point_from(q.lr))
}

pub(crate) fn matrix_to(m: &Matrix) -> mupdf::Matrix {
    mupdf::Matrix::new(m.a, m.b, m.c, m.d, m.e, m.f)
}

pub(crate) fn matrix_from(m: mupdf::Matrix) -> Matrix {
    Matrix::new(m.a, m.b, m.c, m.d, m.e, m.f)
}

pub(crate) fn size_to(s: Size) -> mupdf::Size {
    mupdf::Size::new(s.width, s.height)
}

/// MuPDF recibe rutas UTF-8 (en Windows `AsRef<FilePath>` no está implementado para `Path`).
pub(crate) fn path_str(path: &std::path::Path) -> pliego_core::Result<&str> {
    path.to_str()
        .ok_or_else(|| Error::InvalidInput(format!("ruta no representable en UTF-8: {}", path.display())))
}
