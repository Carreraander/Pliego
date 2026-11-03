//! De filas de texto a párrafos: unión de líneas, listas, alineación y tabuladores.

use crate::lines::{Frag, Row, is_marker_only, parse_marker, union};
use crate::model::{Align, BBox, ListItem, ListKind, Paragraph, Run};

/// Contador de listas compartido por todo el documento.
#[derive(Debug, Default)]
pub(crate) struct ListCounter {
    next_id: u32,
    last_decimal: Option<(u32, u32)>,
}

impl ListCounter {
    fn item(&mut self, kind: ListKind, number: u32) -> ListItem {
        match kind {
            ListKind::Bullet => {
                self.last_decimal = None;
                ListItem { kind, id: 0, start: 1 }
            }
            ListKind::Decimal => {
                let id = match self.last_decimal {
                    Some((id, n)) if n + 1 == number => id,
                    _ => {
                        self.next_id += 1;
                        self.next_id
                    }
                };
                self.last_decimal = Some((id, number));
                // `start` solo cuenta en el primer elemento de cada lista.
                ListItem {
                    kind,
                    id,
                    start: number,
                }
            }
        }
    }
    pub(crate) fn reset_run(&mut self) {
        self.last_decimal = None;
    }
}

/// Límites del bloque de texto que se está analizando.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Region {
    pub left: f32,
    pub right: f32,
    /// Borde derecho de todo el contenido de la página (para tabuladores derechos).
    pub edge: f32,
}

struct Line {
    bbox: BBox,
    runs: Vec<Run>,
    bold: bool,
    size: f32,
}

struct Builder {
    lines: Vec<Line>,
    list: Option<ListItem>,
    marker_x: f32,
    text_left: f32,
}

fn strip_marker(runs: &mut Vec<Run>, len: usize) {
    let mut left = len;
    for r in runs.iter_mut() {
        if left == 0 {
            break;
        }
        let cut = left.min(r.text.len());
        let cut = (0..=cut).rev().find(|&i| r.text.is_char_boundary(i)).unwrap_or(0);
        r.text.drain(..cut);
        left -= cut;
    }
    runs.retain(|r| !r.text.is_empty());
}

fn same_style(a: &Run, b: &Run) -> bool {
    a.font == b.font
        && a.size == b.size
        && a.color == b.color
        && a.bold == b.bold
        && a.italic == b.italic
        && a.field == b.field
}

fn push_run(out: &mut Vec<Run>, r: Run) {
    if let Some(last) = out.last_mut()
        && same_style(last, &r)
        && r.field.is_none()
        && last.text != "\t"
        && r.text != "\t"
    {
        last.text.push_str(&r.text);
        return;
    }
    out.push(r);
}

impl Builder {
    fn can_continue(&self, line: &Line, region: &Region) -> bool {
        let prev = self.lines.last().expect("párrafo con al menos una línea");
        let first = &self.lines[0];
        let gap = line.bbox.y0 - prev.bbox.y1;
        let ph = prev.bbox.y1 - prev.bbox.y0;
        if gap > 0.45 * ph || gap < -0.5 * ph {
            return false;
        }
        if (line.size - prev.size).abs() > 0.8 || line.bold != prev.bold {
            return false;
        }
        let col_w = (region.right - region.left).max(1.0);
        if prev.bbox.x1 < region.right - 0.14 * col_w {
            return false;
        }
        let ref_left = if self.lines.len() >= 2 {
            self.lines[1].bbox.x0
        } else if self.list.is_some() {
            self.text_left
        } else {
            first.bbox.x0
        };
        let aligned = (line.bbox.x0 - ref_left).abs() <= 3.0;
        let hanging = self.lines.len() == 1
            && self.list.is_none()
            && line.bbox.x0 < first.bbox.x0 - 3.0
            && first.bbox.x0 - line.bbox.x0 <= 4.0 * line.size;
        aligned || hanging
    }

