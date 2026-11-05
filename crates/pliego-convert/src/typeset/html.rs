//! HTML básico → PDF (W40, bloque 8). Lee el HTML con un analizador mínimo y lo traduce a los mismos eventos que
//! `pulldown-cmark`, de modo que lo coloca el mismo maquetador que el Markdown.
//!
//! # Qué admite
//! Títulos `h1`-`h6`, párrafos y bloques (`p`, `div`, `section`…), `br`, `hr`, negrita, cursiva, tachado, código en línea
//! y en bloque (`pre`), citas, listas ordenadas y sin ordenar anidadas, tablas (`th` como cabecera), enlaces con su
//! dirección y entidades con nombre y numéricas. Lo que no reconoce (`span`, `font`…) deja pasar su contenido.
//!
//! # Qué no
//! CSS (ni `style` ni hojas externas), JavaScript, imágenes (sale su texto alternativo entre corchetes), formularios,
//! `colspan`/`rowspan`, flex, grid, posicionamiento y caracteres fuera de Windows-1252 (se sustituyen por «?»).
//! Se descarta el contenido de `head`, `script`, `style`, `svg`, `iframe` y similares.

use pliego_core::Result;
use pulldown_cmark::{Alignment, BlockQuoteKind, CodeBlockKind, CowStr, Event, HeadingLevel, LinkType, Tag, TagEnd};

use super::Typeset;
use super::layout::PageSpec;
use super::markdown;

/// Profundidad máxima de elementos abiertos: más allá, las etiquetas se ignoran (el contenido se conserva).
const MAX_DEPTH: usize = 256;

pub(super) fn typeset(html: &str, spec: &PageSpec) -> Result<Typeset> {
    let events = Translator::new().run(html);
    markdown::typeset_events(events, spec)
}

/// Elemento abierto que produce eventos.
#[derive(Clone, PartialEq)]
enum Open {
    Para,
    Heading(HeadingLevel),
    Quote,
    List(bool),
    Item,
    Pre,
    Link,
    Em,
    Strong,
    Strike,
    Code,
    Table,
    Section(SectionKind),
    Row,
    Cell,
    /// Elemento sin eventos propios (`span`, `body`…). Hace falta para emparejar su cierre.
    Plain(String),
}

#[derive(Clone, Copy, PartialEq)]
enum SectionKind {
    Head,
    Body,
}

struct Translator {
    out: Vec<Event<'static>>,
    stack: Vec<Open>,
    /// Texto acumulado de `<code>` en línea (se emite como un solo evento).
    code: Option<String>,
    /// Eventos de la fila `<tr>` en curso, y si ya tuvo alguna celda `<td>`.
    row: Option<Vec<Event<'static>>>,
    row_has_td: bool,
    /// Último carácter emitido fue un espacio (o estamos al inicio de línea): se colapsan los espacios.
    pending_space: bool,
    at_start: bool,
    /// Primer salto de línea tras `<pre>`: se ignora, como hacen los navegadores.
    pre_first: bool,
    /// Profundidad dentro de un elemento cuyo contenido se descarta.
    skip: usize,
}

const SKIPPED: &[&str] = &[
    "head", "script", "style", "template", "noscript", "svg", "canvas", "iframe", "object", "embed", "select",
    "textarea", "audio", "video", "math", "title",
];

const VOID: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source", "track", "wbr",
];

const BLOCKS: &[&str] = &[
    "p",
    "div",
    "section",
    "article",
    "header",
    "footer",
    "main",
    "nav",
    "aside",
    "figure",
    "figcaption",
    "address",
    "form",
    "fieldset",
    "dl",
    "dt",
    "dd",
    "details",
    "summary",
    "center",
    "body",
    "html",
];

impl Translator {
    fn new() -> Self {
        Self {
            out: Vec::new(),
            stack: Vec::new(),
            code: None,
            row: None,
            row_has_td: false,
            pending_space: false,
            at_start: true,
            pre_first: false,
            skip: 0,
        }
    }

