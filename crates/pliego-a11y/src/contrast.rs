//! Comprobación básica de contraste de color del texto (WCAG 2.x, razón mínima 4,5:1; 3:1 para texto grande).
//!
//! Solo recorre el contenido directo de la página: lee el color de relleno de `g`, `rg`, `k` y de `sc`/`scn`
//! con componentes numéricos, y toma como fondo el último rectángulo relleno que contiene el origen del texto
//! (blanco si no hay ninguno). No analiza degradados, imágenes de fondo, patrones ni XObjects de formulario:
//! cuando no puede saber el color, no dice nada.

use crate::content::{IDENTITY, Tok, mul, scale_of, tokenize};

/// Razón mínima para texto normal.
pub(crate) const MIN_RATIO: f32 = 4.5;
/// Razón mínima para texto grande (a partir de 18 pt).
pub(crate) const MIN_RATIO_LARGE: f32 = 3.0;
const LARGE_PT: f32 = 18.0;

type Rgb = [f32; 3];

/// Texto de bajo contraste encontrado en una página.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LowContrast {
    pub count: usize,
    pub worst_ratio: f32,
    pub text: String,
}

fn lin(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.03928 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn luminance(c: Rgb) -> f32 {
    0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2])
}

pub(crate) fn ratio(a: Rgb, b: Rgb) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

fn color_from(nums: &[f32]) -> Option<Rgb> {
    match nums {
        [g] => Some([*g; 3]),
        [r, g, b] => Some([*r, *g, *b]),
        [c, m, y, k] => Some([(1.0 - c) * (1.0 - k), (1.0 - m) * (1.0 - k), (1.0 - y) * (1.0 - k)]),
        _ => None,
    }
}

fn apply(m: &[f32; 6], x: f32, y: f32) -> (f32, f32) {
    (x * m[0] + y * m[2] + m[4], x * m[1] + y * m[3] + m[5])
}

struct Bg {
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    color: Rgb,
}

