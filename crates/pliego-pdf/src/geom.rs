//! Geometría propia: puntos, rectángulos, cuadriláteros, matrices y tamaños.
//!
//! Los nombres y campos reproducen los de MuPDF para que la migración sea mecánica, pero son tipos de Pliego:
//! ningún motor aparece en la API pública. Todo en puntos PDF (1/72 de pulgada) salvo `IRect` (píxeles).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn transform(self, m: &Matrix) -> Self {
        let (x, y) = m.transform_xy(self.x, self.y);
        Self { x, y }
    }

    pub fn length(self) -> f32 {
        self.x.hypot(self.y)
    }
}

/// Rectángulo `[x0, y0] – [x1, y1]`. Vacío si `x0 >= x1` o `y0 >= y1`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

impl Rect {
    pub const fn new(x0: f32, y0: f32, x1: f32, y1: f32) -> Self {
        Self { x0, y0, x1, y1 }
    }

    pub fn width(&self) -> f32 {
        self.x1 - self.x0
    }

    pub fn height(&self) -> f32 {
        self.y1 - self.y0
    }

    pub fn is_empty(&self) -> bool {
        self.x0 >= self.x1 || self.y0 >= self.y1
    }

    pub fn contains(&self, x: f32, y: f32) -> bool {
        !self.is_empty() && x >= self.x0 && x < self.x1 && y >= self.y0 && y < self.y1
    }

    pub fn origin(&self) -> Point {
        Point::new(self.x0, self.y0)
    }

    pub fn size(&self) -> Size {
        Size::new(self.width(), self.height())
    }

    pub fn quad(&self) -> Quad {
        Quad::from(*self)
    }

    /// Rectángulo mínimo que contiene a ambos. Un rectángulo vacío no aporta nada.
    pub fn union(&self, other: &Rect) -> Rect {
        if self.is_empty() {
            return *other;
        }
        if other.is_empty() {
            return *self;
        }
        Rect::new(
            self.x0.min(other.x0),
            self.y0.min(other.y0),
            self.x1.max(other.x1),
            self.y1.max(other.y1),
        )
    }

    /// Intersección; si no se tocan devuelve un rectángulo vacío.
    pub fn intersect(&self, other: &Rect) -> Rect {
        let r = Rect::new(
            self.x0.max(other.x0),
            self.y0.max(other.y0),
            self.x1.min(other.x1),
            self.y1.min(other.y1),
        );
        if r.is_empty() { Rect::default() } else { r }
    }

    pub fn translate(&self, dx: f32, dy: f32) -> Rect {
        Rect::new(self.x0 + dx, self.y0 + dy, self.x1 + dx, self.y1 + dy)
    }

    /// Rectángulo que contiene al transformado (caja envolvente de las cuatro esquinas).
    pub fn transform(&self, m: &Matrix) -> Rect {
        let q = self.quad();
        let pts = [q.ul, q.ur, q.ll, q.lr].map(|p| p.transform(m));
        let xs = pts.map(|p| p.x);
        let ys = pts.map(|p| p.y);
        Rect::new(
            xs.iter().copied().fold(f32::INFINITY, f32::min),
            ys.iter().copied().fold(f32::INFINITY, f32::min),
            xs.iter().copied().fold(f32::NEG_INFINITY, f32::max),
            ys.iter().copied().fold(f32::NEG_INFINITY, f32::max),
        )
    }

    /// Redondeo hacia fuera a píxeles enteros.
    pub fn round(&self) -> IRect {
        IRect::new(
            self.x0.floor() as i32,
            self.y0.floor() as i32,
            self.x1.ceil() as i32,
            self.y1.ceil() as i32,
        )
    }
}

