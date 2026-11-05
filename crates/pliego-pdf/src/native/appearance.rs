//! Flujos de apariencia (`/AP /N`) de las anotaciones, escritos a partir de la especificación PDF (ISO 32000-1,
//! §12.5.5 y §12.5.6) y no de ninguna otra implementación.
//!
//! Todo aquí son **funciones puras**: reciben las propiedades de la anotación en el espacio de usuario del PDF y
//! devuelven un [`Ap`] (rectángulo del formulario, operadores de contenido y recursos que necesita). Quien llama
//! (`annot.rs`) lo convierte en un objeto de flujo.

use std::fmt::Write as _;

use crate::geom::{Point, Rect};

/// Una apariencia lista para escribirse como XObject de formulario.
#[derive(Debug, Clone, Default)]
pub(crate) struct Ap {
    /// `/BBox` del formulario (espacio de usuario, sin `/Matrix`: el formulario y la anotación coinciden).
    pub bbox: Rect,
    pub content: String,
    /// Estado gráfico `/H`: opacidad de trazo, de relleno y si mezcla con `Multiply`.
    pub gs: Option<Gs>,
    /// Fuentes de las que depende el contenido: (nombre de recurso, `BaseFont` de las 14 estándar).
    pub fonts: Vec<(String, &'static str)>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Gs {
    pub stroke: f32,
    pub fill: f32,
    pub multiply: bool,
}

/// Propiedades de línea y color compartidas por la mayoría de las anotaciones.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Style {
    pub color: Option<Vec<f32>>,
    pub interior: Option<Vec<f32>>,
    pub width: f32,
    /// `None` = continuo; `Some` = discontinuo con esa lista (`/BS /D`).
    pub dash: Option<Vec<f32>>,
    pub opacity: f32,
}

impl Style {
    pub(crate) fn gs(&self, multiply: bool) -> Option<Gs> {
        if multiply || (self.opacity - 1.0).abs() > 1e-4 {
            Some(Gs {
                stroke: self.opacity,
                fill: self.opacity,
                multiply,
            })
        } else {
            None
        }
    }
}

/// Número con pocos decimales y sin ceros sobrantes (`1`, `0.5`, `-3.25`).
pub(crate) fn n(v: f32) -> String {
    if !v.is_finite() {
        return "0".into();
    }
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-" || s == "-0" {
        "0".into()
    } else {
        s.to_string()
    }
}

/// Operador de color (`g`, `rg`, `k` o `G`, `RG`, `K`) para 1, 3 o 4 componentes.
pub(crate) fn color_op(c: &[f32], stroke: bool) -> String {
    let (g, rg, k) = if stroke { ("G", "RG", "K") } else { ("g", "rg", "k") };
    let v: Vec<String> = c.iter().map(|x| n(*x)).collect();
    match c.len() {
        1 => format!("{} {g}\n", v[0]),
        3 => format!("{} {} {} {rg}\n", v[0], v[1], v[2]),
        4 => format!("{} {} {} {} {k}\n", v[0], v[1], v[2], v[3]),
        _ => String::new(),
    }
}

fn dash_op(style: &Style) -> String {
    match &style.dash {
        Some(d) if !d.is_empty() => {
            let items: Vec<String> = d.iter().map(|x| n(*x)).collect();
            format!("[{}] 0 d\n", items.join(" "))
        }
        _ => String::new(),
    }
}

