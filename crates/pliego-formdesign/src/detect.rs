//! Detección automática de campos en PDFs planos y escaneos.
//!
//! Se renderiza la página en grises y se buscan, sobre el mapa de bits, (1) recuadros: regiones blancas cerradas por
//! un borde oscuro, (2) casillas de peine (filas de recuadros pequeños iguales) y (3) líneas horizontales, que se
//! interpretan como campos de «subrayado». Las etiquetas salen de la capa de texto si existe; en un escaneo sin texto
//! los campos se nombran `campo_N`. Es una heurística: la interfaz debe dejar revisar el resultado.

use mupdf::{Colorspace, Matrix, TextExtractOptions, pdf::PdfDocument};
use pliego_core::{Error, JobContext, Result};

use crate::pe;
use crate::spec::{FieldSpec, FieldType};

/// Píxeles por punto del mapa de bits (144 ppp).
const SCALE: f32 = 2.0;
/// Por debajo de este gris (0-255) un píxel cuenta como tinta.
const DARK: u8 = 190;

/// Rectángulo en puntos, en el espacio de página de MuPDF (origen arriba a la izquierda).
#[derive(Debug, Clone, Copy)]
struct R {
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
}

impl R {
    fn w(&self) -> f32 {
        self.x1 - self.x0
    }
    fn h(&self) -> f32 {
        self.y1 - self.y0
    }
    fn cx(&self) -> f32 {
        (self.x0 + self.x1) / 2.0
    }
    fn cy(&self) -> f32 {
        (self.y0 + self.y1) / 2.0
    }
    fn overlaps(&self, o: &R) -> bool {
        self.x0 < o.x1 && o.x0 < self.x1 && self.y0 < o.y1 && o.y0 < self.y1
    }
    fn contains(&self, o: &R) -> bool {
        self.x0 <= o.x0 + 0.5 && self.y0 <= o.y0 + 0.5 && self.x1 >= o.x1 - 0.5 && self.y1 >= o.y1 - 0.5
    }
    fn x_overlap(&self, o: &R) -> f32 {
        (self.x1.min(o.x1) - self.x0.max(o.x0)).max(0.0)
    }
    fn y_overlap(&self, o: &R) -> f32 {
        (self.y1.min(o.y1) - self.y0.max(o.y0)).max(0.0)
    }
}

struct Word {
    text: String,
    r: R,
    block: usize,
    line: usize,
}

struct Candidate {
    r: R,
    kind: FieldType,
    comb: Option<u32>,
    multiline: bool,
}

/// Mapa binario de tinta.
struct Ink {
    w: usize,
    h: usize,
    dark: Vec<bool>,
}

impl Ink {
    fn at(&self, x: i64, y: i64) -> bool {
        x >= 0
            && y >= 0
            && (x as usize) < self.w
            && (y as usize) < self.h
            && self.dark[y as usize * self.w + x as usize]
    }
}

fn to_px(v: f32) -> i64 {
    (v * SCALE).round() as i64
}

/// Componentes blancos cerrados: rectángulos candidatos (en puntos, relativos al origen del mapa).
fn find_boxes(ink: &Ink) -> Vec<R> {
    let (w, h) = (ink.w, ink.h);
    let mut label = vec![0u32; w * h];
    let mut next = 1u32;
    let mut out = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    for start in 0..w * h {
        if ink.dark[start] || label[start] != 0 {
            continue;
        }
        let id = next;
        next += 1;
        let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0usize, 0usize);
        let mut area = 0usize;
        label[start] = id;
        stack.push(start);
        while let Some(i) = stack.pop() {
            let (x, y) = (i % w, i / w);
            area += 1;
            x0 = x0.min(x);
            x1 = x1.max(x);
            y0 = y0.min(y);
            y1 = y1.max(y);
            let mut push = |j: usize| {
                if !ink.dark[j] && label[j] == 0 {
                    label[j] = id;
                    stack.push(j);
                }
            };
            if x > 0 {
                push(i - 1);
            }
            if x + 1 < w {
                push(i + 1);
            }
            if y > 0 {
                push(i - w);
            }
            if y + 1 < h {
                push(i + w);
            }
        }
        if x0 == 0 || y0 == 0 || x1 + 1 == w || y1 + 1 == h {
            continue; // toca el borde: es fondo
        }
        let (bw, bh) = (x1 - x0 + 1, y1 - y0 + 1);
        if bw < 10 || bh < 10 || (area as f32) < 0.75 * (bw * bh) as f32 {
            continue;
        }
        if ring_dark(ink, x0 as i64, y0 as i64, x1 as i64, y1 as i64) < 0.85 {
            continue;
        }
        out.push(R {
            x0: x0 as f32 / SCALE,
            y0: y0 as f32 / SCALE,
            x1: (x1 + 1) as f32 / SCALE,
            y1: (y1 + 1) as f32 / SCALE,
        });
    }
    out
}