impl From<Quad> for Rect {
    fn from(q: Quad) -> Self {
        let xs = [q.ul.x, q.ur.x, q.ll.x, q.lr.x];
        let ys = [q.ul.y, q.ur.y, q.ll.y, q.lr.y];
        Rect::new(
            xs.iter().copied().fold(f32::INFINITY, f32::min),
            ys.iter().copied().fold(f32::INFINITY, f32::min),
            xs.iter().copied().fold(f32::NEG_INFINITY, f32::max),
            ys.iter().copied().fold(f32::NEG_INFINITY, f32::max),
        )
    }
}

/// Rectángulo de píxeles.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IRect {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

impl IRect {
    pub const fn new(x0: i32, y0: i32, x1: i32, y1: i32) -> Self {
        Self { x0, y0, x1, y1 }
    }

    pub fn width(&self) -> i32 {
        self.x1 - self.x0
    }

    pub fn height(&self) -> i32 {
        self.y1 - self.y0
    }

    pub fn is_empty(&self) -> bool {
        self.x0 >= self.x1 || self.y0 >= self.y1
    }
}

/// Cuadrilátero (esquinas superior/inferior izquierda/derecha del texto, ya con su orientación).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Quad {
    pub ul: Point,
    pub ur: Point,
    pub ll: Point,
    pub lr: Point,
}

impl Quad {
    pub const fn new(ul: Point, ur: Point, ll: Point, lr: Point) -> Self {
        Self { ul, ur, ll, lr }
    }

    /// Ocho coordenadas en el orden `[ul.x, ul.y, ur.x, ur.y, ll.x, ll.y, lr.x, lr.y]` (el de `/QuadPoints` y
    /// `SearchHit`).
    pub fn to_array(&self) -> [f32; 8] {
        [
            self.ul.x, self.ul.y, self.ur.x, self.ur.y, self.ll.x, self.ll.y, self.lr.x, self.lr.y,
        ]
    }

    pub fn transform(&self, m: &Matrix) -> Quad {
        Quad::new(
            self.ul.transform(m),
            self.ur.transform(m),
            self.ll.transform(m),
            self.lr.transform(m),
        )
    }
}

impl From<Rect> for Quad {
    fn from(r: Rect) -> Self {
        Quad::new(
            Point::new(r.x0, r.y0),
            Point::new(r.x1, r.y0),
            Point::new(r.x0, r.y1),
            Point::new(r.x1, r.y1),
        )
    }
}

/// Matriz afín `[a b c d e f]` de PDF: `x' = a·x + c·y + e`, `y' = b·x + d·y + f`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Matrix {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub e: f32,
    pub f: f32,
}

impl Default for Matrix {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Matrix {
    pub const IDENTITY: Matrix = Matrix::new(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);

    pub const fn new(a: f32, b: f32, c: f32, d: f32, e: f32, f: f32) -> Self {
        Self { a, b, c, d, e, f }
    }

    pub const fn new_scale(sx: f32, sy: f32) -> Self {
        Self::new(sx, 0.0, 0.0, sy, 0.0, 0.0)
    }

    pub const fn new_translate(tx: f32, ty: f32) -> Self {
        Self::new(1.0, 0.0, 0.0, 1.0, tx, ty)
    }

    /// Giro antihorario (en el sistema de coordenadas de PDF) de `degrees` grados.
    pub fn new_rotate(degrees: f32) -> Self {
        let mut deg = degrees;
        while deg < 0.0 {
            deg += 360.0;
        }
        while deg >= 360.0 {
            deg -= 360.0;
        }
        // Ángulos rectos exactos para no arrastrar error de redondeo.
        let (s, c) = if (deg - 90.0).abs() < 1e-4 {
            (1.0, 0.0)
        } else if (deg - 180.0).abs() < 1e-4 {
            (0.0, -1.0)
        } else if (deg - 270.0).abs() < 1e-4 {
            (-1.0, 0.0)
        } else {
            let r = deg.to_radians();
            (r.sin(), r.cos())
        };
        Self::new(c, s, -s, c, 0.0, 0.0)
    }