fn bbox_of(points: &[Point]) -> Rect {
    let mut r = Rect::new(f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
    for p in points {
        r.x0 = r.x0.min(p.x);
        r.y0 = r.y0.min(p.y);
        r.x1 = r.x1.max(p.x);
        r.y1 = r.y1.max(p.y);
    }
    if r.x0 > r.x1 { Rect::new(0.0, 0.0, 0.0, 0.0) } else { r }
}

pub(crate) fn expand(r: Rect, d: f32) -> Rect {
    Rect::new(r.x0 - d, r.y0 - d, r.x1 + d, r.y1 + d)
}

/// Un cuadrilátero de texto: `[ul, ur, ll, lr]`, el orden de `/QuadPoints` de los programas habituales.
pub(crate) type Q = [Point; 4];

pub(crate) fn quads_bbox(quads: &[Q]) -> Rect {
    let pts: Vec<Point> = quads.iter().flatten().copied().collect();
    bbox_of(&pts)
}

fn lerp(a: Point, b: Point, t: f32) -> Point {
    Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}

fn unit(a: Point, b: Point) -> (f32, f32) {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let l = (dx * dx + dy * dy).sqrt();
    if l < 1e-6 { (1.0, 0.0) } else { (dx / l, dy / l) }
}

// --- Marcado de texto -------------------------------------------------------------------------------------------

/// Resaltado: cada cuadrilátero relleno con el color y mezcla `Multiply` (el texto se sigue leyendo debajo).
pub(crate) fn highlight(quads: &[Q], color: &[f32], opacity: f32) -> Ap {
    let mut c = String::from("/H gs\n");
    c.push_str(&color_op(color, false));
    for q in quads {
        // Orden de recorrido: ul → ur → lr → ll.
        let _ = writeln!(
            c,
            "{} {} m\n{} {} l\n{} {} l\n{} {} l\nh\nf",
            n(q[0].x),
            n(q[0].y),
            n(q[1].x),
            n(q[1].y),
            n(q[3].x),
            n(q[3].y),
            n(q[2].x),
            n(q[2].y)
        );
    }
    Ap {
        bbox: quads_bbox(quads),
        content: c,
        gs: Some(Gs {
            stroke: opacity,
            fill: opacity,
            multiply: true,
        }),
        fonts: Vec::new(),
    }
}

/// Subrayado, tachado u ondulado: una línea (o zigzag) por cuadrilátero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LineMark {
    Underline,
    StrikeOut,
    Squiggly,
}

pub(crate) fn line_mark(kind: LineMark, quads: &[Q], color: &[f32], opacity: f32) -> Ap {
    let width = 1.0f32;
    let mut c = String::new();
    c.push_str(&color_op(color, true));
    let _ = writeln!(c, "{} w", n(width));
    let mut all = Vec::new();
    for q in quads {
        let (ul, ur, ll, lr) = (q[0], q[1], q[2], q[3]);
        all.extend_from_slice(q);
        match kind {
            LineMark::Underline => {
                // Un poco por encima del borde inferior.
                let up = unit(ll, ul);
                let off = width * 1.2;
                let (a, b) = (
                    Point::new(ll.x + up.0 * off, ll.y + up.1 * off),
                    Point::new(lr.x + up.0 * off, lr.y + up.1 * off),
                );
                let _ = writeln!(c, "{} {} m\n{} {} l\nS", n(a.x), n(a.y), n(b.x), n(b.y));
            }
            LineMark::StrikeOut => {
                let (a, b) = (lerp(ll, ul, 0.5), lerp(lr, ur, 0.5));
                let _ = writeln!(c, "{} {} m\n{} {} l\nS", n(a.x), n(a.y), n(b.x), n(b.y));
            }
            LineMark::Squiggly => {
                let along = unit(ll, lr);
                let up = unit(ll, ul);
                let len = ((lr.x - ll.x).powi(2) + (lr.y - ll.y).powi(2)).sqrt();
                let (step, amp) = (2.0f32, 2.0f32);
                let steps = (len / step).ceil().max(1.0) as usize;
                for i in 0..=steps {
                    let d = (i as f32 * step).min(len);
                    let h = if i % 2 == 1 { amp } else { 0.0 } + width;
                    let p = Point::new(ll.x + along.0 * d + up.0 * h, ll.y + along.1 * d + up.1 * h);
                    let _ = writeln!(c, "{} {} {}", n(p.x), n(p.y), if i == 0 { "m" } else { "l" });
                }
                c.push_str("S\n");
            }
        }
    }
    let pad = match kind {
        LineMark::Squiggly => 4.0,
        _ => 1.5,
    };
    let style = Style {
        color: None,
        interior: None,
        width,
        dash: None,
        opacity,
    };
    Ap {
        bbox: expand(bbox_of(&all), pad),
        content: c,
        gs: style.gs(false).map(|mut g| {
            g.multiply = false;
            g
        }),
        fonts: Vec::new(),
    }
    .with_gs_prefix()
}

impl Ap {
    /// Si hay estado gráfico, el contenido empieza activándolo.
    fn with_gs_prefix(mut self) -> Self {
        if self.gs.is_some() && !self.content.starts_with("/H gs") {
            self.content = format!("/H gs\n{}", self.content);
        }
        self
    }
}

// --- Figuras -------------------------------------------------------------------------------------------------------