    fn run(mut self, html: &str) -> Vec<Event<'static>> {
        let b = html.as_bytes();
        let mut i = 0;
        let mut text_start = 0;
        while i < b.len() {
            if b[i] != b'<' {
                i += 1;
                continue;
            }
            let Some(next) = markup_end(html, i) else {
                // `<` suelto sin cierre: es texto.
                i += 1;
                continue;
            };
            self.text(&html[text_start..i]);
            self.markup(&html[i..next.end]);
            if let Some(raw) = next.raw_until {
                // Contenido de `script`/`style`: se salta hasta el cierre.
                i = find_close(html, next.end, raw);
                text_start = i;
                continue;
            }
            i = next.end;
            text_start = i;
        }
        self.text(&html[text_start..]);
        self.close_all();
        self.out
    }

    // ---- salida ----

    fn emit(&mut self, ev: Event<'static>) {
        match &mut self.row {
            Some(r) => r.push(ev),
            None => self.out.push(ev),
        }
    }

    fn start(&mut self, tag: Tag<'static>) {
        self.emit(Event::Start(tag));
    }

    fn end(&mut self, tag: TagEnd) {
        self.emit(Event::End(tag));
    }

    fn in_pre(&self) -> bool {
        self.stack.contains(&Open::Pre)
    }

    // ---- texto ----

    fn text(&mut self, raw: &str) {
        if raw.is_empty() || self.skip > 0 {
            return;
        }
        let decoded = decode_entities(raw);
        if let Some(code) = &mut self.code {
            code.push_str(&decoded);
            return;
        }
        if self.in_pre() {
            let mut t = decoded.as_str();
            if std::mem::take(&mut self.pre_first) {
                t = t.strip_prefix("\r\n").or_else(|| t.strip_prefix('\n')).unwrap_or(t);
            }
            if !t.is_empty() {
                self.emit(Event::Text(CowStr::from(t.to_owned())));
            }
            return;
        }
        // Colapso de espacios: una racha de blancos es un solo espacio, y no se escribe al inicio de línea.
        let mut buf = String::new();
        for c in decoded.chars() {
            if c.is_ascii_whitespace() || c == '\u{c}' {
                self.pending_space = true;
            } else {
                if self.pending_space && !self.at_start {
                    buf.push(' ');
                }
                self.pending_space = false;
                self.at_start = false;
                buf.push(c);
            }
        }
        if !buf.is_empty() {
            self.emit(Event::Text(CowStr::from(buf)));
        }
    }

    // ---- etiquetas ----

    fn markup(&mut self, tag: &str) {
        if tag.starts_with("<!") || tag.starts_with("<?") {
            return;
        }
        let closing = tag.starts_with("</");
        let inner = tag
            .trim_start_matches("</")
            .trim_start_matches('<')
            .trim_end_matches('>');
        let self_closing = inner.ends_with('/');
        let inner = inner.strip_suffix('/').unwrap_or(inner);
        let name_end = inner
            .find(|c: char| c.is_ascii_whitespace() || c == '/')
            .unwrap_or(inner.len());
        let name = inner[..name_end].to_ascii_lowercase();
        if name.is_empty() {
            return;
        }
        let attrs = &inner[name_end..];

        if self.skip > 0 && name == "body" && !closing {
            // `head` sin cerrar: el cuerpo empieza aquí.
            self.skip = 0;
        }
        if self_closing && SKIPPED.contains(&name.as_str()) {
            return;
        }
        if self.skip > 0 {
            if SKIPPED.contains(&name.as_str()) {
                if closing {
                    self.skip -= 1;
                } else if !VOID.contains(&name.as_str()) {
                    self.skip += 1;
                }
            }
            return;
        }
        if closing {
            self.close(&name);
        } else {
            self.open(&name, attrs);
        }
    }

    fn open(&mut self, name: &str, attrs: &str) {
        if SKIPPED.contains(&name) {
            self.skip = 1;
            return;
        }
        match name {
            "br" => {
                self.break_line();
                return;
            }
            "hr" => {
                self.end_inline_block();
                self.emit(Event::Rule);
                return;
            }
            "img" => {
                if let Some(alt) = attr(attrs, "alt").filter(|a| !a.trim().is_empty()) {
                    self.start(Tag::Image {
                        link_type: LinkType::Inline,
                        dest_url: CowStr::from(String::new()),
                        title: CowStr::from(String::new()),
                        id: CowStr::from(String::new()),
                    });
                    self.text_event(&alt);
                    self.end(TagEnd::Image);
                }
                return;
            }
            _ if VOID.contains(&name) => return,
            _ => {}
        }
        if self.stack.len() >= MAX_DEPTH {
            return;
        }
        let kind = match name {
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                self.close_paragraph();
                let level = match name {
                    "h1" => HeadingLevel::H1,
                    "h2" => HeadingLevel::H2,
                    "h3" => HeadingLevel::H3,
                    "h4" => HeadingLevel::H4,
                    "h5" => HeadingLevel::H5,
                    _ => HeadingLevel::H6,
                };
                self.start(Tag::Heading {
                    level,
                    id: None,
                    classes: Vec::new(),
                    attrs: Vec::new(),
                });
                self.block_boundary();
                Open::Heading(level)
            }
            "p" => {
                self.close_paragraph();
                self.start(Tag::Paragraph);
                self.block_boundary();
                Open::Para
            }
            _ if BLOCKS.contains(&name) => {
                self.close_paragraph();
                self.start(Tag::Paragraph);
                self.block_boundary();
                Open::Plain(name.to_owned())
            }
            "blockquote" => {
                self.close_paragraph();
                self.start(Tag::BlockQuote(None::<BlockQuoteKind>));
                self.block_boundary();
                Open::Quote
            }
            "ul" | "ol" => {
                self.close_paragraph();
                let ordered = name == "ol";
                let start = ordered.then(|| attr(attrs, "start").and_then(|s| s.trim().parse().ok()).unwrap_or(1));
                self.start(Tag::List(start));
                self.block_boundary();
                Open::List(ordered)
            }
            "li" => {
                self.close_to_list_scope(&Open::Item);
                self.start(Tag::Item);
                self.block_boundary();
                Open::Item
            }
            "pre" => {
                self.close_paragraph();
                self.start(Tag::CodeBlock(CodeBlockKind::Indented));
                self.pre_first = true;
                Open::Pre
            }
            "a" => match attr(attrs, "href").and_then(|h| safe_href(&h)) {
                Some(url) => {
                    self.start(Tag::Link {
                        link_type: LinkType::Inline,
                        dest_url: CowStr::from(url),
                        title: CowStr::from(String::new()),
                        id: CowStr::from(String::new()),
                    });
                    Open::Link
                }
                None => Open::Plain("a".into()),
            },
            "b" | "strong" => {
                self.start(Tag::Strong);
                Open::Strong
            }
            "i" | "em" | "cite" | "dfn" | "var" => {
                self.start(Tag::Emphasis);
                Open::Em
            }
            "s" | "strike" | "del" => {
                self.start(Tag::Strikethrough);
                Open::Strike
            }
            "code" | "kbd" | "samp" | "tt" if !self.in_pre() => {
                if self.pending_space && !self.at_start {
                    self.emit(Event::Text(CowStr::from(" ".to_owned())));
                }
                self.pending_space = false;
                self.at_start = false;
                self.code = Some(String::new());
                Open::Code
            }
            "table" => {
                self.close_paragraph();
                self.start(Tag::Table(Vec::<Alignment>::new()));
                Open::Table
            }
            "thead" => Open::Section(SectionKind::Head),
            "tbody" | "tfoot" => Open::Section(SectionKind::Body),
            "tr" => {
                self.close_to_table_scope(&Open::Row);
                self.row = Some(Vec::new());
                self.row_has_td = false;
                Open::Row
            }
            "td" | "th" => {
                self.close_to_table_scope(&Open::Cell);
                if self.row.is_none() {
                    // Celda sin `<tr>`: se abre una fila implícita.
                    self.row = Some(Vec::new());
                    self.row_has_td = false;
                    self.stack.push(Open::Row);
                }
                if name == "td" {
                    self.row_has_td = true;
                }
                self.start(Tag::TableCell);
                self.block_boundary();
                Open::Cell
            }
            _ => Open::Plain(name.to_owned()),
        };
        self.stack.push(kind);
    }

    /// Texto ya decodificado que se emite tal cual (para el texto alternativo de una imagen).
    fn text_event(&mut self, t: &str) {
        let t = decode_entities(t);
        let t: String = t.split_whitespace().collect::<Vec<_>>().join(" ");
        if !t.is_empty() {
            self.pending_space = false;
            self.at_start = false;
            self.emit(Event::Text(CowStr::from(t)));
        }
    }

    fn close(&mut self, name: &str) {
        let Some(pos) = self.stack.iter().rposition(|o| matches_close(o, name)) else {
            return;
        };
        while self.stack.len() > pos {
            let Some(o) = self.stack.pop() else { break };
            self.finish(o);
        }
    }

    fn close_all(&mut self) {
        while let Some(o) = self.stack.pop() {
            self.finish(o);
        }
        if let Some(code) = self.code.take() {
            self.out.push(Event::Code(CowStr::from(code)));
        }
        // Una fila sin cerrar al final del documento.
        if let Some(row) = self.row.take() {
            self.flush_row(row);
        }
    }

    fn finish(&mut self, o: Open) {
        match o {
            Open::Para => {
                self.end(TagEnd::Paragraph);
                self.block_boundary();
            }
            Open::Plain(n) => {
                if BLOCKS.contains(&n.as_str()) {
                    self.end(TagEnd::Paragraph);
                    self.block_boundary();
                }
            }
            Open::Heading(l) => {
                self.end(TagEnd::Heading(l));
                self.block_boundary();
            }
            Open::Quote => {
                self.end(TagEnd::BlockQuote(None));
                self.block_boundary();
            }
            Open::List(ordered) => {
                self.end(TagEnd::List(ordered));
                self.block_boundary();
            }
            Open::Item => {
                self.end(TagEnd::Item);
                self.block_boundary();
            }
            Open::Pre => {
                self.end(TagEnd::CodeBlock);
                self.pre_first = false;
                self.block_boundary();
            }
            Open::Link => self.end(TagEnd::Link),
            Open::Em => self.end(TagEnd::Emphasis),
            Open::Strong => self.end(TagEnd::Strong),
            Open::Strike => self.end(TagEnd::Strikethrough),
            Open::Code => {
                if let Some(code) = self.code.take() {
                    let code: String = code.split_whitespace().collect::<Vec<_>>().join(" ");
                    if !code.is_empty() {
                        self.emit(Event::Code(CowStr::from(code)));
                        self.at_start = false;
                    }
                }
            }
            Open::Table => {
                if let Some(row) = self.row.take() {
                    self.flush_row(row);
                }
                self.end(TagEnd::Table);
                self.block_boundary();
            }
            Open::Section(_) => {}
            Open::Row => {
                if let Some(row) = self.row.take() {
                    self.flush_row(row);
                }
            }
            Open::Cell => {
                self.end(TagEnd::TableCell);
                self.block_boundary();
            }
        }
    }

    /// Vuelca la fila: cabecera si estaba en `thead` o solo tenía `th`; fila normal en otro caso.
    fn flush_row(&mut self, events: Vec<Event<'static>>) {
        let in_head = self.stack.contains(&Open::Section(SectionKind::Head));
        let header = in_head || !self.row_has_td;
        let (open, close) = if header {
            (Tag::TableHead, TagEnd::TableHead)
        } else {
            (Tag::TableRow, TagEnd::TableRow)
        };
        self.out.push(Event::Start(open));
        self.out.extend(events);
        self.out.push(Event::End(close));
    }

    // ---- cierres implícitos ----

    /// Un `<p>` abierto termina donde empieza otro bloque.
    fn close_paragraph(&mut self) {
        if matches!(self.stack.last(), Some(Open::Para)) {
            self.stack.pop();
            self.finish(Open::Para);
        }
    }

    /// Cierra, hasta `target` (incluido), lo abierto dentro de la misma lista (`li` que cierra al `li` anterior).
    fn close_to_list_scope(&mut self, target: &Open) {
        self.close_implicit(target, |o| matches!(o, Open::List(_) | Open::Table | Open::Quote));
    }

    /// Ídem para filas y celdas dentro de la misma tabla.
    fn close_to_table_scope(&mut self, target: &Open) {
        let target_is_row = matches!(target, Open::Row);
        self.close_implicit(target, move |o| {
            matches!(o, Open::Table | Open::List(_)) || (!target_is_row && matches!(o, Open::Row))
        });
    }

    fn close_implicit(&mut self, target: &Open, barrier: impl Fn(&Open) -> bool) {
        let mut found = None;
        for (i, o) in self.stack.iter().enumerate().rev() {
            if o == target {
                found = Some(i);
                break;
            }
            if barrier(o) {
                break;
            }
            // Un `td` abierto se cierra al llegar otro `td`, pero un `tr` no se cierra por un `td` ajeno.
        }
        if let Some(pos) = found {
            while self.stack.len() > pos {
                let Some(o) = self.stack.pop() else { break };
                self.finish(o);
            }
        }
    }

    /// Cierra lo que no admite bloques dentro (un `p`) antes de una regla horizontal.
    fn end_inline_block(&mut self) {
        self.close_paragraph();
        self.block_boundary();
    }

    fn break_line(&mut self) {
        self.emit(Event::HardBreak);
        self.block_boundary();
    }

    fn block_boundary(&mut self) {
        self.at_start = true;
        self.pending_space = false;
    }
}

