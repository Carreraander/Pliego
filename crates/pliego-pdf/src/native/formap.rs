//! Apariencias de campos de formulario (ISO 32000-1, §12.7.3.3 y §12.7.4): texto, combinado, lista, botón de
//! acción, casilla y radio. Funciones puras que devuelven un [`Ap`]; `widget.rs` las instala.

use std::fmt::Write as _;

use super::appearance::{Ap, color_op, n};
use super::metrics::{escape, text_width, winansi, wrap};
use crate::geom::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Text,
    Combo,
    List,
    Push,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Border {
    #[default]
    Solid,
    Dashed,
    Beveled,
    Inset,
    Underline,
}

/// Todo lo que hace falta para dibujar un campo de texto, combinado, lista o botón.
pub(crate) struct FieldDraw<'a> {
    pub kind: Kind,
    pub width: f32,
    pub height: f32,
    pub background: Option<&'a [f32]>,
    pub border_color: Option<&'a [f32]>,
    pub border_width: f32,
    pub border: Border,
    /// Texto a mostrar (valor; en combinados ya traducido a la etiqueta; en botones, el rótulo).
    pub text: &'a str,
    pub font_res: &'a str,
    pub base_font: &'static str,
    /// 0 = automático.
    pub size: f32,
    pub color: &'a [f32],
    pub quadding: i32,
    pub multiline: bool,
    pub password: bool,
    /// Número de celdas si es un campo «peine» (`/MaxLen` con la marca Comb).
    pub comb: Option<usize>,
    /// Opciones (valor de exportación, etiqueta) y valores seleccionados, para las listas.
    pub options: &'a [(String, String)],
    pub selected: &'a [String],
    pub top_index: usize,
}

fn border_ops(f: &FieldDraw<'_>) -> String {
    let mut c = String::new();
    let (w, h, bw) = (f.width, f.height, f.border_width);
    if let Some(bg) = f.background.filter(|b| !b.is_empty()) {
        c.push_str(&color_op(bg, false));
        let _ = writeln!(c, "0 0 {} {} re\nf", n(w), n(h));
    }
    let Some(bc) = f.border_color.filter(|b| !b.is_empty() && bw > 0.0) else {
        return c;
    };
    c.push_str(&color_op(bc, true));
    c.push_str(&color_op(bc, false));
    match f.border {
        Border::Underline => {
            let _ = writeln!(c, "{} w\n0 {} m\n{} {} l\nS", n(bw), n(bw / 2.0), n(w), n(bw / 2.0));
        }
        Border::Dashed => {
            let _ = writeln!(
                c,
                "{} w\n[3] 0 d\n{} {} {} {} re\nS\n[] 0 d",
                n(bw),
                n(bw / 2.0),
                n(bw / 2.0),
                n(w - bw),
                n(h - bw)
            );
        }
        Border::Beveled | Border::Inset => {
            // Marco exterior y dos listones de sombra en los bordes inferior y derecho.
            let _ = writeln!(
                c,
                "{} w\n{} {} {} {} re\nS",
                n(bw),
                n(bw / 2.0),
                n(bw / 2.0),
                n(w - bw),
                n(h - bw)
            );
            let shade = if f.border == Border::Beveled { "0.5 g" } else { "0.75 g" };
            let _ = writeln!(
                c,
                "{shade}\n{} {} m\n{} {} l\n{} {} l\n{} {} l\nf",
                n(bw),
                n(bw),
                n(w - bw),
                n(bw),
                n(w - bw),
                n(h - bw),
                n(w - 2.0 * bw),
                n(h - 2.0 * bw)
            );
        }
        Border::Solid => {
            let _ = writeln!(
                c,
                "{} w\n{} {} {} {} re\nS",
                n(bw),
                n(bw / 2.0),
                n(bw / 2.0),
                n(w - bw),
                n(h - bw)
            );
        }
    }
    c
}

/// Cuerpo de letra de un campo con tamaño automático.
fn auto_size(f: &FieldDraw<'_>, avail_w: f32, avail_h: f32) -> f32 {
    if f.size > 0.0 {
        return f.size;
    }
    if f.multiline || f.kind == Kind::List {
        return 12.0;
    }
    let mut size = (avail_h * 0.75).clamp(4.0, 14.0);
    let tw = text_width(f.base_font, f.text, size);
    if tw > avail_w && tw > 0.0 {
        size = (size * avail_w / tw).max(4.0);
    }
    size
}

