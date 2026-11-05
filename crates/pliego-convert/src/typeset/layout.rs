//! Núcleo del maquetador: caras de letra, ajuste de líneas, páginas y escritura del PDF.

use std::rc::Rc;

use pliego_core::{Error, Result};
use pliego_pdf::native::PdfDocument;
use pliego_pdf::{LinkTarget, NewLink, Rect, Size, WriteOptions};

use super::Typeset;
use super::metrics;
use super::winansi;

/// Medidas de la página.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PageSpec {
    pub width: f32,
    pub height: f32,
    pub margin: f32,
    pub font_size: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Face {
    Roman,
    Bold,
    Italic,
    BoldItalic,
    Mono,
    MonoBold,
}

pub(super) const FACES: [Face; 6] = [
    Face::Roman,
    Face::Bold,
    Face::Italic,
    Face::BoldItalic,
    Face::Mono,
    Face::MonoBold,
];

impl Face {
    fn index(self) -> usize {
        FACES.iter().position(|f| *f == self).unwrap_or(0)
    }

    fn base_font(self) -> &'static str {
        match self {
            Face::Roman => "Times-Roman",
            Face::Bold => "Times-Bold",
            Face::Italic => "Times-Italic",
            Face::BoldItalic => "Times-BoldItalic",
            Face::Mono => "Courier",
            Face::MonoBold => "Courier-Bold",
        }
    }

    /// Ancho de un código WinAnsi en milésimas de em.
    pub(super) fn width(self, byte: u8) -> f32 {
        let i = usize::from(byte.max(32)) - 32;
        f32::from(match self {
            Face::Roman => metrics::ROMAN[i],
            Face::Bold => metrics::BOLD[i],
            Face::Italic => metrics::ITALIC[i],
            Face::BoldItalic => metrics::BOLD_ITALIC[i],
            Face::Mono | Face::MonoBold => 600,
        })
    }

    pub(super) fn with(self, bold: bool, italic: bool) -> Face {
        match (self, bold, italic) {
            (Face::Mono | Face::MonoBold, true, _) => Face::MonoBold,
            (Face::Mono | Face::MonoBold, false, _) => Face::Mono,
            (_, true, true) => Face::BoldItalic,
            (_, true, false) => Face::Bold,
            (_, false, true) => Face::Italic,
            _ => Face::Roman,
        }
    }
}

pub(super) type Color = [u8; 3];

pub(super) const BLACK: Color = [0, 0, 0];
pub(super) const LINK_BLUE: Color = [0, 51, 170];
pub(super) const GRAY_TEXT: Color = [90, 90, 90];
pub(super) const CODE_BG: Color = [240, 240, 240];
pub(super) const HEAD_BG: Color = [228, 228, 228];
pub(super) const RULE: Color = [150, 150, 150];

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Style {
    pub face: Face,
    pub size: f32,
    pub color: Color,
    pub underline: bool,
    pub strike: bool,
    pub link: Option<Rc<str>>,
}

impl Style {
    pub(super) fn plain(face: Face, size: f32) -> Style {
        Style {
            face,
            size,
            color: BLACK,
            underline: false,
            strike: false,
            link: None,
        }
    }
}

/// Texto con estilo, o un salto de línea forzado.
#[derive(Debug, Clone)]
pub(super) enum Piece {
    Text(String, Style),
    Break,
}

#[derive(Debug, Clone)]
struct Item {
    bytes: Vec<u8>,
    style: Style,
    width: f32,
}

/// Una línea ya partida.
#[derive(Debug, Clone)]
pub(super) struct Line {
    items: Vec<Item>,
    pub(super) width: f32,
    /// Cuerpo mayor de la línea (da su altura).
    size: f32,
}

impl Line {
    pub(super) fn size(&self) -> f32 {
        self.size
    }

    pub(super) fn empty_for(size: f32) -> Line {
        Line::empty(size)
    }

    fn empty(size: f32) -> Line {
        Line {
            items: Vec::new(),
            width: 0.0,
            size,
        }
    }

    fn push(&mut self, item: Item) {
        self.width += item.width;
        self.size = self.size.max(item.style.size);
        if let Some(last) = self.items.last_mut()
            && last.style == item.style
        {
            last.bytes.extend_from_slice(&item.bytes);
            last.width += item.width;
            return;
        }
        self.items.push(item);
    }
}