fn matches_close(o: &Open, name: &str) -> bool {
    match (o, name) {
        (Open::Para, "p")
        | (Open::Quote, "blockquote")
        | (Open::List(false), "ul")
        | (Open::List(true), "ol")
        | (Open::Item, "li")
        | (Open::Pre, "pre")
        | (Open::Link, "a")
        | (Open::Em, "i" | "em" | "cite" | "dfn" | "var")
        | (Open::Strong, "b" | "strong")
        | (Open::Strike, "s" | "strike" | "del")
        | (Open::Code, "code" | "kbd" | "samp" | "tt")
        | (Open::Table, "table")
        | (Open::Row, "tr")
        | (Open::Cell, "td" | "th")
        | (Open::Section(SectionKind::Head), "thead")
        | (Open::Section(SectionKind::Body), "tbody" | "tfoot") => true,
        (Open::Heading(l), n) => {
            let want = match l {
                HeadingLevel::H1 => "h1",
                HeadingLevel::H2 => "h2",
                HeadingLevel::H3 => "h3",
                HeadingLevel::H4 => "h4",
                HeadingLevel::H5 => "h5",
                HeadingLevel::H6 => "h6",
            };
            n == want
        }
        (Open::Plain(p), n) => p == n,
        _ => false,
    }
}