    fn finish(self, region: &Region) -> Paragraph {
        let n = self.lines.len();
        let bbox = self
            .lines
            .iter()
            .map(|l| l.bbox)
            .reduce(union)
            .expect("al menos una línea");
        let first_x = if self.list.is_some() {
            self.marker_x
        } else {
            self.lines[0].bbox.x0
        };
        let left_x = if n >= 2 {
            self.lines[1].bbox.x0
        } else if self.list.is_some() {
            self.text_left
        } else {
            self.lines[0].bbox.x0
        };
        let line_pitch = if n >= 2 {
            (self.lines[n - 1].bbox.y0 - self.lines[0].bbox.y0) / (n as f32 - 1.0)
        } else {
            bbox.y1 - bbox.y0
        };
        let col_w = (region.right - region.left).max(1.0);
        let align = if n == 1 {
            let l = &self.lines[0].bbox;
            let centered = ((l.x0 + l.x1) / 2.0 - (region.left + region.right) / 2.0).abs() <= 3.0;
            if self.list.is_none() && centered && l.x0 - region.left > 0.1 * col_w {
                Align::Center
            } else if self.list.is_none() && region.right - l.x1 <= 3.0 && l.x0 - region.left > 0.3 * col_w {
                Align::Right
            } else {
                Align::Left
            }
        } else {
            let max_x1 = self.lines.iter().map(|l| l.bbox.x1).fold(f32::MIN, f32::max);
            if n >= 3 && self.lines[..n - 1].iter().all(|l| max_x1 - l.bbox.x1 <= 1.5) {
                Align::Justify
            } else {
                Align::Left
            }
        };

        let mut runs: Vec<Run> = Vec::new();
        for (i, line) in self.lines.into_iter().enumerate() {
            let mut line_runs = line.runs;
            let mut joined_prev_hyphen = false;
            if i > 0 {
                let prev_text: String = runs.iter().map(|r| r.text.as_str()).collect();
                let next_lower = line_runs
                    .first()
                    .and_then(|r| r.text.trim_start().chars().next())
                    .is_some_and(|c| c.is_lowercase());
                let hyphen = prev_text.ends_with('-')
                    && prev_text.chars().rev().nth(1).is_some_and(|c| c.is_alphabetic())
                    && next_lower;
                if hyphen {
                    if let Some(last) = runs.last_mut() {
                        last.text.pop();
                    }
                    joined_prev_hyphen = true;
                } else if !prev_text.ends_with(char::is_whitespace) {
                    let mut sp = runs.last().cloned().expect("hay texto previo");
                    sp.text = " ".into();
                    push_run(&mut runs, sp);
                }
            }
            if let Some(first) = line_runs.first_mut()
                && (i > 0 || joined_prev_hyphen)
            {
                first.text = first.text.trim_start().to_string();
            }
            for r in line_runs {
                push_run(&mut runs, r);
            }
        }
        if let Some(first) = runs.first_mut() {
            first.text = first.text.trim_start().to_string();
        }
        if let Some(last) = runs.last_mut() {
            last.text = last.text.trim_end().to_string();
        }
        runs.retain(|r| !r.text.is_empty());
        Paragraph {
            bbox,
            runs,
            lines: n as u32,
            line_pitch,
            first_x,
            left_x,
            align,
            list: self.list,
            tabs: Vec::new(),
        }
    }
}

fn line_from(frags: &[Frag], strip: usize) -> Option<Line> {
    let mut runs: Vec<Run> = Vec::new();
    for (i, f) in frags.iter().enumerate() {
        if i > 0 {
            let mut sp = runs.last().cloned()?;
            sp.text = " ".into();
            push_run(&mut runs, sp);
        }
        for r in f.runs() {
            push_run(&mut runs, r);
        }
    }
    if strip > 0 {
        strip_marker(&mut runs, strip);
    }
    if runs.iter().all(|r| r.text.trim().is_empty()) {
        return None;
    }
    let bbox = frags.iter().map(|f| f.bbox).reduce(union)?;
    let bold = frags.iter().all(|f| f.mostly_bold());
    let size = frags.iter().map(|f| f.size()).fold(0.0, f32::max);
    Some(Line { bbox, runs, bold, size })
}