    /// `self = self × m` (primero `self`, después `m`), como `fz_concat`. Devuelve `self` para encadenar.
    pub fn concat(&mut self, m: Matrix) -> &mut Self {
        let r = Matrix::new(
            self.a * m.a + self.b * m.c,
            self.a * m.b + self.b * m.d,
            self.c * m.a + self.d * m.c,
            self.c * m.b + self.d * m.d,
            self.e * m.a + self.f * m.c + m.e,
            self.e * m.b + self.f * m.d + m.f,
        );
        *self = r;
        self
    }

    /// `None` si la matriz es singular. Cálculo intermedio en doble precisión, como MuPDF.
    pub fn invert(&self) -> Option<Matrix> {
        let (a, b, c, d) = (self.a as f64, self.b as f64, self.c as f64, self.d as f64);
        let det = a * d - b * c;
        if det.abs() <= f64::EPSILON {
            return None;
        }
        let rdet = 1.0 / det;
        let (ia, ib, ic, id) = (d * rdet, -b * rdet, -c * rdet, a * rdet);
        let ie = -(self.e as f64) * ia - (self.f as f64) * ic;
        let iff = -(self.e as f64) * ib - (self.f as f64) * id;
        Some(Matrix::new(
            ia as f32, ib as f32, ic as f32, id as f32, ie as f32, iff as f32,
        ))
    }

    pub fn transform_xy(&self, x: f32, y: f32) -> (f32, f32) {
        (x * self.a + y * self.c + self.e, x * self.b + y * self.d + self.f)
    }
}

/// Anchura y altura en puntos.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub const A3: Size = Size::new(842.0, 1191.0);
    pub const A4: Size = Size::new(595.0, 842.0);
    pub const A5: Size = Size::new(420.0, 595.0);
    pub const LEGAL: Size = Size::new(612.0, 1008.0);
    pub const LETTER: Size = Size::new(612.0, 792.0);

    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    pub fn to_rect(&self) -> Rect {
        Rect::new(0.0, 0.0, self.width, self.height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concat_order_matches_pdf() {
        // Escalar x2 y después trasladar (10, 0): el origen acaba en (10, 0) y (1, 0) en (12, 0).
        let mut m = Matrix::new_scale(2.0, 2.0);
        m.concat(Matrix::new_translate(10.0, 0.0));
        assert_eq!(m.transform_xy(1.0, 0.0), (12.0, 0.0));
    }

    #[test]
    fn invert_roundtrip() {
        let mut m = Matrix::new_rotate(90.0);
        m.concat(Matrix::new_translate(5.0, 7.0));
        let inv = m.invert().unwrap();
        let (x, y) = m.transform_xy(3.0, 4.0);
        let (bx, by) = inv.transform_xy(x, y);
        assert!((bx - 3.0).abs() < 1e-4 && (by - 4.0).abs() < 1e-4);
        assert!(Matrix::new(0.0, 0.0, 0.0, 0.0, 1.0, 1.0).invert().is_none());
    }

    #[test]
    fn rect_ops() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(5.0, 5.0, 20.0, 20.0);
        assert_eq!(a.union(&b), Rect::new(0.0, 0.0, 20.0, 20.0));
        assert_eq!(a.intersect(&b), Rect::new(5.0, 5.0, 10.0, 10.0));
        assert!(a.intersect(&Rect::new(11.0, 11.0, 12.0, 12.0)).is_empty());
        assert_eq!(Rect::new(0.5, 0.5, 2.2, 2.2).round(), IRect::new(0, 0, 3, 3));
        let r = a.transform(&Matrix::new_rotate(90.0));
        assert!((r.width() - 10.0).abs() < 1e-4);
    }

    #[test]
    fn quad_rect_roundtrip() {
        let r = Rect::new(1.0, 2.0, 3.0, 4.0);
        assert_eq!(Rect::from(r.quad()), r);
        assert_eq!(r.quad().to_array(), [1.0, 2.0, 3.0, 2.0, 1.0, 4.0, 3.0, 4.0]);
    }
}