// ---- análisis léxico ----

struct Markup {
    /// Posición del byte siguiente al `>`.
    end: usize,
    /// Si es la apertura de `script`/`style`: etiqueta cuyo cierre hay que buscar.
    raw_until: Option<&'static str>,
}

/// Si en `start` empieza una etiqueta, comentario o declaración, dónde termina.
fn markup_end(html: &str, start: usize) -> Option<Markup> {
    let rest = &html[start..];
    if let Some(body) = rest.strip_prefix("<!--") {
        let end = body.find("-->").map_or(html.len(), |p| start + 4 + p + 3);
        return Some(Markup { end, raw_until: None });
    }
    let second = rest.as_bytes().get(1).copied()?;
    if !(second.is_ascii_alphabetic() || second == b'/' || second == b'!' || second == b'?') {
        return None;
    }
    if second == b'/' && !rest.as_bytes().get(2).is_some_and(u8::is_ascii_alphabetic) {
        return None;
    }
    // El `>` que cierra no cuenta dentro de comillas.
    let mut quote: Option<u8> = None;
    for (i, &c) in rest.as_bytes().iter().enumerate().skip(1) {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (None, b'"' | b'\'') => quote = Some(c),
            (None, b'>') => {
                let end = start + i + 1;
                let lower = rest[..=i].to_ascii_lowercase();
                let raw_until = if lower.starts_with("<script") && !lower.ends_with("/>") {
                    Some("script")
                } else if lower.starts_with("<style") && !lower.ends_with("/>") {
                    Some("style")
                } else {
                    None
                };
                return Some(Markup { end, raw_until });
            }
            _ => {}
        }
    }
    // Etiqueta sin cerrar: se trata como texto.
    None
}