/// Convierte filas de texto (sin tablas) en párrafos.
pub(crate) fn build_paragraphs(rows: &[Row], region: Region, counter: &mut ListCounter) -> Vec<Paragraph> {
    let mut out: Vec<Paragraph> = Vec::new();
    let mut cur: Option<Builder> = None;
    let flush = |cur: &mut Option<Builder>, out: &mut Vec<Paragraph>, counter: &mut ListCounter| {
        if let Some(b) = cur.take() {
            let p = b.finish(&region);
            if p.list.is_none() {
                counter.reset_run();
            }
            out.push(p);
        }
    };

    for row in rows {
        let frags = &row.frags;
        if frags.len() >= 2 && is_marker_only(&frags[0].text()) {
            let Some((kind, number, _)) = parse_marker(&frags[0].text()) else {
                continue;
            };
            flush(&mut cur, &mut out, counter);
            let Some(line) = line_from(&frags[1..], 0) else {
                continue;
            };
            cur = Some(Builder {
                list: Some(counter.item(kind, number)),
                marker_x: frags[0].bbox.x0,
                text_left: line.bbox.x0,
                lines: vec![line],
            });
        } else if frags.len() >= 2 {
            flush(&mut cur, &mut out, counter);
            if let Some(p) = tabbed(frags, &region) {
                counter.reset_run();
                out.push(p);
            }
        } else if let Some(f) = frags.first() {
            let text = f.text();
            let marker = parse_marker(&text).filter(|(k, _, _)| *k == ListKind::Bullet || !f.mostly_bold());
            if let Some((kind, number, len)) = marker {
                flush(&mut cur, &mut out, counter);
                // x del texto tras el marcador: se estima con la proporción de caracteres.
                let Some(line) = line_from(std::slice::from_ref(f), len) else {
                    continue;
                };
                let chars_total = text.chars().count().max(1) as f32;
                let marker_chars = text[..len].chars().count() as f32;
                let text_left = f.bbox.x0 + f.width() * marker_chars / chars_total;
                cur = Some(Builder {
                    list: Some(counter.item(kind, number)),
                    marker_x: f.bbox.x0,
                    text_left,
                    lines: vec![line],
                });
            } else if let Some(line) = line_from(std::slice::from_ref(f), 0) {
                match cur.as_mut() {
                    Some(b) if b.can_continue(&line, &region) => b.lines.push(line),
                    _ => {
                        flush(&mut cur, &mut out, counter);
                        cur = Some(Builder {
                            lines: vec![line],
                            list: None,
                            marker_x: 0.0,
                            text_left: 0.0,
                        });
                    }
                }
            }
        }
    }
    flush(&mut cur, &mut out, counter);
    out
}

/// Fila con varios fragmentos separados (p. ej. nombre a la izquierda y fecha a la derecha): un párrafo con tabuladores.
fn tabbed(frags: &[Frag], region: &Region) -> Option<Paragraph> {
    let mut runs: Vec<Run> = Vec::new();
    let mut tabs = Vec::new();
    for (i, f) in frags.iter().enumerate() {
        let fr = f.runs();
        if i > 0 {
            let like = runs.last().or(fr.first())?.clone();
            runs.push(Run::tab(&like));
            tabs.push(if f.bbox.x1 >= region.edge - 3.0 {
                (f.bbox.x1, true)
            } else {
                (f.bbox.x0, false)
            });
        }
        for r in fr {
            push_run(&mut runs, r);
        }
    }
    let bbox = frags.iter().map(|f| f.bbox).reduce(union)?;
    if let Some(first) = runs.first_mut() {
        first.text = first.text.trim_start().to_string();
    }
    Some(Paragraph {
        bbox,
        runs,
        lines: 1,
        line_pitch: bbox.y1 - bbox.y0,
        first_x: bbox.x0,
        left_x: bbox.x0,
        align: Align::Left,
        list: None,
        tabs,
    })
}