pub(crate) fn draw_field(f: &FieldDraw<'_>) -> Ap {
    let mut c = border_ops(f);
    let pad = f.border_width.max(1.0) + 1.0;
    let (aw, ah) = ((f.width - 2.0 * pad).max(1.0), (f.height - 2.0 * pad).max(1.0));
    let size = auto_size(f, aw, ah);
    let mut shown = f.text.to_string();
    if f.password {
        shown = "*".repeat(shown.chars().count());
    }
    let color = if f.color.is_empty() { &[0.0][..] } else { f.color };

    match f.kind {
        Kind::Push => {
            if !shown.is_empty() {
                let size = if f.size > 0.0 {
                    f.size
                } else {
                    (f.height * 0.5).clamp(4.0, 14.0)
                };
                let tw = text_width(f.base_font, &shown, size);
                let (x, y) = ((f.width - tw) / 2.0, (f.height - size * 0.72) / 2.0);
                c.push_str("q\n");
                c.push_str(&color_op(color, false));
                let _ = writeln!(
                    c,
                    "BT\n/{} {} Tf\n{} {} Td\n({}) Tj\nET\nQ",
                    f.font_res,
                    n(size),
                    n(x),
                    n(y),
                    escape(&winansi(&shown))
                );
            }
        }
        Kind::List => {
            c.push_str("/Tx BMC\nq\n");
            let _ = writeln!(c, "{} {} {} {} re\nW\nn", n(pad), n(pad), n(aw), n(ah));
            let lh = size * 1.15;
            let mut y = f.height - pad;
            for (export, label) in f.options.iter().skip(f.top_index) {
                if y - lh < pad - lh * 0.5 {
                    break;
                }
                if f.selected.iter().any(|s| s == export || s == label) {
                    let _ = writeln!(
                        c,
                        "0.6 0.75 0.85 rg\n{} {} {} {} re\nf",
                        n(pad),
                        n(y - lh),
                        n(aw),
                        n(lh)
                    );
                }
                c.push_str(&color_op(color, false));
                let _ = writeln!(
                    c,
                    "BT\n/{} {} Tf\n{} {} Td\n({}) Tj\nET",
                    f.font_res,
                    n(size),
                    n(pad + 1.0),
                    n(y - size * 0.95),
                    escape(&winansi(label))
                );
                y -= lh;
            }
            c.push_str("Q\nEMC\n");
        }
        Kind::Text | Kind::Combo => {
            if !shown.is_empty() {
                c.push_str("/Tx BMC\nq\n");
                let _ = writeln!(c, "{} {} {} {} re\nW\nn", n(pad), n(pad), n(aw), n(ah));
                c.push_str(&color_op(color, false));
                let _ = writeln!(c, "BT\n/{} {} Tf", f.font_res, n(size));
                if let Some(cells) = f.comb.filter(|k| *k > 0 && !f.multiline) {
                    let cell = (f.width - 2.0 * f.border_width) / cells as f32;
                    let y = (f.height - size * 0.72) / 2.0;
                    let mut px = 0.0;
                    for (i, ch) in shown.chars().take(cells).enumerate() {
                        let s = ch.to_string();
                        let tw = text_width(f.base_font, &s, size);
                        let x = f.border_width + cell * i as f32 + (cell - tw) / 2.0;
                        let _ = writeln!(
                            c,
                            "{} {} Td\n({}) Tj",
                            n(x - px),
                            n(if i == 0 { y } else { 0.0 }),
                            escape(&winansi(&s))
                        );
                        px = x;
                    }
                } else {
                    let lines = if f.multiline {
                        wrap(&shown, f.base_font, size, aw)
                    } else {
                        vec![shown.replace(['\n', '\r'], " ")]
                    };
                    let mut y = if f.multiline {
                        f.height - pad - size * 0.9
                    } else {
                        (f.height - size * 0.72) / 2.0
                    };
                    let (mut px, mut py) = (0.0f32, 0.0f32);
                    for line in &lines {
                        let tw = text_width(f.base_font, line, size);
                        let x = match f.quadding {
                            1 => (f.width - tw) / 2.0,
                            2 => f.width - pad - tw,
                            _ => pad + 1.0,
                        };
                        let _ = writeln!(c, "{} {} Td\n({}) Tj", n(x - px), n(y - py), escape(&winansi(line)));
                        px = x;
                        py = y;
                        y -= size * 1.2;
                    }
                }
                c.push_str("ET\nQ\nEMC\n");
            }
        }
    }
    Ap {
        bbox: Rect::new(0.0, 0.0, f.width, f.height),
        content: c,
        gs: None,
        fonts: vec![(f.font_res.to_string(), f.base_font)],
    }
}