/// Fracción del contorno exterior del rectángulo que es tinta (hasta 2 px de distancia).
fn ring_dark(ink: &Ink, x0: i64, y0: i64, x1: i64, y1: i64) -> f32 {
    let mut total = 0;
    let mut dark = 0;
    let mut check = |xs: &[i64], ys: &[i64]| {
        total += 1;
        if xs.iter().zip(ys).any(|(x, y)| ink.at(*x, *y)) {
            dark += 1;
        }
    };
    for x in x0..=x1 {
        check(&[x, x], &[y0 - 1, y0 - 2]);
        check(&[x, x], &[y1 + 1, y1 + 2]);
    }
    for y in y0..=y1 {
        check(&[x0 - 1, x0 - 2], &[y, y]);
        check(&[x1 + 1, x1 + 2], &[y, y]);
    }
    dark as f32 / total.max(1) as f32
}

/// Segmentos horizontales finos de tinta (líneas), en puntos.
fn find_lines(ink: &Ink) -> Vec<R> {
    struct Seg {
        x0: usize,
        x1: usize,
        y0: usize,
        y1: usize,
    }
    let mut segs: Vec<Seg> = Vec::new();
    let min_len = 30;
    for y in 0..ink.h {
        let mut x = 0;
        while x < ink.w {
            if !ink.dark[y * ink.w + x] {
                x += 1;
                continue;
            }
            let s = x;
            while x < ink.w && ink.dark[y * ink.w + x] {
                x += 1;
            }
            if x - s >= min_len {
                match segs
                    .iter_mut()
                    .find(|g| g.y1 + 1 == y && g.x0 <= x && s <= g.x1 + 1 && (g.x0 as i64 - s as i64).abs() <= 4)
                {
                    Some(g) => {
                        g.y1 = y;
                        g.x0 = g.x0.min(s);
                        g.x1 = g.x1.max(x - 1);
                    }
                    None => segs.push(Seg {
                        x0: s,
                        x1: x - 1,
                        y0: y,
                        y1: y,
                    }),
                }
            }
        }
    }
    segs.into_iter()
        .filter(|g| g.y1 - g.y0 < 4)
        .map(|g| R {
            x0: g.x0 as f32 / SCALE,
            y0: g.y0 as f32 / SCALE,
            x1: (g.x1 + 1) as f32 / SCALE,
            y1: (g.y1 + 1) as f32 / SCALE,
        })
        .collect()
}

fn is_leader(text: &str) -> bool {
    let n = text.chars().count();
    n >= 4 && text.chars().all(|c| matches!(c, '_' | '.' | '…' | '-' | '·'))
}

fn sanitize(label: &str) -> String {
    let mut out = String::new();
    let mut last_us = true;
    for c in label.chars() {
        if c.is_alphanumeric() {
            out.push(c);
            last_us = false;
        } else if (c.is_whitespace() || c == '_' || c == '-' || c == '/') && !last_us {
            out.push('_');
            last_us = true;
        }
    }
    let t = out.trim_matches('_');
    t.chars().take(40).collect()
}

struct TextLine {
    r: R,
    words: Vec<usize>,
}

fn group_lines(words: &[Word]) -> Vec<TextLine> {
    let mut lines: Vec<(usize, usize, TextLine)> = Vec::new();
    for (i, w) in words.iter().enumerate() {
        if is_leader(&w.text) {
            continue;
        }
        match lines.iter_mut().find(|(b, l, _)| *b == w.block && *l == w.line) {
            Some((_, _, t)) => {
                t.r.x0 = t.r.x0.min(w.r.x0);
                t.r.y0 = t.r.y0.min(w.r.y0);
                t.r.x1 = t.r.x1.max(w.r.x1);
                t.r.y1 = t.r.y1.max(w.r.y1);
                t.words.push(i);
            }
            None => lines.push((w.block, w.line, TextLine { r: w.r, words: vec![i] })),
        }
    }
    lines.into_iter().map(|(_, _, t)| t).collect()
}

