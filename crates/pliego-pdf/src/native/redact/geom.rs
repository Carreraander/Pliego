//! Geometría en doble precisión para la redacción (espacio de usuario del PDF, origen abajo a la izquierda).

/// Matriz PDF `[a b c d e f]`: `x' = a·x + c·y + e`, `y' = b·x + d·y + f`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct M {
    pub(super) a: f64,
    pub(super) b: f64,
    pub(super) c: f64,
    pub(super) d: f64,
    pub(super) e: f64,
    pub(super) f: f64,
}

impl M {
    pub(super) const IDENT: M = M {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        e: 0.0,
        f: 0.0,
    };

    pub(super) fn from_slice(v: &[f64]) -> Option<M> {
        (v.len() >= 6).then(|| M {
            a: v[0],
            b: v[1],
            c: v[2],
            d: v[3],
            e: v[4],
            f: v[5],
        })
    }

    pub(super) fn translate(tx: f64, ty: f64) -> M {
        M {
            e: tx,
            f: ty,
            ..M::IDENT
        }
    }

    /// Aplica `self` y después `o` (el orden de `cm`: `nueva.then(ctm)`).
    pub(super) fn then(self, o: M) -> M {
        M {
            a: self.a * o.a + self.b * o.c,
            b: self.a * o.b + self.b * o.d,
            c: self.c * o.a + self.d * o.c,
            d: self.c * o.b + self.d * o.d,
            e: self.e * o.a + self.f * o.c + o.e,
            f: self.e * o.b + self.f * o.d + o.f,
        }
    }

    pub(super) fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        (self.a * x + self.c * y + self.e, self.b * x + self.d * y + self.f)
    }

    pub(super) fn invert(&self) -> Option<M> {
        let det = self.a * self.d - self.b * self.c;
        if det.abs() < 1e-12 || !det.is_finite() {
            return None;
        }
        let (a, b, c, d) = (self.d / det, -self.b / det, -self.c / det, self.a / det);
        Some(M {
            a,
            b,
            c,
            d,
            e: -(self.e * a + self.f * c),
            f: -(self.e * b + self.f * d),
        })
    }

    /// Factor de escala lineal medio.
    pub(super) fn scale(&self) -> f64 {
        (self.a * self.d - self.b * self.c).abs().sqrt()
    }
}

/// Rectángulo normalizado (`x0 <= x1`, `y0 <= y1`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct R {
    pub(super) x0: f64,
    pub(super) y0: f64,
    pub(super) x1: f64,
    pub(super) y1: f64,
}

impl R {
    pub(super) fn new(xa: f64, ya: f64, xb: f64, yb: f64) -> R {
        R {
            x0: xa.min(xb),
            y0: ya.min(yb),
            x1: xa.max(xb),
            y1: ya.max(yb),
        }
    }

    /// Caja envolvente de los puntos.
    pub(super) fn of_points(pts: &[(f64, f64)]) -> R {
        let mut r = R {
            x0: f64::MAX,
            y0: f64::MAX,
            x1: f64::MIN,
            y1: f64::MIN,
        };
        for &(x, y) in pts {
            r.x0 = r.x0.min(x);
            r.y0 = r.y0.min(y);
            r.x1 = r.x1.max(x);
            r.y1 = r.y1.max(y);
        }
        r
    }

    pub(super) fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x0 && x <= self.x1 && y >= self.y0 && y <= self.y1
    }

    /// Solapan (con área o tocándose por un borde: las cajas degeneradas también cuentan).
    pub(super) fn intersects(&self, o: &R) -> bool {
        self.x0 <= o.x1 && self.x1 >= o.x0 && self.y0 <= o.y1 && self.y1 >= o.y0
    }

    /// `self` está dentro de `o`.
    pub(super) fn inside(&self, o: &R) -> bool {
        self.x0 >= o.x0 && self.x1 <= o.x1 && self.y0 >= o.y0 && self.y1 <= o.y1
    }

    pub(super) fn inflate(&self, d: f64) -> R {
        R {
            x0: self.x0 - d,
            y0: self.y0 - d,
            x1: self.x1 + d,
            y1: self.y1 + d,
        }
    }

    /// Rectángulo transformado (caja envolvente de las cuatro esquinas).
    pub(super) fn transform(&self, m: &M) -> R {
        R::of_points(&[
            m.apply(self.x0, self.y0),
            m.apply(self.x1, self.y0),
            m.apply(self.x0, self.y1),
            m.apply(self.x1, self.y1),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn then_follows_pdf_order_and_inverts() {
        let scale = M {
            a: 2.0,
            d: 2.0,
            ..M::IDENT
        };
        let m = M::translate(10.0, 0.0).then(scale);
        assert_eq!(m.apply(1.0, 1.0), (22.0, 2.0));
        let inv = m.invert().unwrap();
        let (x, y) = inv.apply(22.0, 2.0);
        assert!((x - 1.0).abs() < 1e-9 && (y - 1.0).abs() < 1e-9);
    }
}
