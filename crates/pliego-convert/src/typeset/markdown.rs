//! Markdown → PDF: recorre los eventos de `pulldown-cmark` y los coloca con el [`Typesetter`].

use std::rc::Rc;

use pliego_core::Result;
use pulldown_cmark::{Alignment, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use super::Typeset;
use super::layout::{
    Align, CODE_BG, Face, GRAY_TEXT, HEAD_BG, LINK_BLUE, Line, Marker, PageSpec, Piece, RULE, Style, Typesetter,
    measure, text_width, wrap, wrap_pre,
};

struct ListCtx {
    ordered: bool,
    next: u64,
}

#[derive(Default)]
struct TableBuilder {
    aligns: Vec<Alignment>,
    rows: Vec<(bool, Vec<Vec<Piece>>)>,
    current: Vec<Vec<Piece>>,
}

struct Md {
    ts: Typesetter,
    base: f32,
    pieces: Vec<Piece>,
    bold: u32,
    italic: u32,
    strike: u32,
    code: bool,
    link: Option<Rc<str>>,
    heading: Option<HeadingLevel>,
    indents: Vec<f32>,
    lists: Vec<ListCtx>,
    code_block: Option<String>,
    html_block: bool,
    table: Option<TableBuilder>,
    in_head: bool,
}

pub(super) fn typeset(md: &str, spec: &PageSpec) -> Result<Typeset> {
    let parser = Parser::new_ext(
        md,
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS,
    );
    let mut m = Md {
        ts: Typesetter::new(*spec),
        base: spec.font_size,
        pieces: Vec::new(),
        bold: 0,
        italic: 0,
        strike: 0,
        code: false,
        link: None,
        heading: None,
        indents: Vec::new(),
        lists: Vec::new(),
        code_block: None,
        html_block: false,
        table: None,
        in_head: false,
    };
    for ev in parser {
        m.event(ev);
    }
    m.flush(0.0);
    m.ts.finish()
}

fn heading_scale(level: HeadingLevel) -> f32 {
    match level {
        HeadingLevel::H1 => 1.8,
        HeadingLevel::H2 => 1.45,
        HeadingLevel::H3 => 1.2,
        HeadingLevel::H4 => 1.0,
        HeadingLevel::H5 => 0.9,
        HeadingLevel::H6 => 0.85,
    }
}

/// Quita las etiquetas de un fragmento HTML y decodifica las entidades más comunes.
fn strip_tags(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&nbsp;", "\u{a0}")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&amp;", "&")
}

impl Md {
    fn left(&self) -> f32 {
        self.ts.spec.margin + self.indents.iter().sum::<f32>()
    }

    fn width(&self) -> f32 {
        (self.ts.spec.width - self.ts.spec.margin - self.left()).max(20.0)
    }

    fn style(&self) -> Style {
        let (mut face, mut size) = if let Some(h) = self.heading {
            (Face::Bold, self.base * heading_scale(h))
        } else {
            (Face::Roman, self.base)
        };
        let bold = self.bold > 0 || self.heading.is_some() || self.in_head;
        let italic = self.italic > 0;
        if self.code {
            face = Face::Mono.with(bold, false);
            size *= 0.92;
        } else {
            face = face.with(bold, italic);
        }
        let mut s = Style::plain(face, size);
        s.strike = self.strike > 0;
        if let Some(l) = &self.link {
            s.color = LINK_BLUE;
            s.underline = true;
            s.link = Some(l.clone());
        }
        s
    }

    fn push_text(&mut self, text: &str) {
        let style = self.style();
        self.pieces.push(Piece::Text(text.to_owned(), style));
    }

    /// Coloca el párrafo pendiente (si lo hay) y deja `gap` puntos debajo.
    fn flush(&mut self, gap: f32) {
        if self.table.is_some() {
            return;
        }
        let has_content = self.pieces.iter().any(|p| match p {
            Piece::Text(t, _) => !t.trim().is_empty(),
            Piece::Break => true,
        });
        if !has_content {
            self.pieces.clear();
            return;
        }
        let pieces = std::mem::take(&mut self.pieces);
        let (left, width) = (self.left(), self.width());
        let lines = wrap(&pieces, width, self.base, &mut self.ts.subs);
        if self.heading.is_some() {
            // Un título no se queda solo al final de la página.
            let own: f32 = lines.iter().map(|l| Typesetter::line_height(l.size())).sum();
            self.ts.need(own + Typesetter::line_height(self.base) * 2.0);
        }
        self.sync_marker();
        self.ts.place_lines(&lines, left, width, Align::Left, None);
        self.ts.gap(gap);
    }