/// Etiqueta de un campo según su tipo: dentro, a la derecha (casillas), a la izquierda o encima.
fn find_label(c: &Candidate, words: &[Word], lines: &[TextLine]) -> Option<String> {
    let r = c.r;
    let join = |ws: &[usize]| {
        let t: Vec<&str> = ws.iter().map(|i| words[*i].text.as_str()).collect();
        t.join(" ")
    };
    // 1. Texto dentro del recuadro.
    if c.kind == FieldType::Text && c.comb.is_none() {
        let inside: Vec<usize> = (0..words.len())
            .filter(|i| !is_leader(&words[*i].text) && r.contains(&words[*i].r))
            .collect();
        if !inside.is_empty() && inside.len() <= 6 {
            return Some(join(&inside));
        }
    }
    let right = || {
        lines
            .iter()
            .filter(|l| {
                l.r.y_overlap(&r) > 0.4 * r.h().min(l.r.h())
                    && l.r.x1 > r.x1
                    && l.r.x0 - r.x1 < 12.0
                    && l.r.x0 >= r.x1 - 1.0
            })
            .min_by(|a, b| a.r.x0.total_cmp(&b.r.x0))
            .map(|l| {
                let mut ws = l.words.clone();
                ws.sort_by(|a, b| words[*a].r.x0.total_cmp(&words[*b].r.x0));
                let mut take = Vec::new();
                let mut prev = r.x1;
                for i in ws {
                    if words[i].r.x0 - prev > 12.0 || take.len() >= 8 {
                        break;
                    }
                    prev = words[i].r.x1;
                    take.push(i);
                }
                join(&take)
            })
    };
    let left = || {
        // Misma franja vertical, a la izquierda del campo y cerca.
        lines
            .iter()
            .filter(|l| {
                let band = R {
                    y0: r.y0 - 2.0,
                    y1: r.y1 + 2.0,
                    ..r
                };
                l.r.y_overlap(&band) > 0.4 * l.r.h().min(band.h()) && l.r.x0 < r.x0
            })
            .filter_map(|l| {
                let mut ws: Vec<usize> = l
                    .words
                    .iter()
                    .copied()
                    .filter(|i| words[*i].r.x1 <= r.x0 + 2.0)
                    .collect();
                ws.sort_by(|a, b| words[*b].r.x1.total_cmp(&words[*a].r.x1));
                let first = *ws.first()?;
                if r.x0 - words[first].r.x1 > 70.0 {
                    return None;
                }
                let mut take = vec![first];
                for i in ws.into_iter().skip(1) {
                    if words[*take.last()?].r.x0 - words[i].r.x1 > 12.0 || take.len() >= 6 {
                        break;
                    }
                    take.push(i);
                }
                take.reverse();
                Some((r.x0 - words[*take.last()?].r.x1, take))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, t)| join(&t))
    };
    let above = || {
        lines
            .iter()
            .filter(|l| l.r.y1 <= r.y0 + 1.5 && r.y0 - l.r.y1 < 14.0 && l.r.x_overlap(&r) > 0.0)
            .min_by(|a, b| (r.y0 - a.r.y1).total_cmp(&(r.y0 - b.r.y1)))
            .and_then(|l| {
                let mut ws: Vec<usize> = l
                    .words
                    .iter()
                    .copied()
                    .filter(|i| words[*i].r.cx() >= r.x0 - 2.0 && words[*i].r.cx() <= r.x1)
                    .collect();
                ws.sort_by(|a, b| words[*a].r.x0.total_cmp(&words[*b].r.x0));
                let mut take = Vec::new();
                let mut prev: Option<f32> = None;
                for i in ws {
                    if prev.is_some_and(|p| words[i].r.x0 - p > 12.0) || take.len() >= 8 {
                        break;
                    }
                    prev = Some(words[i].r.x1);
                    take.push(i);
                }
                (!take.is_empty()).then(|| join(&take))
            })
    };
    match c.kind {
        FieldType::CheckBox => right().or_else(left).or_else(above),
        _ => left().or_else(above),
    }
}