/// Busca texto con contraste insuficiente en el contenido de una página.
pub(crate) fn scan(data: &[u8]) -> Option<LowContrast> {
    let toks = tokenize(data);
    let mut ctm = IDENTITY;
    let mut tm = IDENTITY;
    let mut stack: Vec<([f32; 6], Option<Rgb>)> = Vec::new();
    let mut fill: Option<Rgb> = Some([0.0; 3]);
    let mut size = 0.0f32;
    let mut invisible = false;
    let mut rects: Vec<(f32, f32, f32, f32)> = Vec::new();
    let mut bgs: Vec<Bg> = Vec::new();
    let mut ops: Vec<&Tok> = Vec::new();
    let mut found: Option<LowContrast> = None;

    for t in &toks {
        let Tok::Op(op) = &t.tok else {
            ops.push(&t.tok);
            continue;
        };
        let nums: Vec<f32> = ops
            .iter()
            .filter_map(|t| if let Tok::Num(v) = t { Some(*v) } else { None })
            .collect();
        let has_name = ops.iter().any(|t| matches!(t, Tok::Name(_)));
        match op.as_str() {
            "q" => stack.push((ctm, fill)),
            "Q" => {
                if let Some((c, f)) = stack.pop() {
                    ctm = c;
                    fill = f;
                }
            }
            "cm" if nums.len() == 6 => ctm = mul(&[nums[0], nums[1], nums[2], nums[3], nums[4], nums[5]], &ctm),
            "g" | "rg" | "k" => fill = color_from(&nums),
            "sc" | "scn" => fill = if has_name { None } else { color_from(&nums) },
            "cs" => fill = if nums.is_empty() { fill } else { None },
            "BT" => {
                tm = IDENTITY;
                invisible = false;
            }
            "Tr" => invisible = nums.first().is_some_and(|m| *m != 0.0),
            "Tf" => size = nums.last().copied().unwrap_or(0.0).abs(),
            "Tm" if nums.len() == 6 => tm = [nums[0], nums[1], nums[2], nums[3], nums[4], nums[5]],
            "Td" | "TD" if nums.len() == 2 => {
                tm[4] += nums[0] * tm[0] + nums[1] * tm[2];
                tm[5] += nums[0] * tm[1] + nums[1] * tm[3];
            }
            "re" if nums.len() == 4 => {
                let (x0, y0) = apply(&ctm, nums[0], nums[1]);
                let (x1, y1) = apply(&ctm, nums[0] + nums[2], nums[1] + nums[3]);
                rects.push((x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1)));
            }
            "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" => {
                for (x0, y0, x1, y1) in rects.drain(..) {
                    // Un relleno de color desconocido tapa lo anterior: se anula con un fondo «desconocido».
                    match fill {
                        Some(color) => bgs.push(Bg { x0, y0, x1, y1, color }),
                        None => bgs.push(Bg {
                            x0,
                            y0,
                            x1,
                            y1,
                            color: [f32::NAN; 3],
                        }),
                    }
                }
            }
            "S" | "s" | "n" => rects.clear(),
            "Tj" | "TJ" | "'" | "\"" if !invisible => {
                let shown = ops.iter().any(|t| match t {
                    Tok::Str(s) => s.iter().any(|b| !b.is_ascii_whitespace() && *b != 0),
                    _ => false,
                });
                if shown && let Some(fg) = fill {
                    let eff = size * scale_of(&tm) * scale_of(&ctm);
                    let full = mul(&tm, &ctm);
                    let (px, py) = apply(&full, 0.0, size * 0.3);
                    let bg = bgs
                        .iter()
                        .rev()
                        .find(|b| px >= b.x0 && px <= b.x1 && py >= b.y0 && py <= b.y1)
                        .map(|b| b.color)
                        .unwrap_or([1.0; 3]);
                    if !bg[0].is_nan() {
                        let r = ratio(fg, bg);
                        let need = if eff >= LARGE_PT { MIN_RATIO_LARGE } else { MIN_RATIO };
                        if r < need {
                            let sample: String = ops
                                .iter()
                                .find_map(|t| {
                                    if let Tok::Str(s) = t {
                                        Some(String::from_utf8_lossy(s).into_owned())
                                    } else {
                                        None
                                    }
                                })
                                .unwrap_or_default();
                            let f = found.get_or_insert(LowContrast {
                                count: 0,
                                worst_ratio: r,
                                text: sample.trim().chars().take(40).collect(),
                            });
                            f.count += 1;
                            if r < f.worst_ratio {
                                f.worst_ratio = r;
                                f.text = sample.trim().chars().take(40).collect();
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        ops.clear();
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn black_on_white_is_fine() {
        assert!(scan(b"BT /F1 12 Tf 72 700 Td (hola) Tj ET").is_none());
    }

    #[test]
    fn light_gray_on_white_fails() {
        let r = scan(b"0.8 g BT /F1 12 Tf 72 700 Td (claro) Tj ET").unwrap();
        assert_eq!(r.count, 1);
        assert!(r.worst_ratio < 2.0, "{}", r.worst_ratio);
    }

    #[test]
    fn white_on_dark_rect_is_fine_but_not_on_white() {
        let ok = b"0 0 0.4 rg 50 650 300 100 re f 1 g BT /F1 12 Tf 72 700 Td (blanco) Tj ET";
        assert!(scan(ok).is_none());
        let bad = b"1 g BT /F1 12 Tf 72 700 Td (blanco) Tj ET";
        assert!(scan(bad).is_some());
    }

    #[test]
    fn large_text_has_lower_threshold() {
        // gris 0.55 sobre blanco ≈ 3,4:1: falla en texto pequeño y pasa en texto grande.
        let small = b"0.55 g BT /F1 12 Tf 72 700 Td (x) Tj ET";
        let large = b"0.55 g BT /F1 24 Tf 72 700 Td (x) Tj ET";
        assert!(scan(small).is_some());
        assert!(scan(large).is_none());
    }

    #[test]
    fn unknown_color_and_invisible_text_are_ignored() {
        assert!(scan(b"/Pattern cs /P1 scn BT /F1 12 Tf 72 700 Td (x) Tj ET").is_none());
        assert!(scan(b"1 g BT 3 Tr /F1 12 Tf 72 700 Td (oculto) Tj ET").is_none());
    }

    #[test]
    fn cmyk_converts() {
        assert!(scan(b"0 0 0 0.1 k BT /F1 12 Tf 72 700 Td (x) Tj ET").is_some());
    }
}