    fn sync_marker(&mut self) {
        self.ts.marker_right = self.left() - 5.0;
    }

    fn event(&mut self, ev: Event<'_>) {
        match ev {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(t) => {
                if let Some(code) = &mut self.code_block {
                    code.push_str(&t);
                } else if self.html_block {
                    let s = strip_tags(&t);
                    self.push_text(&s);
                } else {
                    self.push_text(&t);
                }
            }
            Event::Code(t) => {
                self.code = true;
                self.push_text(&t);
                self.code = false;
            }
            Event::Html(t) => {
                let s = strip_tags(&t);
                if !s.trim().is_empty() {
                    self.push_text(&s);
                }
            }
            Event::InlineHtml(t) => {
                let tag = t.trim().to_ascii_lowercase();
                if tag.starts_with("<br") {
                    self.pieces.push(Piece::Break);
                }
            }
            Event::SoftBreak => self.push_text(" "),
            Event::HardBreak => self.pieces.push(Piece::Break),
            Event::Rule => {
                self.flush(0.0);
                self.ts.gap(self.base * 0.6);
                self.ts.need(self.base);
                let (l, w) = (self.left(), self.width());
                let y = self.ts.y;
                self.ts.stroke(l, y, l + w, y, 0.8, RULE);
                self.ts.y += self.base * 0.8;
            }
            Event::TaskListMarker(done) => {
                if self.ts.marker.is_some() {
                    let mut bytes = Vec::new();
                    bytes.extend_from_slice(if done { b"[x]" } else { b"[ ]" });
                    let size = self.base;
                    let w = text_width(Face::Mono, size, &bytes);
                    self.ts.marker = Some(Marker {
                        bytes,
                        face: Face::Mono,
                        size,
                    });
                    if let Some(last) = self.indents.last_mut() {
                        *last = last.max(w + 8.0);
                    }
                    self.sync_marker();
                }
            }
            Event::FootnoteReference(t) => self.push_text(&format!("[{t}]")),
            _ => {}
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => self.flush(self.base * 0.3),
            Tag::Heading { level, .. } => {
                self.flush(self.base * 0.3);
                self.ts.gap(self.base * 0.7);
                self.heading = Some(level);
            }
            Tag::BlockQuote(_) => {
                self.flush(self.base * 0.3);
                let x = self.left() + 2.0;
                self.ts.bars.push(x);
                self.indents.push(14.0);
            }
            Tag::CodeBlock(_) => {
                self.flush(self.base * 0.3);
                self.code_block = Some(String::new());
            }
            Tag::HtmlBlock => {
                self.flush(self.base * 0.3);
                self.html_block = true;
            }
            Tag::List(start) => {
                self.flush(self.base * 0.2);
                self.lists.push(ListCtx {
                    ordered: start.is_some(),
                    next: start.unwrap_or(1),
                });
            }
            Tag::Item => {
                self.flush(self.base * 0.2);
                let level = self.lists.len();
                let (bytes, indent) = match self.lists.last_mut() {
                    Some(l) if l.ordered => {
                        let b = format!("{}.", l.next).into_bytes();
                        l.next += 1;
                        let w = text_width(Face::Roman, self.base, &b) + 9.0;
                        (b, w.max(18.0))
                    }
                    _ => {
                        let glyph = match level {
                            0 | 1 => 0x95,
                            2 => 0x96,
                            _ => 0xB7,
                        };
                        (vec![glyph], 16.0)
                    }
                };
                self.indents.push(indent);
                self.ts.marker = Some(Marker {
                    bytes,
                    face: Face::Roman,
                    size: self.base,
                });
                self.sync_marker();
            }
            Tag::Table(aligns) => {
                self.flush(self.base * 0.3);
                self.table = Some(TableBuilder {
                    aligns,
                    ..Default::default()
                });
            }
            Tag::TableHead => self.in_head = true,
            Tag::TableRow => {}
            Tag::TableCell => self.pieces.clear(),
            Tag::Emphasis => self.italic += 1,
            Tag::Strong => self.bold += 1,
            Tag::Strikethrough => self.strike += 1,
            Tag::Link { dest_url, .. } => self.link = Some(Rc::from(dest_url.as_ref())),
            Tag::Image { .. } => self.push_text("["),
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => {
                let gap = if self.lists.is_empty() {
                    self.base * 0.6
                } else {
                    self.base * 0.25
                };
                self.flush(gap);
            }
            TagEnd::Heading(_) => {
                self.flush(self.base * 0.4);
                self.heading = None;
            }
            TagEnd::BlockQuote(_) => {
                self.flush(self.base * 0.3);
                self.indents.pop();
                self.ts.bars.pop();
                self.ts.gap(self.base * 0.3);
            }
            TagEnd::CodeBlock => {
                let code = self.code_block.take().unwrap_or_default();
                self.code_block_out(&code);
            }
            TagEnd::HtmlBlock => {
                self.flush(self.base * 0.6);
                self.html_block = false;
            }
            TagEnd::List(_) => {
                self.flush(self.base * 0.2);
                self.lists.pop();
                if self.lists.is_empty() {
                    self.ts.gap(self.base * 0.5);
                }
            }
            TagEnd::Item => {
                self.flush(self.base * 0.2);
                if self.ts.marker.is_some() {
                    // Elemento vacío: se dibuja solo el marcador.
                    self.pieces.push(Piece::Text(" ".into(), self.style()));
                    let empty = vec![Line::empty_for(self.base)];
                    let (l, w) = (self.left(), self.width());
                    self.sync_marker();
                    self.ts.place_lines(&empty, l, w, Align::Left, None);
                    self.pieces.clear();
                }
                self.ts.marker = None;
                self.indents.pop();
            }
            TagEnd::Table => {
                if let Some(t) = self.table.take() {
                    self.render_table(t);
                }
            }
            TagEnd::TableHead => {
                self.in_head = false;
                if let Some(t) = &mut self.table {
                    let row = std::mem::take(&mut t.current);
                    t.rows.push((true, row));
                }
            }
            TagEnd::TableRow => {
                if let Some(t) = &mut self.table {
                    let row = std::mem::take(&mut t.current);
                    t.rows.push((false, row));
                }
            }
            TagEnd::TableCell => {
                let cell = std::mem::take(&mut self.pieces);
                if let Some(t) = &mut self.table {
                    t.current.push(cell);
                }
            }
            TagEnd::Emphasis => self.italic = self.italic.saturating_sub(1),
            TagEnd::Strong => self.bold = self.bold.saturating_sub(1),
            TagEnd::Strikethrough => self.strike = self.strike.saturating_sub(1),
            TagEnd::Link => self.link = None,
            TagEnd::Image => self.push_text("]"),
            _ => {}
        }
    }