fn width_of(face: Face, size: f32, bytes: &[u8]) -> f32 {
    bytes.iter().map(|&b| face.width(b)).sum::<f32>() * size / 1000.0
}

/// Codifica en WinAnsi transcribiendo lo que se puede; el resto es «?» y se cuenta en `subs`.
pub(super) fn encode_text(s: &str, subs: &mut usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    for c in s.chars() {
        if let Some(b) = winansi::encode(c) {
            out.push(b);
        } else if let Some(t) = winansi::transliterate(c) {
            for tc in t.chars() {
                match winansi::encode(tc) {
                    Some(b) => out.push(b),
                    None if tc == '\n' => out.push(b'\n'),
                    None => {}
                }
            }
        } else if c == '\t' {
            out.extend_from_slice(b"    ");
        } else if c == '\n' || c == '\r' {
            out.push(b'\n');
        } else if c.is_control() {
            // Los de control no se dibujan.
        } else {
            *subs += 1;
            out.push(b'?');
        }
    }
    out
}

/// Parte `pieces` en líneas de como máximo `avail` puntos (por palabras; una palabra más larga se corta).
pub(super) fn wrap(pieces: &[Piece], avail: f32, default_size: f32, subs: &mut usize) -> Vec<Line> {
    let avail = avail.max(1.0);
    let mut lines = Vec::new();
    let mut cur = Line::empty(default_size);
    let mut pending: Vec<Item> = Vec::new();
    let mut pending_w = 0.0f32;

    let finish = |lines: &mut Vec<Line>, cur: &mut Line, pending: &mut Vec<Item>, pw: &mut f32| {
        lines.push(std::mem::replace(cur, Line::empty(default_size)));
        pending.clear();
        *pw = 0.0;
    };

    for piece in pieces {
        let (text, style) = match piece {
            Piece::Break => {
                finish(&mut lines, &mut cur, &mut pending, &mut pending_w);
                continue;
            }
            Piece::Text(t, s) => (t, s),
        };
        let bytes = encode_text(text, subs);
        let mut i = 0;
        while i < bytes.len() {
            let b = bytes[i];
            if b == b'\n' {
                finish(&mut lines, &mut cur, &mut pending, &mut pending_w);
                i += 1;
                continue;
            }
            let is_space = b == b' ' || b == 0xA0;
            let mut j = i;
            while j < bytes.len() && bytes[j] != b'\n' && (bytes[j] == b' ' || bytes[j] == 0xA0) == is_space {
                j += 1;
            }
            let chunk = &bytes[i..j];
            i = j;
            let w = width_of(style.face, style.size, chunk);
            let item = Item {
                bytes: chunk.to_vec(),
                style: style.clone(),
                width: w,
            };
            if is_space {
                if !cur.items.is_empty() {
                    pending_w += w;
                    pending.push(item);
                }
                continue;
            }
            if cur.width + pending_w + w <= avail + 0.01 {
                for p in pending.drain(..) {
                    cur.push(p);
                }
                pending_w = 0.0;
                cur.push(item);
                continue;
            }
            if !cur.items.is_empty() {
                finish(&mut lines, &mut cur, &mut pending, &mut pending_w);
            }
            if w <= avail + 0.01 {
                cur.push(item);
                continue;
            }
            // Palabra más larga que la línea: se corta por caracteres.
            let mut start = 0;
            while start < chunk.len() {
                let mut end = start;
                let mut acc = 0.0;
                while end < chunk.len() {
                    let cw = style.face.width(chunk[end]) * style.size / 1000.0;
                    if acc + cw > avail && end > start {
                        break;
                    }
                    acc += cw;
                    end += 1;
                }
                cur.push(Item {
                    bytes: chunk[start..end].to_vec(),
                    style: style.clone(),
                    width: acc,
                });
                start = end;
                if start < chunk.len() {
                    finish(&mut lines, &mut cur, &mut pending, &mut pending_w);
                }
            }
        }
    }
    if !cur.items.is_empty() || lines.is_empty() {
        lines.push(cur);
    }
    lines
}