/// Posición del `</name` que cierra un `script`/`style` (o el final del documento).
fn find_close(html: &str, from: usize, name: &str) -> usize {
    let needle = format!("</{name}");
    let lower = html[from..].to_ascii_lowercase();
    match lower.find(&needle) {
        Some(p) => from + p,
        None => html.len(),
    }
}

/// Valor de un atributo (entre comillas o sin ellas), con entidades decodificadas.
fn attr(attrs: &str, wanted: &str) -> Option<String> {
    let b = attrs.as_bytes();
    let mut i = 0;
    while i < b.len() {
        while i < b.len() && (b[i].is_ascii_whitespace() || b[i] == b'/') {
            i += 1;
        }
        let ns = i;
        while i < b.len() && !b[i].is_ascii_whitespace() && b[i] != b'=' && b[i] != b'/' {
            i += 1;
        }
        let name = &attrs[ns..i];
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        let mut value = String::new();
        if i < b.len() && b[i] == b'=' {
            i += 1;
            while i < b.len() && b[i].is_ascii_whitespace() {
                i += 1;
            }
            if i < b.len() && (b[i] == b'"' || b[i] == b'\'') {
                let q = b[i];
                i += 1;
                let vs = i;
                while i < b.len() && b[i] != q {
                    i += 1;
                }
                value = attrs[vs..i].to_owned();
                i = (i + 1).min(b.len());
            } else {
                let vs = i;
                while i < b.len() && !b[i].is_ascii_whitespace() {
                    i += 1;
                }
                value = attrs[vs..i].to_owned();
            }
        }
        if name.eq_ignore_ascii_case(wanted) {
            return Some(decode_entities(&value));
        }
        if name.is_empty() {
            i += 1;
        }
    }
    None
}