    fn code_block_out(&mut self, code: &str) {
        let size = self.base * 0.9;
        let style = Style::plain(Face::Mono, size);
        let (left, width) = (self.left(), self.width());
        let lines = wrap_pre(code, &style, width - 8.0, 4, &mut self.ts.subs);
        self.ts.gap(self.base * 0.2);
        self.ts.y += 3.0;
        self.sync_marker();
        self.ts
            .place_lines(&lines, left + 4.0, width - 8.0, Align::Left, Some(CODE_BG));
        self.ts.y += 3.0;
        self.ts.gap(self.base * 0.5);
    }

    fn render_table(&mut self, t: TableBuilder) {
        let cols = t.rows.iter().map(|(_, r)| r.len()).max().unwrap_or(0);
        if cols == 0 {
            return;
        }
        let (left, avail) = (self.left(), self.width());
        let (padx, pady) = (4.0f32, 2.5f32);
        // Anchos: lo que pide cada columna (palabra más larga, texto entero en una línea).
        let mut min = vec![0.0f32; cols];
        let mut nat = vec![0.0f32; cols];
        for (head, row) in &t.rows {
            for (c, cell) in row.iter().enumerate() {
                let cell = bold_if(cell, *head);
                let (longest, total) = measure(&cell, &mut self.ts.subs);
                min[c] = min[c].max(longest + 2.0 * padx);
                nat[c] = nat[c].max(total + 2.0 * padx);
            }
        }
        let widths = fit_columns(&min, &nat, avail);
        self.ts.gap(self.base * 0.2);
        let total_w: f32 = widths.iter().sum();
        let mut first_row = true;
        for (head, row) in &t.rows {
            let mut cells: Vec<Vec<Line>> = Vec::new();
            let mut row_h = 0.0f32;
            for (c, &w) in widths.iter().enumerate() {
                let pieces = row.get(c).map(|p| bold_if(p, *head)).unwrap_or_default();
                let lines = wrap(&pieces, (w - 2.0 * padx).max(8.0), self.base, &mut self.ts.subs);
                let h: f32 = lines.iter().map(|l| Typesetter::line_height(l.size())).sum();
                row_h = row_h.max(h);
                cells.push(lines);
            }
            row_h += 2.0 * pady;
            self.ts.need(row_h);
            let y = self.ts.y;
            if *head {
                self.ts.fill(left, y, total_w, row_h, HEAD_BG);
            }
            let mut x = left;
            for (c, lines) in cells.iter().enumerate() {
                let align = match t.aligns.get(c) {
                    Some(Alignment::Center) => Align::Center,
                    Some(Alignment::Right) => Align::Right,
                    _ => Align::Left,
                };
                let mut cy = y + pady;
                for line in lines {
                    self.ts.draw_line(line, x + padx, widths[c] - 2.0 * padx, align, cy);
                    cy += Typesetter::line_height(line.size());
                }
                x += widths[c];
            }
            // Bordes de la fila.
            let mut x = left;
            self.ts.stroke(left, y, left + total_w, y, 0.5, GRAY_TEXT);
            for w in &widths {
                self.ts.stroke(x, y, x, y + row_h, 0.5, GRAY_TEXT);
                x += w;
            }
            self.ts.stroke(x, y, x, y + row_h, 0.5, GRAY_TEXT);
            self.ts.y += row_h;
            first_row = false;
        }
        if !first_row {
            let y = self.ts.y;
            self.ts.stroke(left, y, left + total_w, y, 0.5, GRAY_TEXT);
        }
        self.ts.gap(self.base * 0.7);
    }
}