/// Texto preformateado en monoespaciado: respeta espacios y saltos, expande tabuladores y parte donde se acaba la
/// línea (preferentemente tras un espacio).
pub(super) fn wrap_pre(text: &str, style: &Style, avail: f32, tab: usize, subs: &mut usize) -> Vec<Line> {
    let cw = style.size * 0.6;
    let cols = ((avail / cw).floor() as usize).max(1);
    let mut lines = Vec::new();
    let text = text.strip_suffix('\n').unwrap_or(text);
    for raw in text.split('\n') {
        let raw = raw.strip_suffix('\r').unwrap_or(raw);
        // Tabuladores a la columna siguiente.
        let mut expanded = String::new();
        let mut col = 0;
        for c in raw.chars() {
            if c == '\t' {
                let n = tab - col % tab;
                expanded.extend(std::iter::repeat_n(' ', n));
                col += n;
            } else {
                expanded.push(c);
                col += 1;
            }
        }
        let bytes = encode_text(&expanded, subs);
        if bytes.is_empty() {
            lines.push(Line::empty(style.size));
            continue;
        }
        let mut rest: &[u8] = &bytes;
        while !rest.is_empty() {
            let take = if rest.len() <= cols {
                rest.len()
            } else {
                // Corte tras el último espacio que cabe; si no hay, en seco.
                match rest[..=cols].iter().rposition(|&b| b == b' ') {
                    Some(p) if p > 0 => p + 1,
                    _ => cols,
                }
            };
            let mut line = Line::empty(style.size);
            let chunk = &rest[..take];
            line.push(Item {
                bytes: chunk.to_vec(),
                style: style.clone(),
                width: chunk.len() as f32 * cw,
            });
            lines.push(line);
            rest = &rest[take..];
        }
    }
    lines
}

pub(super) fn text_width(face: Face, size: f32, bytes: &[u8]) -> f32 {
    width_of(face, size, bytes)
}