/// Direcciones que se pueden poner en una anotación de enlace: se descartan los destinos internos (`#…`), vacíos y los
/// esquemas que ejecutan código o incrustan datos.
fn safe_href(href: &str) -> Option<String> {
    let h: String = href.chars().filter(|c| !c.is_control()).collect();
    let h = h.trim();
    if h.is_empty() || h.starts_with('#') {
        return None;
    }
    let lower = h.to_ascii_lowercase();
    if ["javascript:", "vbscript:", "data:", "file:"]
        .iter()
        .any(|s| lower.starts_with(s))
    {
        return None;
    }
    Some(h.to_owned())
}

/// Decodifica las entidades de uso común y las numéricas. Una entidad desconocida se deja como está.
fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_owned();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(p) = rest.find('&') {
        out.push_str(&rest[..p]);
        rest = &rest[p..];
        let decoded = rest[1..]
            .find(';')
            .filter(|&e| e <= 10)
            .and_then(|e| entity(&rest[1..1 + e]).map(|c| (c, e + 2)));
        match decoded {
            Some((c, len)) => {
                out.push(c);
                rest = &rest[len..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn entity(name: &str) -> Option<char> {
    if let Some(num) = name.strip_prefix('#') {
        let code = match num.strip_prefix(['x', 'X']) {
            Some(hex) => u32::from_str_radix(hex, 16).ok()?,
            None => num.parse().ok()?,
        };
        return char::from_u32(code).filter(|c| *c != '\0');
    }
    Some(match name {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => '\u{a0}',
        "copy" => '©',
        "reg" => '®',
        "trade" => '™',
        "euro" => '€',
        "pound" => '£',
        "yen" => '¥',
        "cent" => '¢',
        "sect" => '§',
        "para" => '¶',
        "deg" => '°',
        "plusmn" => '±',
        "times" => '×',
        "divide" => '÷',
        "micro" => 'µ',
        "middot" => '·',
        "bull" => '•',
        "hellip" => '…',
        "ndash" => '–',
        "mdash" => '—',
        "lsquo" => '‘',
        "rsquo" => '’',
        "sbquo" => '‚',
        "ldquo" => '“',
        "rdquo" => '”',
        "bdquo" => '„',
        "laquo" => '«',
        "raquo" => '»',
        "iexcl" => '¡',
        "iquest" => '¿',
        "ntilde" => 'ñ',
        "Ntilde" => 'Ñ',
        "aacute" => 'á',
        "eacute" => 'é',
        "iacute" => 'í',
        "oacute" => 'ó',
        "uacute" => 'ú',
        "Aacute" => 'Á',
        "Eacute" => 'É',
        "Iacute" => 'Í',
        "Oacute" => 'Ó',
        "Uacute" => 'Ú',
        "uuml" => 'ü',
        "Uuml" => 'Ü',
        "auml" => 'ä',
        "ouml" => 'ö',
        "Auml" => 'Ä',
        "Ouml" => 'Ö',
        "szlig" => 'ß',
        "agrave" => 'à',
        "egrave" => 'è',
        "igrave" => 'ì',
        "ograve" => 'ò',
        "ugrave" => 'ù',
        "acirc" => 'â',
        "ecirc" => 'ê',
        "icirc" => 'î',
        "ocirc" => 'ô',
        "ucirc" => 'û',
        "ccedil" => 'ç',
        "Ccedil" => 'Ç',
        "aring" => 'å',
        "oslash" => 'ø',
        "aelig" => 'æ',
        "frac12" => '½',
        "frac14" => '¼',
        "frac34" => '¾',
        "larr" => '←',
        "rarr" => '→',
        _ => return None,
    })
}