/// Detecta campos de formulario en una página. Devuelve `FieldSpec` con nombre, tipo y rectángulo en espacio de
/// usuario del PDF (listos para `add_field`), en orden de lectura.
pub fn detect_fields(doc: &PdfDocument, page: usize, ctx: &JobContext) -> Result<Vec<FieldSpec>> {
    let pages = doc.page_count().map_err(pe)?;
    if page as i32 >= pages {
        return Err(Error::InvalidInput(format!("la página {} no existe", page + 1)));
    }
    ctx.step(0, 4, "Analizando la página")?;
    let pg = doc.load_pdf_page(page as i32).map_err(pe)?;
    // Se trabaja con la página «derecha»: espacio de usuario con y hacia abajo, sin giro, con el origen en la
    // esquina superior izquierda de la caja visible. Así las líneas del documento siguen siendo horizontales
    // aunque la página tenga /Rotate.
    let page_obj = doc.find_page(page as i32).map_err(pe)?;
    let ctm = page_obj.page_ctm().map_err(pe)?;
    let inv = ctm
        .invert()
        .ok_or_else(|| Error::Pdf("la transformación de la página no es invertible".into()))?;
    let (cx0, cy1) = visible_box(&page_obj);
    let render = Matrix::new(
        SCALE * inv.a,
        -SCALE * inv.b,
        SCALE * inv.c,
        -SCALE * inv.d,
        SCALE * (inv.e - cx0),
        SCALE * (cy1 - inv.f),
    );
    let pm = pg
        .to_pixmap(&render, &Colorspace::device_gray(), false, false)
        .map_err(pe)?;
    let (w, h, n) = (pm.width() as usize, pm.height() as usize, pm.n() as usize);
    let stride = pm.stride() as usize;
    let samples = pm.samples();
    let (ox, oy) = pm.origin();
    let (ox, oy) = (ox as f32 / SCALE, oy as f32 / SCALE);
    // Rectángulo del espacio de página al espacio derecho.
    let upright = |x0: f32, y0: f32, x1: f32, y1: f32| {
        let pts = [(x0, y0), (x1, y0), (x0, y1), (x1, y1)].map(|(x, y)| {
            let (ux, uy) = (inv.a * x + inv.c * y + inv.e, inv.b * x + inv.d * y + inv.f);
            (ux - cx0 - ox, cy1 - uy - oy)
        });
        R {
            x0: pts.iter().map(|p| p.0).fold(f32::INFINITY, f32::min),
            y0: pts.iter().map(|p| p.1).fold(f32::INFINITY, f32::min),
            x1: pts.iter().map(|p| p.0).fold(f32::NEG_INFINITY, f32::max),
            y1: pts.iter().map(|p| p.1).fold(f32::NEG_INFINITY, f32::max),
        }
    };
    let mut dark = vec![false; w * h];
    for y in 0..h {
        for x in 0..w {
            dark[y * w + x] = samples[y * stride + x * n] < DARK;
        }
    }
    let mut ink = Ink { w, h, dark };

    // Texto de la página (si lo hay): se borra del mapa para que no estorbe a recuadros y líneas.
    let words: Vec<Word> = pg
        .words(TextExtractOptions::default())
        .map_err(pe)?
        .into_iter()
        .map(|t| Word {
            text: t.text,
            r: upright(t.bounds.x0, t.bounds.y0, t.bounds.x1, t.bounds.y1),
            block: t.block,
            line: t.line,
        })
        .collect();
    for wd in words.iter().filter(|w| !is_leader(&w.text)) {
        for y in to_px(wd.r.y0 - 0.5).max(0)..to_px(wd.r.y1 + 0.5).min(h as i64) {
            for x in to_px(wd.r.x0 - 0.5).max(0)..to_px(wd.r.x1 + 0.5).min(w as i64) {
                ink.dark[y as usize * w + x as usize] = false;
            }
        }
    }
    // Campos que ya existen: no se proponen otra vez.
    let existing: Vec<R> = pg
        .widgets()
        .filter_map(|wd| wd.annotation().bounds().ok())
        .map(|b| upright(b.x0, b.y0, b.x1, b.y1))
        .collect();
    let page_w = w as f32 / SCALE;
    let page_h = h as f32 / SCALE;

    ctx.step(1, 4, "Buscando recuadros")?;
    let raw_boxes = find_boxes(&ink);
    // Un recuadro que contiene a otros (marco de sección) o mucho texto no es un campo.
    let boxes: Vec<R> = raw_boxes
        .iter()
        .enumerate()
        .filter(|(i, b)| {
            !raw_boxes
                .iter()
                .enumerate()
                .any(|(j, o)| j != *i && b.contains(o) && o.w() * o.h() < b.w() * b.h())
                && words.iter().filter(|w| !is_leader(&w.text) && b.contains(&w.r)).count() <= 6
                && b.w() * b.h() < 0.4 * page_w * page_h
        })
        .map(|(_, b)| *b)
        .collect();

    let mut cands: Vec<Candidate> = Vec::new();
    // Recuadros pequeños e iguales en fila → un solo campo de peine.
    let mut used = vec![false; boxes.len()];
    let mut order: Vec<usize> = (0..boxes.len()).collect();
    order.sort_by(|a, b| {
        boxes[*a]
            .cy()
            .total_cmp(&boxes[*b].cy())
            .then(boxes[*a].x0.total_cmp(&boxes[*b].x0))
    });
    for &i in &order {
        if used[i] || boxes[i].w() > 30.0 || boxes[i].h() > 30.0 {
            continue;
        }
        let mut row = vec![i];
        loop {
            let last = boxes[*row.last().unwrap_or(&i)];
            let nxt = order.iter().copied().find(|j| {
                !used[*j]
                    && !row.contains(j)
                    && (boxes[*j].cy() - last.cy()).abs() < 2.0
                    && (boxes[*j].w() - last.w()).abs() < 2.0
                    && (boxes[*j].h() - last.h()).abs() < 2.0
                    && boxes[*j].x0 >= last.x1 - 2.0
                    && boxes[*j].x0 - last.x1 < 4.0
            });
            match nxt {
                Some(j) => row.push(j),
                None => break,
            }
        }
        if row.len() >= 4 {
            let r = R {
                x0: boxes[row[0]].x0,
                y0: boxes[row[0]].y0,
                x1: boxes[*row.last().unwrap_or(&i)].x1,
                y1: boxes[row[0]].y1,
            };
            for j in &row {
                used[*j] = true;
            }
            cands.push(Candidate {
                r,
                kind: FieldType::Text,
                comb: Some(row.len() as u32),
                multiline: false,
            });
        }
    }
    for (i, b) in boxes.iter().enumerate() {
        if used[i] {
            continue;
        }
        let square = (6.0..=18.0).contains(&b.w())
            && (6.0..=18.0).contains(&b.h())
            && (b.w() / b.h()).clamp(0.0, 9.0) > 0.8
            && b.w() / b.h() < 1.25;
        if square {
            cands.push(Candidate {
                r: *b,
                kind: FieldType::CheckBox,
                comb: None,
                multiline: false,
            });
        } else if b.h() >= 10.0 && b.w() >= 20.0 {
            cands.push(Candidate {
                r: *b,
                kind: FieldType::Text,
                comb: None,
                multiline: b.h() >= 36.0,
            });
        }
    }

    ctx.step(2, 4, "Buscando líneas")?;
    // Subrayados: líneas que no son el borde de un recuadro ni reglas de página completa.
    for l in find_lines(&ink) {
        if l.w() > 0.9 * page_w {
            continue;
        }
        let on_box = raw_boxes.iter().any(|b| {
            ((l.cy() - b.y0).abs() < 4.0 || (l.cy() - b.y1).abs() < 4.0) && l.x_overlap(b) > 0.5 * l.w().min(b.w())
        });
        if on_box {
            continue;
        }
        let height = 14.0f32;
        let mut r = R {
            x0: l.x0,
            y0: l.y0 - height,
            x1: l.x1,
            y1: l.y0,
        };
        // Si hay texto encima de la línea, el campo se encoge para no taparlo.
        let probe = r;
        for wd in words.iter().filter(|w| !is_leader(&w.text) && w.r.overlaps(&probe)) {
            if wd.r.cx() < r.x0 + 0.4 * r.w() {
                r.x0 = r.x0.max(wd.r.x1 + 2.0);
            } else {
                r.x1 = r.x1.min(wd.r.x0 - 2.0);
            }
        }
        if r.w() >= 20.0 {
            cands.push(Candidate {
                r,
                kind: FieldType::Text,
                comb: None,
                multiline: false,
            });
        }
    }
    // Rellenos de guiones bajos o puntos en el propio texto.
    for wd in words.iter().filter(|w| is_leader(&w.text)) {
        let r = R {
            x0: wd.r.x0,
            y0: wd.r.y0 - 1.0,
            x1: wd.r.x1,
            y1: wd.r.y1 + 1.0,
        };
        if r.w() >= 20.0 {
            cands.push(Candidate {
                r,
                kind: FieldType::Text,
                comb: None,
                multiline: false,
            });
        }
    }
    cands.retain(|c| !existing.iter().any(|e| e.overlaps(&c.r)));
    // Un subrayado y un recuadro que se solapan son el mismo campo: queda el recuadro.
    let snapshot: Vec<(R, bool)> = cands
        .iter()
        .map(|c| (c.r, c.comb.is_some() || c.r.h() > 15.0))
        .collect();
    let mut keep = vec![true; cands.len()];
    for i in 0..cands.len() {
        for j in 0..cands.len() {
            if i != j && keep[j] && snapshot[i].0.overlaps(&snapshot[j].0) {
                let (a, b) = (&snapshot[i], &snapshot[j]);
                let inter = a.0.x_overlap(&b.0) * a.0.y_overlap(&b.0);
                let smaller = (a.0.w() * a.0.h()).min(b.0.w() * b.0.h());
                if inter > 0.5 * smaller && (a.1 || i < j) && !(b.1 && !a.1) {
                    keep[j] = false;
                }
            }
        }
    }
    let mut kept: Vec<Candidate> = cands
        .into_iter()
        .zip(keep)
        .filter(|(_, k)| *k)
        .map(|(c, _)| c)
        .collect();
    kept.sort_by(|a, b| {
        let (ay, by) = (a.r.cy(), b.r.cy());
        if (ay - by).abs() < 6.0 {
            a.r.x0.total_cmp(&b.r.x0)
        } else {
            ay.total_cmp(&by)
        }
    });

    ctx.step(3, 4, "Poniendo nombre a los campos")?;
    let lines = group_lines(&words);
    let mut taken: Vec<String> = Vec::new();
    for p in 0..pages {
        for wd in doc.load_pdf_page(p).map_err(pe)?.widgets() {
            if let Ok(Some(nm)) = wd.name() {
                taken.push(nm);
            }
        }
    }
    // Espacio derecho → espacio de usuario del PDF.
    let user = |x: f32, y: f32| (x + ox + cx0, cy1 - (y + oy));

    let mut out = Vec::new();
    let mut anon = 0;
    for c in &kept {
        let label = find_label(c, &words, &lines);
        let mut base = label
            .as_deref()
            .map(sanitize)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                anon += 1;
                format!("campo_{anon}")
            });
        if taken.contains(&base) {
            let mut k = 2;
            while taken.contains(&format!("{base}_{k}")) {
                k += 1;
            }
            base = format!("{base}_{k}");
        }
        taken.push(base.clone());
        let (ax, ay) = user(c.r.x0, c.r.y0);
        let (bx, by) = user(c.r.x1, c.r.y1);
        out.push(FieldSpec {
            name: base,
            kind: c.kind,
            page,
            rect: [ax.min(bx), ay.min(by), ax.max(bx), ay.max(by)],
            multiline: c.multiline,
            max_len: c.comb,
            tooltip: label.filter(|l| !l.trim().is_empty()),
            ..Default::default()
        });
    }
    ctx.step(4, 4, "Listo")?;
    Ok(out)
}

/// Esquina izquierda y borde superior (espacio de usuario) de la caja visible: `/CropBox` o, si falta, `/MediaBox`.
fn visible_box(page: &mupdf::pdf::PdfObject) -> (f32, f32) {
    let get = |k: &str| {
        let a = page.get_dict_inheritable(k).ok().flatten()?;
        let mut v = [0f32; 4];
        for (i, x) in v.iter_mut().enumerate() {
            *x = a.get_array(i as i32).ok().flatten()?.as_float().ok()?;
        }
        Some(v)
    };
    let b = get("CropBox")
        .or_else(|| get("MediaBox"))
        .unwrap_or([0.0, 0.0, 612.0, 792.0]);
    (b[0].min(b[2]), b[1].max(b[3]))
}