/// Anchura de la palabra más larga y de todo el texto en una sola línea (para columnas de tabla).
pub(super) fn measure(pieces: &[Piece], subs: &mut usize) -> (f32, f32) {
    let mut longest = 0.0f32;
    let mut total = 0.0f32;
    let mut word = 0.0f32;
    for p in pieces {
        let Piece::Text(t, s) = p else {
            longest = longest.max(word);
            word = 0.0;
            continue;
        };
        for b in encode_text(t, subs) {
            let w = s.face.width(b) * s.size / 1000.0;
            total += w;
            if b == b' ' || b == 0xA0 || b == b'\n' {
                longest = longest.max(word);
                word = 0.0;
            } else {
                word += w;
            }
        }
    }
    (longest.max(word), total)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Align {
    Left,
    Center,
    Right,
}

enum Op {
    Text {
        x: f32,
        y: f32,
        face: Face,
        size: f32,
        color: Color,
        bytes: Vec<u8>,
    },
    Fill {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: Color,
    },
    Stroke {
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        width: f32,
        color: Color,
    },
}

#[derive(Default)]
struct PageBuf {
    ops: Vec<Op>,
    links: Vec<(Rect, String)>,
}

/// Marcador de lista pendiente de dibujar junto a la primera línea del elemento.
pub(super) struct Marker {
    pub bytes: Vec<u8>,
    pub face: Face,
    pub size: f32,
}

pub(super) struct Typesetter {
    pub(super) spec: PageSpec,
    pages: Vec<PageBuf>,
    /// Cursor vertical en puntos desde arriba de la página.
    pub(super) y: f32,
    pub(super) subs: usize,
    /// Posiciones x de las barras de cita activas.
    pub(super) bars: Vec<f32>,
    pub(super) marker: Option<Marker>,
    /// Borde derecho del marcador (su `x` de salida es `marker_right - ancho`).
    pub(super) marker_right: f32,
}

impl Typesetter {
    pub(super) fn new(spec: PageSpec) -> Self {
        Self {
            spec,
            pages: vec![PageBuf::default()],
            y: spec.margin,
            subs: 0,
            bars: Vec::new(),
            marker: None,
            marker_right: 0.0,
        }
    }

    pub(super) fn top(&self) -> f32 {
        self.spec.margin
    }

    pub(super) fn bottom(&self) -> f32 {
        self.spec.height - self.spec.margin
    }

    pub(super) fn at_top(&self) -> bool {
        self.y <= self.top() + 0.01
    }

    pub(super) fn page_break(&mut self) {
        self.pages.push(PageBuf::default());
        self.y = self.top();
    }

    /// Si no caben `h` puntos más en la página, pasa a la siguiente (salvo que ya esté arriba).
    pub(super) fn need(&mut self, h: f32) {
        if self.y + h > self.bottom() + 0.01 && !self.at_top() {
            self.page_break();
        }
    }

    /// Espacio vertical que no se pone al principio de una página.
    pub(super) fn gap(&mut self, h: f32) {
        if !self.at_top() {
            self.y += h;
        }
    }

    fn page(&mut self) -> &mut PageBuf {
        self.pages.last_mut().expect("siempre hay una página")
    }

    pub(super) fn fill(&mut self, x: f32, y: f32, w: f32, h: f32, color: Color) {
        self.page().ops.push(Op::Fill { x, y, w, h, color });
    }

    pub(super) fn stroke(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, width: f32, color: Color) {
        self.page().ops.push(Op::Stroke {
            x0,
            y0,
            x1,
            y1,
            width,
            color,
        });
    }

    /// Altura de una línea con el cuerpo `size`.
    pub(super) fn line_height(size: f32) -> f32 {
        size * 1.35
    }

    /// Dibuja una línea con su borde superior en `y` (sin avanzar el cursor); `x` y `width` son la caja disponible.
    pub(super) fn draw_line(&mut self, line: &Line, x: f32, width: f32, align: Align, y: f32) {
        let lh = Self::line_height(line.size);
        let baseline = y + (lh - line.size) / 2.0 + line.size * 0.78;
        let mut cx = match align {
            Align::Left => x,
            Align::Center => x + ((width - line.width) / 2.0).max(0.0),
            Align::Right => x + (width - line.width).max(0.0),
        };
        for item in &line.items {
            let s = &item.style;
            if item.bytes.iter().any(|&b| b != b' ') {
                self.page().ops.push(Op::Text {
                    x: cx,
                    y: baseline,
                    face: s.face,
                    size: s.size,
                    color: s.color,
                    bytes: item.bytes.clone(),
                });
            }
            if s.underline {
                self.stroke(
                    cx,
                    baseline + s.size * 0.12,
                    cx + item.width,
                    baseline + s.size * 0.12,
                    0.6,
                    s.color,
                );
            }
            if s.strike {
                self.stroke(
                    cx,
                    baseline - s.size * 0.28,
                    cx + item.width,
                    baseline - s.size * 0.28,
                    0.6,
                    s.color,
                );
            }
            if let Some(uri) = &s.link {
                let r = Rect::new(cx, y, cx + item.width, y + lh);
                self.page().links.push((r, uri.to_string()));
            }
            cx += item.width;
        }
    }

    /// Coloca las líneas desde el cursor, pasando de página cuando hace falta. `bg` pinta un fondo detrás.
    pub(super) fn place_lines(&mut self, lines: &[Line], x: f32, width: f32, align: Align, bg: Option<Color>) {
        for line in lines {
            let lh = Self::line_height(line.size);
            self.need(lh);
            let y = self.y;
            if let Some(c) = bg {
                self.fill(x - 3.0, y, width + 6.0, lh, c);
            }
            for bx in self.bars.clone() {
                self.fill(bx, y, 1.5, lh, RULE);
            }
            if let Some(m) = self.marker.take() {
                let mw = text_width(m.face, m.size, &m.bytes);
                let marker_line = Line {
                    items: vec![Item {
                        width: mw,
                        bytes: m.bytes,
                        style: Style::plain(m.face, m.size),
                    }],
                    width: mw,
                    size: m.size,
                };
                let mx = self.marker_right - mw;
                self.draw_line(
                    &marker_line,
                    mx,
                    mw,
                    Align::Left,
                    y + (lh - Self::line_height(m.size)) / 2.0,
                );
            }
            self.draw_line(line, x, width, align, y);
            self.y += lh;
        }
    }

    /// Cierra el documento: escribe cada página en un PDF nuevo.
    pub(super) fn finish(mut self) -> Result<Typeset> {
        // Una última página vacía por un salto final no se escribe.
        if self.pages.len() > 1 && self.pages.last().is_some_and(|p| p.ops.is_empty()) {
            self.pages.pop();
        }
        let mut pdf = PdfDocument::new();
        let mut font_objs: [Option<pliego_pdf::native::Object>; 6] = Default::default();
        let spec = self.spec;
        let count = self.pages.len();
        for buf in self.pages {
            let mut page = pdf.new_page(Size::new(spec.width, spec.height))?;
            let mut used = [false; 6];
            let mut content = String::with_capacity(buf.ops.len() * 64);
            for op in &buf.ops {
                match op {
                    Op::Text {
                        x,
                        y,
                        face,
                        size,
                        color,
                        bytes,
                    } => {
                        used[face.index()] = true;
                        content.push_str(&format!(
                            "BT /F{} {} Tf {} rg {} {} Td ({}) Tj ET\n",
                            face.index(),
                            num(*size),
                            rgb(*color),
                            num(*x),
                            num(spec.height - y),
                            escape(bytes)
                        ));
                    }
                    Op::Fill { x, y, w, h, color } => content.push_str(&format!(
                        "{} rg {} {} {} {} re f\n",
                        rgb(*color),
                        num(*x),
                        num(spec.height - y - h),
                        num(*w),
                        num(*h)
                    )),
                    Op::Stroke {
                        x0,
                        y0,
                        x1,
                        y1,
                        width,
                        color,
                    } => content.push_str(&format!(
                        "{} RG {} w {} {} m {} {} l S\n",
                        rgb(*color),
                        num(*width),
                        num(*x0),
                        num(spec.height - y0),
                        num(*x1),
                        num(spec.height - y1)
                    )),
                }
            }
            page.insert_contents(&mut pdf, content.as_bytes(), true)?;
            let mut fonts = pdf.new_dict()?;
            for face in FACES {
                if !used[face.index()] {
                    continue;
                }
                let obj = match &font_objs[face.index()] {
                    Some(o) => o.clone(),
                    None => {
                        let mut d = pdf.new_dict()?;
                        d.dict_put_name("Type", "Font")?;
                        d.dict_put_name("Subtype", "Type1")?;
                        d.dict_put_name("BaseFont", face.base_font())?;
                        d.dict_put_name("Encoding", "WinAnsiEncoding")?;
                        let r = pdf.add_object(&d)?;
                        font_objs[face.index()] = Some(r.clone());
                        r
                    }
                };
                fonts.dict_put(&format!("F{}", face.index()), obj)?;
            }
            let mut res = page.resources()?;
            res.dict_put("Font", fonts)?;
            let links: Vec<NewLink> = buf
                .links
                .iter()
                .map(|(r, u)| NewLink {
                    bounds: *r,
                    target: LinkTarget::Uri(ascii_uri(u)),
                })
                .collect();
            if !links.is_empty() {
                page.add_links(&mut pdf, &links)?;
            }
        }
        if count == 0 {
            return Err(Error::Other("el maquetador no produjo ninguna página".into()));
        }
        Ok(Typeset {
            pdf,
            pages: count,
            substituted: self.subs,
        })
    }
}

/// Escribe el PDF resultante con flujos comprimidos.
pub(crate) fn save_bytes(pdf: &PdfDocument) -> Result<Vec<u8>> {
    let mut o = WriteOptions::new();
    o.set_compress(true);
    pdf.to_bytes(&o)
}

fn num(v: f32) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-0" {
        "0".into()
    } else {
        s.into()
    }
}