/// Una casilla o un radio con su fondo y borde.
pub(crate) struct ToggleDraw<'a> {
    pub radio: bool,
    pub width: f32,
    pub height: f32,
    pub background: Option<&'a [f32]>,
    pub border_color: Option<&'a [f32]>,
    pub border_width: f32,
    pub color: &'a [f32],
}

/// Estado activado (`on`) o desactivado de una casilla o de un radio.
pub(crate) fn draw_toggle(t: &ToggleDraw<'_>, on: bool) -> Ap {
    let ToggleDraw {
        radio,
        width,
        height,
        background,
        border_color,
        border_width,
        color,
    } = *t;
    let mut c = String::new();
    let (w, h) = (width, height);
    let r = w.min(h) / 2.0;
    let (cx, cy) = (w / 2.0, h / 2.0);
    let circle = |cx: f32, cy: f32, r: f32| {
        let k = r * 0.552_284_7;
        format!(
            "{} {} m\n{} {} {} {} {} {} c\n{} {} {} {} {} {} c\n{} {} {} {} {} {} c\n{} {} {} {} {} {} c\n",
            n(cx + r),
            n(cy),
            n(cx + r),
            n(cy + k),
            n(cx + k),
            n(cy + r),
            n(cx),
            n(cy + r),
            n(cx - k),
            n(cy + r),
            n(cx - r),
            n(cy + k),
            n(cx - r),
            n(cy),
            n(cx - r),
            n(cy - k),
            n(cx - k),
            n(cy - r),
            n(cx),
            n(cy - r),
            n(cx + k),
            n(cy - r),
            n(cx + r),
            n(cy - k),
            n(cx + r),
            n(cy),
        )
    };
    if let Some(bg) = background.filter(|b| !b.is_empty()) {
        c.push_str(&color_op(bg, false));
        if radio {
            c.push_str(&circle(cx, cy, r));
            c.push_str("f\n");
        } else {
            let _ = writeln!(c, "0 0 {} {} re\nf", n(w), n(h));
        }
    }
    if let Some(bc) = border_color.filter(|b| !b.is_empty() && border_width > 0.0) {
        c.push_str(&color_op(bc, true));
        let _ = writeln!(c, "{} w", n(border_width));
        if radio {
            c.push_str(&circle(cx, cy, r - border_width / 2.0));
            c.push_str("S\n");
        } else {
            let _ = writeln!(
                c,
                "{} {} {} {} re\nS",
                n(border_width / 2.0),
                n(border_width / 2.0),
                n(w - border_width),
                n(h - border_width)
            );
        }
    }
    if on {
        let col = if color.is_empty() { &[0.0][..] } else { color };
        c.push_str(&color_op(col, false));
        c.push_str(&color_op(col, true));
        if radio {
            c.push_str(&circle(cx, cy, r * 0.5));
            c.push_str("f\n");
        } else {
            let _ = writeln!(
                c,
                "{} w\n1 J\n1 j\n{} {} m\n{} {} l\n{} {} l\nS",
                n((w.min(h) * 0.12).max(1.0)),
                n(w * 0.22),
                n(h * 0.5),
                n(w * 0.42),
                n(h * 0.25),
                n(w * 0.8),
                n(h * 0.78)
            );
        }
    }
    Ap {
        bbox: Rect::new(0.0, 0.0, w, h),
        content: c,
        gs: None,
        fonts: Vec::new(),
    }
}