fn paint_op(fill: bool, stroke: bool, close: bool) -> &'static str {
    match (fill, stroke, close) {
        (true, true, true) => "b",
        (true, true, false) => "B",
        (true, false, true) => "h f",
        (true, false, false) => "f",
        (false, true, true) => "s",
        (false, true, false) => "S",
        (false, false, _) => "n",
    }
}

fn start(style: &Style) -> (String, bool, bool) {
    let mut c = String::new();
    let stroke = style.width > 0.0 && style.color.as_ref().is_some_and(|c| !c.is_empty());
    let fill = style.interior.as_ref().is_some_and(|c| !c.is_empty());
    if stroke {
        let _ = writeln!(c, "{} w", n(style.width));
        c.push_str(&dash_op(style));
        c.push_str(&color_op(style.color.as_deref().unwrap_or(&[]), true));
    }
    if fill {
        c.push_str(&color_op(style.interior.as_deref().unwrap_or(&[]), false));
    }
    (c, stroke, fill)
}

fn with_gs(style: &Style, c: String) -> (String, Option<Gs>) {
    let gs = style.gs(false);
    if gs.is_some() {
        (format!("/H gs\n{c}"), gs)
    } else {
        (c, None)
    }
}

/// Cuadrado o rectángulo: `inner` es el rectángulo que traza la línea (el `/Rect` menos `/RD`).
pub(crate) fn square(inner: Rect, style: &Style, bbox: Rect) -> Ap {
    let (mut c, stroke, fill) = start(style);
    let _ = writeln!(
        c,
        "{} {} {} {} re\n{}",
        n(inner.x0),
        n(inner.y0),
        n(inner.width()),
        n(inner.height()),
        paint_op(fill, stroke, false)
    );
    let (content, gs) = with_gs(style, c);
    Ap {
        bbox,
        content,
        gs,
        fonts: Vec::new(),
    }
}

const KAPPA: f32 = 0.552_284_7;

fn ellipse_path(r: Rect) -> String {
    let (cx, cy) = ((r.x0 + r.x1) / 2.0, (r.y0 + r.y1) / 2.0);
    let (rx, ry) = (r.width() / 2.0, r.height() / 2.0);
    let (kx, ky) = (rx * KAPPA, ry * KAPPA);
    let mut c = String::new();
    let _ = writeln!(c, "{} {} m", n(cx), n(cy + ry));
    let _ = writeln!(
        c,
        "{} {} {} {} {} {} c",
        n(cx + kx),
        n(cy + ry),
        n(cx + rx),
        n(cy + ky),
        n(cx + rx),
        n(cy)
    );
    let _ = writeln!(
        c,
        "{} {} {} {} {} {} c",
        n(cx + rx),
        n(cy - ky),
        n(cx + kx),
        n(cy - ry),
        n(cx),
        n(cy - ry)
    );
    let _ = writeln!(
        c,
        "{} {} {} {} {} {} c",
        n(cx - kx),
        n(cy - ry),
        n(cx - rx),
        n(cy - ky),
        n(cx - rx),
        n(cy)
    );
    let _ = writeln!(
        c,
        "{} {} {} {} {} {} c",
        n(cx - rx),
        n(cy + ky),
        n(cx - kx),
        n(cy + ry),
        n(cx),
        n(cy + ry)
    );
    c
}

pub(crate) fn circle(inner: Rect, style: &Style, bbox: Rect) -> Ap {
    let (mut c, stroke, fill) = start(style);
    c.push_str(&ellipse_path(inner));
    c.push_str(paint_op(fill, stroke, true));
    c.push('\n');
    let (content, gs) = with_gs(style, c);
    Ap {
        bbox,
        content,
        gs,
        fonts: Vec::new(),
    }
}

/// Polígono (cerrado, con relleno posible) o polilínea (abierta).
pub(crate) fn poly(points: &[Point], closed: bool, style: &Style, bbox: Rect) -> Ap {
    let (mut c, stroke, fill) = start(style);
    for (i, p) in points.iter().enumerate() {
        let _ = writeln!(c, "{} {} {}", n(p.x), n(p.y), if i == 0 { "m" } else { "l" });
    }
    c.push_str(paint_op(fill && closed, stroke, closed));
    c.push('\n');
    let (content, gs) = with_gs(style, c);
    Ap {
        bbox,
        content,
        gs,
        fonts: Vec::new(),
    }
}

pub(crate) fn poly_bbox(points: &[Point]) -> Rect {
    bbox_of(points)
}