fn rgb(c: Color) -> String {
    format!(
        "{} {} {}",
        num(f32::from(c[0]) / 255.0),
        num(f32::from(c[1]) / 255.0),
        num(f32::from(c[2]) / 255.0)
    )
}

/// Cadena literal PDF: escapa `( ) \` y los bytes no ASCII en octal.
fn escape(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() + 8);
    for &b in bytes {
        match b {
            b'(' | b')' | b'\\' => {
                s.push('\\');
                s.push(b as char);
            }
            0x20..=0x7E => s.push(b as char),
            _ => s.push_str(&format!("\\{b:03o}")),
        }
    }
    s
}

/// Las direcciones de enlace van en ASCII: lo que no lo es se codifica en porcentaje.
fn ascii_uri(u: &str) -> String {
    let mut out = String::with_capacity(u.len());
    for b in u.bytes() {
        if b.is_ascii_graphic() {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

// --- Texto plano ---

pub(super) fn typeset_plain(text: &str, spec: &PageSpec) -> Result<Typeset> {
    let mut ts = Typesetter::new(*spec);
    let width = spec.width - 2.0 * spec.margin;
    let style = Style::plain(Face::Mono, spec.font_size);
    let mut subs = 0;
    let lines = wrap_pre(text, &style, width, 4, &mut subs);
    ts.subs += subs;
    ts.place_lines(&lines, spec.margin, width, Align::Left, None);
    ts.finish()
}