fn bold_if(cell: &[Piece], bold: bool) -> Vec<Piece> {
    cell.iter()
        .map(|p| match p {
            Piece::Text(t, s) if bold => {
                let mut s = s.clone();
                s.face = s.face.with(true, matches!(s.face, Face::Italic | Face::BoldItalic));
                Piece::Text(t.clone(), s)
            }
            other => other.clone(),
        })
        .collect()
}

/// Reparte `avail` entre las columnas: el ancho natural si cabe; si no, cada columna parte de su mínimo (la palabra más
/// larga) y el resto se reparte por igual entre las que aún quieren más, de modo que las columnas estrechas conservan
/// su ancho natural y solo se encogen las largas. Si ni los mínimos caben, las palabras más largas se parten.
fn fit_columns(min: &[f32], nat: &[f32], avail: f32) -> Vec<f32> {
    if nat.iter().sum::<f32>() <= avail {
        return nat.to_vec();
    }
    if min.iter().sum::<f32>() <= avail {
        return water_fill(min, nat, avail);
    }
    // Ni los mínimos caben: cada columna tiene al menos para dos letras y las demás se reparten el resto.
    let floor: Vec<f32> = min.iter().map(|m| m.min(28.0)).collect();
    let floor_sum: f32 = floor.iter().sum();
    if floor_sum >= avail {
        let k = avail / floor_sum.max(1.0);
        return floor.iter().map(|f| f * k).collect();
    }
    water_fill(&floor, min, avail)
}

/// Parte de `base` y reparte lo que sobra de `avail` hasta `want`, empezando por las columnas que menos piden.
fn water_fill(base: &[f32], want: &[f32], avail: f32) -> Vec<f32> {
    let mut widths = base.to_vec();
    let mut spare = avail - base.iter().sum::<f32>();
    let mut order: Vec<usize> = (0..base.len()).collect();
    order.sort_by(|&a, &b| (want[a] - base[a]).total_cmp(&(want[b] - base[b])));
    let mut left = order.len();
    for i in order {
        let give = (want[i] - base[i]).min(spare / left as f32).max(0.0);
        widths[i] += give;
        spare -= give;
        left -= 1;
    }
    widths
}