/// Lápiz: trazos suaves con extremos y uniones redondeados.
pub(crate) fn ink(strokes: &[Vec<Point>], style: &Style) -> Ap {
    let mut c = String::from("1 J\n1 j\n");
    let _ = writeln!(c, "{} w", n(style.width.max(0.1)));
    c.push_str(&dash_op(style));
    c.push_str(&color_op(style.color.as_deref().unwrap_or(&[1.0, 0.0, 0.0]), true));
    let mut all = Vec::new();
    for s in strokes {
        if s.is_empty() {
            continue;
        }
        all.extend_from_slice(s);
        for (i, p) in s.iter().enumerate() {
            let _ = writeln!(c, "{} {} {}", n(p.x), n(p.y), if i == 0 { "m" } else { "l" });
        }
        if s.len() == 1 {
            // Un punto: se dibuja como un segmento de longitud cero con extremo redondo.
            let _ = writeln!(c, "{} {} l", n(s[0].x), n(s[0].y));
        }
        c.push_str("S\n");
    }
    let (content, gs) = with_gs(style, c);
    Ap {
        bbox: expand(bbox_of(&all), style.width / 2.0 + 1.0),
        content,
        gs,
        fonts: Vec::new(),
    }
}

// --- Línea con terminaciones -----------------------------------------------------------------------------------

/// Terminaciones de línea (`/LE`), con los nombres de la especificación.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ending {
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

fn ending_path(kind: Ending, p: Point, outward: (f32, f32), w: f32, out: &mut Vec<Point>) -> (String, bool) {
    // Devuelve (operadores de camino, cerrado). `out` recoge los puntos para el cálculo del rectángulo.
    let r = 3.0 * w.max(1.0);
    let (dx, dy) = outward;
    let (nx, ny) = (-dy, dx);
    let pt = |a: f32, b: f32| Point::new(p.x + dx * a + nx * b, p.y + dy * a + ny * b);
    let mut s = String::new();
    let mut put = |pts: &[Point], close: bool, out: &mut Vec<Point>| {
        for (i, q) in pts.iter().enumerate() {
            let _ = writeln!(s, "{} {} {}", n(q.x), n(q.y), if i == 0 { "m" } else { "l" });
            out.push(*q);
        }
        close
    };
    let closed = match kind {
        Ending::None => return (String::new(), false),
        Ending::Square => put(&[pt(-r, -r), pt(r, -r), pt(r, r), pt(-r, r)], true, out),
        Ending::Diamond => put(&[pt(-r, 0.0), pt(0.0, -r), pt(r, 0.0), pt(0.0, r)], true, out),
        Ending::Circle => {
            let rect = Rect::new(p.x - r, p.y - r, p.x + r, p.y + r);
            out.push(Point::new(rect.x0, rect.y0));
            out.push(Point::new(rect.x1, rect.y1));
            return (ellipse_path(rect), true);
        }
        Ending::OpenArrow => {
            let (l, h) = (3.0 * r, r * 0.6);
            put(&[pt(-l, h), pt(0.0, 0.0), pt(-l, -h)], false, out)
        }
        Ending::ClosedArrow => {
            let (l, h) = (3.0 * r, r * 0.6);
            put(&[pt(-l, h), pt(0.0, 0.0), pt(-l, -h)], true, out)
        }
        Ending::ROpenArrow => {
            let (l, h) = (3.0 * r, r * 0.6);
            put(&[pt(l, h), pt(0.0, 0.0), pt(l, -h)], false, out)
        }
        Ending::RClosedArrow => {
            let (l, h) = (3.0 * r, r * 0.6);
            put(&[pt(l, h), pt(0.0, 0.0), pt(l, -h)], true, out)
        }
        Ending::Butt => put(&[pt(0.0, -r), pt(0.0, r)], false, out),
        Ending::Slash => put(&[pt(-r * 0.5, -r * 0.87), pt(r * 0.5, r * 0.87)], false, out),
    };
    (s, closed)
}

/// Línea entre dos puntos con terminaciones. `bbox` incluye las terminaciones.
pub(crate) fn line(a: Point, b: Point, ends: (Ending, Ending), style: &Style) -> Ap {
    let (mut c, stroke, fill_color) = start(style);
    let stroke = stroke || style.width > 0.0;
    let mut pts = vec![a, b];
    let _ = writeln!(c, "{} {} m\n{} {} l\nS", n(a.x), n(a.y), n(b.x), n(b.y));
    let u = unit(a, b);
    for (kind, p, dir) in [(ends.0, a, (-u.0, -u.1)), (ends.1, b, u)] {
        let (path, closed) = ending_path(kind, p, dir, style.width, &mut pts);
        if path.is_empty() {
            continue;
        }
        let fill = fill_color && closed;
        c.push_str(&path);
        let op = if closed {
            paint_op(fill, stroke, true)
        } else {
            paint_op(false, stroke, false)
        };
        c.push_str(op);
        c.push('\n');
    }
    let (content, gs) = with_gs(style, c);
    Ap {
        bbox: expand(bbox_of(&pts), style.width.max(1.0)),
        content,
        gs,
        fonts: Vec::new(),
    }
}

// --- Iconos y marcas ---------------------------------------------------------------------------------------------

/// Icono de nota (`/Text`): dibujado en una caja de 20 × 20 con el color de relleno dado.
pub(crate) fn note_icon(name: &str, color: &[f32], opacity: f32) -> Ap {
    let mut c = String::new();
    let fill = if color.is_empty() { &[1.0, 1.0, 0.0][..] } else { color };
    c.push_str(&color_op(fill, false));
    c.push_str("0 G\n0.8 w\n");
    match name {
        "Note" | "Insert" | "Paragraph" | "NewParagraph" | "Key" | "Help" => {
            // Hoja con la esquina doblada.
            c.push_str("3 1 m\n17 1 l\n17 14 l\n13 19 l\n3 19 l\nh\nB\n");
            c.push_str("13 19 m\n13 14 l\n17 14 l\nS\n");
            c.push_str("6 6 m\n14 6 l\n6 9 m\n14 9 l\n6 12 m\n11 12 l\nS\n");
        }
        _ => {
            // «Comment»: globo con cola.
            c.push_str("2 6 m\n2 17 l\n18 17 l\n18 6 l\n11 6 l\n6 1 l\n7 6 l\nh\nB\n");
            c.push_str("5 14 m\n15 14 l\n5 11 m\n15 11 l\n5 8.5 m\n11 8.5 l\nS\n");
        }
    }
    let style = Style {
        color: None,
        interior: None,
        width: 1.0,
        dash: None,
        opacity,
    };
    let (content, gs) = with_gs(&style, c);
    Ap {
        bbox: Rect::new(0.0, 0.0, 20.0, 20.0),
        content,
        gs,
        fonts: Vec::new(),
    }
}

/// Marca de inserción (`/Caret`) en una caja de 20 × 14.
pub(crate) fn caret(color: &[f32], opacity: f32) -> Ap {
    let mut c = color_op(if color.is_empty() { &[0.0, 0.0, 1.0] } else { color }, false);
    c.push_str("0 0 m\n10 0 10 7 10 14 c\n10 7 10 0 20 0 c\nf\n");
    let style = Style {
        color: None,
        interior: None,
        width: 1.0,
        dash: None,
        opacity,
    };
    let (content, gs) = with_gs(&style, c);
    Ap {
        bbox: Rect::new(0.0, 0.0, 20.0, 14.0),
        content,
        gs,
        fonts: Vec::new(),
    }
}

/// Marca de redacción pendiente: contorno rojo (solo es una vista previa, no oculta nada).
pub(crate) fn redact_mark(rect: Rect) -> Ap {
    let (w, h) = (rect.width(), rect.height());
    Ap {
        bbox: rect,
        content: format!(
            "1 0 0 RG\n1 w\n{} {} {} {} re\nS\n",
            n(rect.x0 + 0.5),
            n(rect.y0 + 0.5),
            n((w - 1.0).max(0.0)),
            n((h - 1.0).max(0.0))
        ),
        gs: None,
        fonts: Vec::new(),
    }
}

/// Sello estándar sin apariencia propia: borde y nombre en mayúsculas, ajustados al rectángulo.
pub(crate) fn stamp(rect: Rect, name: &str, color: &[f32], opacity: f32) -> Ap {
    let (w, h) = (rect.width().max(1.0), rect.height().max(1.0));
    let label = stamp_label(name);
    let col = if color.is_empty() { &[1.0, 0.0, 0.0][..] } else { color };
    let border = (h * 0.06).clamp(0.75, 3.0);
    let inner_h = h - 2.0 * border;
    let mut size = inner_h * 0.5;
    let text_w = |s: f32| crate::native::metrics::text_width("Helvetica-Bold", &label, s);
    let avail = w - 4.0 * border - 4.0;
    if text_w(size) > avail && avail > 0.0 {
        size *= avail / text_w(size);
    }
    let tw = text_w(size);
    let (tx, ty) = ((w - tw) / 2.0, (h - size * 0.72) / 2.0);
    let mut c = String::new();
    c.push_str(&color_op(col, true));
    c.push_str(&color_op(col, false));
    let _ = writeln!(
        c,
        "{} w\n{} {} {} {} re\nS",
        n(border),
        n(border / 2.0),
        n(border / 2.0),
        n(w - border),
        n(h - border)
    );
    let _ = writeln!(
        c,
        "BT\n/Hebo {} Tf\n{} {} Td\n({}) Tj\nET",
        n(size),
        n(tx),
        n(ty),
        crate::native::metrics::escape(&crate::native::metrics::winansi(&label))
    );
    let style = Style {
        color: None,
        interior: None,
        width: 1.0,
        dash: None,
        opacity,
    };
    let (content, gs) = with_gs(&style, c);
    Ap {
        bbox: Rect::new(0.0, 0.0, w, h),
        content,
        gs,
        fonts: vec![("Hebo".into(), "Helvetica-Bold")],
    }
}

/// «NotApproved» → «NOT APPROVED».
pub(crate) fn stamp_label(name: &str) -> String {
    let mut out = String::new();
    let mut prev_lower = false;
    for ch in name.chars() {
        if ch.is_uppercase() && prev_lower {
            out.push(' ');
        }
        prev_lower = ch.is_lowercase();
        if ch == '_' || ch == '-' {
            out.push(' ');
        } else {
            out.extend(ch.to_uppercase());
        }
    }
    out
}

/// Texto libre: fondo, borde y texto ajustado al ancho con la fuente de `/DA`.
pub(crate) struct FreeText<'a> {
    pub rect: Rect,
    pub text: &'a str,
    pub font_res: &'a str,
    pub base_font: &'static str,
    pub size: f32,
    pub text_color: Vec<f32>,
    pub quadding: i32,
    pub style: &'a Style,
}

pub(crate) fn free_text(ft: &FreeText<'_>) -> Ap {
    let (w, h) = (ft.rect.width().max(1.0), ft.rect.height().max(1.0));
    let mut c = String::new();
    let (head, stroke, fill) = start(ft.style);
    c.push_str(&head);
    if fill || stroke {
        let _ = writeln!(
            c,
            "{} {} {} {} re\n{}",
            n(ft.style.width / 2.0),
            n(ft.style.width / 2.0),
            n(w - ft.style.width),
            n(h - ft.style.width),
            paint_op(fill, stroke, false)
        );
    }
    let pad = ft.style.width.max(0.0) + 2.0;
    let size = if ft.size > 0.0 { ft.size } else { 12.0 };
    let lines = crate::native::metrics::wrap(ft.text, ft.base_font, size, (w - 2.0 * pad).max(size));
    let lead = size * 1.2;
    c.push_str("q\n");
    let _ = writeln!(c, "{} {} {} {} re\nW\nn", n(0.0), n(0.0), n(w), n(h));
    c.push_str(&color_op(&ft.text_color, false));
    let _ = writeln!(c, "BT\n/{} {} Tf", ft.font_res, n(size));
    let mut y = h - pad - size * 0.9;
    let mut prev_x = 0.0f32;
    let mut prev_y = 0.0f32;
    for line in &lines {
        let tw = crate::native::metrics::text_width(ft.base_font, line, size);
        let x = match ft.quadding {
            1 => (w - tw) / 2.0,
            2 => w - pad - tw,
            _ => pad,
        };
        let _ = writeln!(
            c,
            "{} {} Td\n({}) Tj",
            n(x - prev_x),
            n(y - prev_y),
            crate::native::metrics::escape(&crate::native::metrics::winansi(line))
        );
        prev_x = x;
        prev_y = y;
        y -= lead;
    }
    c.push_str("ET\nQ\n");
    let (content, gs) = with_gs(ft.style, c);
    Ap {
        bbox: Rect::new(0.0, 0.0, w, h),
        content,
        gs,
        fonts: vec![(ft.font_res.to_string(), ft.base_font)],
    }
}
