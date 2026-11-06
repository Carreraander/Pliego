//! Mover y cortar contenido de una página por zonas (W43, edición con recolocación tipo Acrobat).
//!
//! Se usa para que, al crecer o encogerse un párrafo editado, **lo que hay debajo suba o baje con su aspecto
//! intacto** (fuentes, colores, kerning), y para llevar a la página siguiente lo que ya no cabe. No recompone nada:
//! reescribe el flujo de contenido añadiendo traslaciones (`cm`).
//!
//! # Cómo
//!
//! Un recorrido del flujo (con el mismo seguimiento de estado gráfico y de texto que la redacción) clasifica cada
//! *elemento*: un **tramo de texto** (los `Tj`/`TJ` entre dos operaciones de posición), un camino pintado, una
//! imagen, un formulario o una imagen en línea. Un segundo recorrido emite dos salidas:
//!
//! - **A** (la página): todo salvo lo cortado; lo movido va con su traslación. Fuera de `BT…ET` se envuelve en
//!   `q … cm … Q`; dentro del texto se cierra y se reabre el objeto de texto (`ET cm BT Tm`) en la frontera entre
//!   tramos, que es donde la matriz de línea y la de texto coinciden.
//! - **B** (lo cortado): las mismas operaciones de estado y solo los elementos cortados; se empaqueta como
//!   formulario XObject para pegarlo en otra página.
//!
//! # Reglas
//!
//! - Un tramo de texto se asigna a una zona si **el centro de su caja** cae dentro; imágenes, formularios y caminos,
//!   si su caja queda **entera** dentro (así un fondo o una línea de tabla que sale de la zona no se mueve).
//! - Los recortes (`W`) no se mueven nunca y van a las dos salidas.
//! - Lo que no se sabe medir (fuente opaca, posición incierta) **no se mueve**: se cuenta en el informe.
//! - Los formularios XObject se mueven enteros (no se recorre su contenido).

use std::collections::HashMap;
use std::rc::Rc;

use lopdf::{Dictionary, Object as L, ObjectId, Stream};
use pliego_core::Result;

use super::geom::{M, R};
use super::lex::{Instr, Operand, fmt_num, parse, write_name, write_string};
use super::metrics::FontInfo;
use super::{Gs, Res, finish, form_bbox, form_matrix, inherited_resources, nums, to_m};
use crate::geom::{Matrix, Rect};
use crate::native::object::decode_stream;
use crate::native::{Object, PdfDocument, State, pdf_err};

/// Qué se hace con un elemento.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Dest {
    Stay,
    /// Traslación en espacio de usuario.
    Move(f64, f64),
    Cut,
}

#[derive(Debug, Clone, Default)]
pub(super) struct ItemInfo {
    pub(super) bbox: Option<R>,
    pub(super) opaque: bool,
    pub(super) text: bool,
}

struct Pend {
    raw: Vec<u8>,
    bbox: Option<R>,
    clip: bool,
    lw: f64,
}

enum Show {
    S(Vec<u8>),
    Adj(f64),
}

struct Walker<'a> {
    st: &'a State,
    fonts: HashMap<ObjectId, Rc<FontInfo>>,
    res: &'a Res,
    decisions: Option<&'a [Dest]>,
    items: Vec<ItemInfo>,
    a: Vec<Vec<u8>>,
    b: Vec<Vec<u8>>,
    gs: Gs,
    stack: Vec<(Gs, (f64, f64))>,
    /// Traslación vigente en la salida A (solo distinta de cero dentro de un objeto de texto).
    shift: (f64, f64),
    tm: M,
    tlm: M,
    uncertain: bool,
    in_text: bool,
    seg: Option<usize>,
    seg_dest: Dest,
    pending: Option<Pend>,
}

fn chunk(bytes: &[u8]) -> Vec<u8> {
    let mut v = bytes.to_vec();
    v.push(b'\n');
    v
}

impl<'a> Walker<'a> {
    fn new(st: &'a State, res: &'a Res, decisions: Option<&'a [Dest]>) -> Self {
        Walker {
            st,
            fonts: HashMap::new(),
            res,
            decisions,
            items: Vec::new(),
            a: Vec::new(),
            b: Vec::new(),
            gs: Gs {
                ctm: M::IDENT,
                font: None,
                size: 0.0,
                tc: 0.0,
                tw: 0.0,
                th: 1.0,
                tl: 0.0,
                rise: 0.0,
                lw: 1.0,
            },
            stack: Vec::new(),
            shift: (0.0, 0.0),
            tm: M::IDENT,
            tlm: M::IDENT,
            uncertain: false,
            in_text: false,
            seg: None,
            seg_dest: Dest::Stay,
            pending: None,
        }
    }

    fn both(&mut self, raw: &[u8]) {
        self.a.push(chunk(raw));
        self.b.push(chunk(raw));
    }

    fn only_a(&mut self, raw: &[u8]) {
        self.a.push(chunk(raw));
    }

    fn font_for(&mut self, name: &[u8]) -> Option<Rc<FontInfo>> {
        let entry = self.res.fonts.get(name).ok()?.clone();
        match entry {
            L::Reference(id) => {
                let id = self.st.resolve_id(id);
                if let Some(f) = self.fonts.get(&id) {
                    return Some(f.clone());
                }
                let info = Rc::new(
                    self.st
                        .get(id)
                        .map_or_else(FontInfo::opaque, |o| FontInfo::load(self.st, o)),
                );
                self.fonts.insert(id, info.clone());
                Some(info)
            }
            other => Some(Rc::new(FontInfo::load(self.st, &other))),
        }
    }

    fn new_item(&mut self, text: bool) -> usize {
        self.items.push(ItemInfo {
            bbox: None,
            opaque: false,
            text,
        });
        self.items.len() - 1
    }

    fn dest(&self, idx: usize) -> Dest {
        self.decisions.and_then(|d| d.get(idx)).copied().unwrap_or(Dest::Stay)
    }

    fn union_into(&mut self, idx: usize, b: R) {
        let it = &mut self.items[idx];
        it.bbox = Some(match it.bbox {
            Some(o) => R {
                x0: o.x0.min(b.x0),
                y0: o.y0.min(b.y0),
                x1: o.x1.max(b.x1),
                y1: o.y1.max(b.y1),
            },
            None => b,
        });
    }

    /// `cm` (en el espacio local actual) que traslada `delta` unidades del espacio de usuario de la página.
    fn cm_for(&self, delta: (f64, f64)) -> String {
        let (lx, ly) = match self.gs.ctm.invert() {
            Some(i) => (i.a * delta.0 + i.c * delta.1, i.b * delta.0 + i.d * delta.1),
            None => (0.0, 0.0),
        };
        format!("1 0 0 1 {} {} cm", fmt_num(lx), fmt_num(ly))
    }

    /// Traslación que quiere la salida A para el tramo actual (`None` si se corta: A no lo dibuja).
    fn want(&self) -> Option<(f64, f64)> {
        match self.seg_dest {
            Dest::Stay => Some((0.0, 0.0)),
            Dest::Move(dx, dy) => Some((dx, dy)),
            Dest::Cut => None,
        }
    }

    fn begin_segment(&mut self) {
        let idx = self.new_item(true);
        self.seg = Some(idx);
        self.seg_dest = self.dest(idx);
    }

    /// Tras una posición dentro del texto: cierra y reabre el objeto de texto con la traslación nueva.
    fn text_transition(&mut self) {
        let Some(w) = self.want() else { return };
        if w == self.shift {
            return;
        }
        let delta = (w.0 - self.shift.0, w.1 - self.shift.1);
        self.only_a(b"ET");
        let cm = self.cm_for(delta);
        self.only_a(cm.as_bytes());
        self.only_a(b"BT");
        let t = self.tlm;
        let tm = format!(
            "{} {} {} {} {} {} Tm",
            fmt_num(t.a),
            fmt_num(t.b),
            fmt_num(t.c),
            fmt_num(t.d),
            fmt_num(t.e),
            fmt_num(t.f)
        );
        self.only_a(tm.as_bytes());
        self.shift = w;
    }

    fn end_text(&mut self) {
        if self.shift != (0.0, 0.0) {
            let cm = self.cm_for((-self.shift.0, -self.shift.1));
            self.only_a(cm.as_bytes());
            self.shift = (0.0, 0.0);
        }
        self.in_text = false;
    }

    fn flush_pending(&mut self) {
        if let Some(p) = self.pending.take() {
            self.a.push(p.raw);
        }
    }

    fn walk(&mut self, content: &[u8]) {
        let instrs = parse(content);
        for ins in &instrs {
            let raw = &content[ins.start..ins.end];
            let op = ins.op.as_slice();
            let is_path = matches!(op, b"m" | b"l" | b"c" | b"v" | b"y" | b"h" | b"re" | b"W" | b"W*");
            let is_paint = matches!(
                op,
                b"S" | b"s" | b"f" | b"F" | b"f*" | b"B" | b"B*" | b"b" | b"b*" | b"n"
            );
            if self.pending.is_some() && !is_path && !is_paint {
                self.flush_pending();
            }
            match op {
                b"q" => {
                    self.stack.push((self.gs.clone(), self.shift));
                    self.both(raw);
                }
                b"Q" => {
                    if let Some((g, s)) = self.stack.pop() {
                        self.gs = g;
                        self.shift = s;
                    }
                    self.both(raw);
                }
                b"cm" => {
                    if let Some(m) = to_m(&ins.operands) {
                        self.gs.ctm = m.then(self.gs.ctm);
                    }
                    self.both(raw);
                }
                b"w" => {
                    if let Some(w) = ins.operands.first().and_then(Operand::num) {
                        self.gs.lw = w;
                    }
                    self.both(raw);
                }
                b"BT" => {
                    self.tm = M::IDENT;
                    self.tlm = M::IDENT;
                    self.uncertain = false;
                    self.begin_segment();
                    if let Some(w) = self.want()
                        && w != self.shift
                    {
                        let cm = self.cm_for((w.0 - self.shift.0, w.1 - self.shift.1));
                        self.only_a(cm.as_bytes());
                        self.shift = w;
                    }
                    self.in_text = true;
                    self.both(raw);
                }
                b"ET" => {
                    self.both(raw);
                    self.end_text();
                }
                b"Tc" | b"Tw" | b"Tz" | b"TL" | b"Ts" | b"Tr" => {
                    if let Some(v) = ins.operands.first().and_then(Operand::num) {
                        match op {
                            b"Tc" => self.gs.tc = v,
                            b"Tw" => self.gs.tw = v,
                            b"Tz" => self.gs.th = v / 100.0,
                            b"TL" => self.gs.tl = v,
                            b"Ts" => self.gs.rise = v,
                            _ => {}
                        }
                    }
                    self.both(raw);
                }
                b"Tf" => {
                    if let (Some(Operand::Name(n)), Some(sz)) =
                        (ins.operands.first(), ins.operands.get(1).and_then(Operand::num))
                    {
                        self.gs.font = self.font_for(n);
                        self.gs.size = sz;
                    }
                    self.both(raw);
                }
                b"Td" | b"TD" | b"T*" | b"Tm" => {
                    self.position(ins);
                    self.both(raw);
                    self.after_position();
                }
                b"'" | b"\"" => self.quote(ins),
                b"Tj" | b"TJ" => {
                    let items = match (op, ins.operands.as_slice()) {
                        (b"Tj", [Operand::Str(s)]) => vec![Show::S(s.clone())],
                        (b"TJ", [Operand::Array(a)]) => a
                            .iter()
                            .filter_map(|o| match o {
                                Operand::Str(s) => Some(Show::S(s.clone())),
                                Operand::Num(n) => Some(Show::Adj(*n)),
                                _ => None,
                            })
                            .collect(),
                        _ => Vec::new(),
                    };
                    self.measure(&items);
                    self.emit_show(raw);
                }
                b"m" | b"l" | b"c" | b"v" | b"y" | b"re" | b"h" => self.path_op(ins, raw),
                b"W" | b"W*" => {
                    let lw = self.gs.lw;
                    let p = self.pending.get_or_insert(Pend {
                        raw: Vec::new(),
                        bbox: None,
                        clip: false,
                        lw,
                    });
                    p.clip = true;
                    p.raw.extend_from_slice(raw);
                    p.raw.push(b'\n');
                }
                b"S" | b"s" | b"f" | b"F" | b"f*" | b"B" | b"B*" | b"b" | b"b*" | b"n" => self.paint(op, raw),
                b"Do" => self.do_xobject(ins, raw),
                b"BI" => {
                    let b = R::new(0.0, 0.0, 1.0, 1.0).transform(&self.gs.ctm);
                    self.drawable(raw, Some(b));
                }
                b"sh" => self.only_a(raw),
                _ => self.both(raw),
            }
        }
        self.flush_pending();
        if self.in_text {
            self.both(b"ET");
            self.end_text();
        }
    }

    /// Actualiza las matrices de texto con una operación de posición.
    fn position(&mut self, ins: &Instr) {
        match ins.op.as_slice() {
            b"Td" | b"TD" => {
                if let (Some(tx), Some(ty)) = (
                    ins.operands.first().and_then(Operand::num),
                    ins.operands.get(1).and_then(Operand::num),
                ) {
                    if ins.op == b"TD" {
                        self.gs.tl = -ty;
                    }
                    self.tlm = M::translate(tx, ty).then(self.tlm);
                    self.tm = self.tlm;
                    self.uncertain = false;
                }
            }
            b"T*" => {
                self.tlm = M::translate(0.0, -self.gs.tl).then(self.tlm);
                self.tm = self.tlm;
                self.uncertain = false;
            }
            b"Tm" => {
                if let Some(m) = to_m(&ins.operands) {
                    self.tm = m;
                    self.tlm = m;
                    self.uncertain = false;
                }
            }
            _ => {}
        }
    }

    fn after_position(&mut self) {
        if self.in_text {
            self.begin_segment();
            self.text_transition();
        }
    }

    /// `'` y `"` = posición (`T*`) + mostrar: se normalizan a `T* (…) Tj` para repartir cada parte.
    fn quote(&mut self, ins: &Instr) {
        let op = ins.op.as_slice();
        let (prefix, s) = match (op, ins.operands.as_slice()) {
            (b"'", [Operand::Str(s)]) => ("T*".to_owned(), s.clone()),
            (b"\"", [Operand::Num(aw), Operand::Num(ac), Operand::Str(s)]) => {
                self.gs.tw = *aw;
                self.gs.tc = *ac;
                (format!("{} Tw {} Tc T*", fmt_num(*aw), fmt_num(*ac)), s.clone())
            }
            _ => return,
        };
        self.tlm = M::translate(0.0, -self.gs.tl).then(self.tlm);
        self.tm = self.tlm;
        self.uncertain = false;
        self.both(prefix.as_bytes());
        self.after_position();
        self.measure(&[Show::S(s.clone())]);
        let mut b = Vec::new();
        write_string(&mut b, &s);
        b.extend_from_slice(b" Tj");
        self.emit_show(&b);
    }

    fn emit_show(&mut self, raw: &[u8]) {
        match (self.in_text, self.seg_dest) {
            (true, Dest::Cut) => self.b.push(chunk(raw)),
            _ => self.only_a(raw),
        }
    }

    /// Caja del texto mostrado, acumulada en el tramo actual; avanza la matriz de texto.
    fn measure(&mut self, items: &[Show]) {
        let Some(seg) = self.seg.filter(|_| self.in_text) else {
            return;
        };
        let (size, th) = (self.gs.size, self.gs.th);
        let font = self.gs.font.clone();
        let (asc, desc) = font.as_ref().map_or((0.9, -0.25), |f| (f.ascent, f.descent));
        for item in items {
            match item {
                Show::Adj(n) => {
                    self.tm = M::translate(-n / 1000.0 * size * th, 0.0).then(self.tm);
                }
                Show::S(s) => {
                    let glyphs = font
                        .as_ref()
                        .and_then(|f| f.decode(s))
                        .filter(|g| g.iter().all(|x| x.width.is_some()))
                        .filter(|_| !self.uncertain);
                    match glyphs {
                        Some(glyphs) => {
                            for g in glyphs {
                                let w0 = g.width.unwrap_or(0.0);
                                let adv = (w0 * size + self.gs.tc + if g.space { self.gs.tw } else { 0.0 }) * th;
                                let m = self.tm.then(self.gs.ctm);
                                let wx = w0 * size * th;
                                let (x0, x1) = if wx >= 0.0 {
                                    (wx * 0.03, wx * 0.97)
                                } else {
                                    (wx * 0.97, wx * 0.03)
                                };
                                let y0 = self.gs.rise + desc * size;
                                let y1 = self.gs.rise + asc * size;
                                self.union_into(seg, R::new(x0, y0, x1, y1).transform(&m));
                                self.tm = M::translate(adv, 0.0).then(self.tm);
                            }
                        }
                        None => {
                            // Se desconoce el avance: la caja es solo la del origen, y el tramo no se mueve.
                            let m = self.tm.then(self.gs.ctm);
                            let y0 = self.gs.rise + (desc - 0.25) * size;
                            let y1 = self.gs.rise + (asc + 0.25) * size;
                            self.union_into(seg, R::new(0.0, y0, size.abs(), y1).transform(&m));
                            self.items[seg].opaque = true;
                            self.uncertain = true;
                        }
                    }
                }
            }
        }
    }

    fn path_op(&mut self, ins: &Instr, raw: &[u8]) {
        let lw = self.gs.lw;
        let ctm = self.gs.ctm;
        let p = self.pending.get_or_insert(Pend {
            raw: Vec::new(),
            bbox: None,
            clip: false,
            lw,
        });
        p.raw.extend_from_slice(raw);
        p.raw.push(b'\n');
        let v = nums(&ins.operands);
        let mut pts: Vec<(f64, f64)> = Vec::new();
        match ins.op.as_slice() {
            b"m" | b"l" if v.len() >= 2 => pts.push(ctm.apply(v[0], v[1])),
            b"c" if v.len() >= 6 => {
                for k in 0..3 {
                    pts.push(ctm.apply(v[2 * k], v[2 * k + 1]));
                }
            }
            b"v" | b"y" if v.len() >= 4 => {
                for k in 0..2 {
                    pts.push(ctm.apply(v[2 * k], v[2 * k + 1]));
                }
            }
            b"re" if v.len() >= 4 => {
                pts.push(ctm.apply(v[0], v[1]));
                pts.push(ctm.apply(v[0] + v[2], v[1]));
                pts.push(ctm.apply(v[0], v[1] + v[3]));
                pts.push(ctm.apply(v[0] + v[2], v[1] + v[3]));
            }
            _ => {}
        }
        if !pts.is_empty() {
            let b = R::of_points(&pts);
            p.bbox = Some(match p.bbox {
                Some(old) => R {
                    x0: old.x0.min(b.x0),
                    y0: old.y0.min(b.y0),
                    x1: old.x1.max(b.x1),
                    y1: old.y1.max(b.y1),
                },
                None => b,
            });
        }
    }

    fn paint(&mut self, op: &[u8], raw: &[u8]) {
        let Some(p) = self.pending.take() else {
            self.only_a(raw);
            return;
        };
        if p.clip {
            // Los recortes se conservan en las dos salidas y no se pintan en B.
            self.a.push(p.raw.clone());
            self.a.push(chunk(raw));
            self.b.push(p.raw);
            self.b.push(b"n\n".to_vec());
            return;
        }
        let stroked = matches!(op, b"S" | b"s" | b"B" | b"B*" | b"b" | b"b*");
        let bbox = p.bbox.map(|b| {
            if stroked {
                b.inflate(p.lw.abs() * self.gs.ctm.scale() / 2.0)
            } else {
                b
            }
        });
        let mut whole = p.raw;
        whole.extend_from_slice(raw);
        self.drawable(&whole, bbox);
    }

    fn do_xobject(&mut self, ins: &Instr, raw: &[u8]) {
        let Some(Operand::Name(name)) = ins.operands.first() else {
            self.only_a(raw);
            return;
        };
        let id = match self.res.xobjects.get(name) {
            Ok(L::Reference(id)) => self.st.resolve_id(*id),
            _ => {
                self.drawable(raw, None);
                return;
            }
        };
        let bbox = match self.st.doc.objects.get(&id) {
            Some(L::Stream(s)) => match s.dict.get(b"Subtype") {
                Ok(L::Name(n)) if n == b"Image" => Some(R::new(0.0, 0.0, 1.0, 1.0).transform(&self.gs.ctm)),
                Ok(L::Name(n)) if n == b"Form" => {
                    let fm = form_matrix(self.st, s).then(self.gs.ctm);
                    form_bbox(self.st, s).map(|b| b.transform(&fm))
                }
                _ => None,
            },
            _ => None,
        };
        self.drawable(raw, bbox);
    }

    /// Elemento dibujable (camino, imagen, formulario): se reparte entre A y B según la decisión.
    /// `raw` ya incluye todas sus operaciones; fuera del texto se envuelve en `q cm … Q` si se mueve.
    fn drawable(&mut self, raw: &[u8], bbox: Option<R>) {
        let idx = self.new_item(false);
        self.items[idx].bbox = bbox;
        match self.dest(idx) {
            Dest::Stay => self.a.push(chunk(raw)),
            Dest::Cut => self.b.push(chunk(raw)),
            Dest::Move(dx, dy) => {
                let cm = self.cm_for((dx, dy));
                let mut v = b"q\n".to_vec();
                v.extend_from_slice(cm.as_bytes());
                v.push(b'\n');
                v.extend_from_slice(raw);
                v.extend_from_slice(b"\nQ\n");
                self.a.push(v);
            }
        }
    }
}

/// Qué ocurrió.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MoveReport {
    /// Tramos de texto (líneas o partes de línea) movidos.
    pub text_runs_moved: usize,
    /// Imágenes, formularios y caminos movidos.
    pub objects_moved: usize,
    /// Tramos de texto que cayeron en una zona pero no se pudieron medir (no se movieron).
    pub opaque_skipped: usize,
    /// Enlaces (anotaciones) que acompañaron al contenido.
    pub links_moved: usize,
}

/// Contenido cortado de una página, listo para pegar en otra con [`PdfDocument::paste_content`].
#[derive(Debug, Clone)]
pub struct CutContent {
    form: ObjectId,
    /// Matriz de usuario a página de la página de origen.
    src_ctm: [f64; 6],
    /// Caja del contenido cortado en el espacio de página de origen.
    pub bounds: Rect,
    pub report: MoveReport,
    /// Enlaces que estaban sobre lo cortado: su diccionario y su caja en espacio de usuario de la página de origen.
    links: Vec<(Dictionary, R)>,
}

/// Zona con su acción, en espacio de usuario.
pub(super) type Region = (R, Dest);

fn decide(items: &[ItemInfo], regions: &[Region], report: &mut MoveReport) -> Vec<Dest> {
    items
        .iter()
        .map(|it| {
            let Some(b) = it.bbox else { return Dest::Stay };
            let hit = regions.iter().find(|(r, _)| {
                if it.text {
                    r.contains((b.x0 + b.x1) / 2.0, (b.y0 + b.y1) / 2.0)
                } else {
                    b.inside(&r.inflate(0.5))
                }
            });
            match hit {
                Some(_) if it.opaque => {
                    report.opaque_skipped += 1;
                    Dest::Stay
                }
                Some((_, d)) => *d,
                None => Dest::Stay,
            }
        })
        .collect()
}

struct Page {
    id: ObjectId,
    content: Vec<u8>,
    res: Res,
}

fn load_page(st: &State, id: ObjectId) -> Result<Page> {
    let dict = st
        .dict_of(st.get(id).ok_or_else(|| pdf_err("la página no existe"))?)
        .ok_or_else(|| pdf_err("la página no es un diccionario"))?
        .clone();
    let parts: Vec<L> = match dict.get(b"Contents").ok().and_then(|o| st.deref(o)) {
        Some(L::Array(a)) => a.clone(),
        Some(s @ L::Stream(_)) => vec![s.clone()],
        _ => Vec::new(),
    };
    let mut content = Vec::new();
    for p in &parts {
        if let Some(L::Stream(s)) = st.deref(p) {
            content.extend_from_slice(&decode_stream(s)?);
            content.push(b'\n');
        }
    }
    let res_dict = dict
        .get(b"Resources")
        .ok()
        .and_then(|o| st.dict_of(o))
        .cloned()
        .or_else(|| inherited_resources(st, &dict));
    let res = Res::new(st, res_dict.as_ref());
    Ok(Page { id, content, res })
}

fn finish_stream(items: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = b"q\n".to_vec();
    for c in items {
        bytes.extend_from_slice(c);
    }
    bytes
}

/// Resultado de repartir una página.
struct Split {
    report: MoveReport,
    changed: bool,
    cut: Option<Cut>,
}

/// Reparte el contenido de la página entre A (queda) y B (cortado).
fn split_page(st: &mut State, page_id: ObjectId, regions: &[Region]) -> Result<Split> {
    let page = load_page(st, page_id)?;
    let mut report = MoveReport::default();
    let (items, decisions) = {
        let mut w1 = Walker::new(st, &page.res, None);
        w1.walk(&page.content);
        let decisions = decide(&w1.items, regions, &mut report);
        (w1.items, decisions)
    };
    if decisions.iter().all(|d| *d == Dest::Stay) {
        return Ok(Split {
            report,
            changed: false,
            cut: None,
        });
    }
    let (a, b, close) = {
        let mut w2 = Walker::new(st, &page.res, Some(&decisions));
        w2.walk(&page.content);
        // Cierra las `q` que el flujo dejara sin cerrar.
        let close: Vec<u8> = b"Q\n".repeat(w2.stack.len());
        (w2.a, w2.b, close)
    };
    for (it, d) in items.iter().zip(&decisions) {
        if matches!(d, Dest::Move(..)) {
            if it.text {
                report.text_runs_moved += 1;
            } else {
                report.objects_moved += 1;
            }
        }
    }

    let mut a_bytes = finish_stream(&a);
    a_bytes.extend_from_slice(&close);
    a_bytes.extend_from_slice(b"Q\n");
    let mut s = Stream::new(Dictionary::new(), a_bytes);
    s.allows_compression = true;
    let _ = s.compress();
    let sid = st.add(L::Stream(s));
    crate::native::document::set_dict_entry(st, page.id, "Contents", L::Reference(sid))?;

    let cut = if decisions.contains(&Dest::Cut) {
        let mut bbox: Option<R> = None;
        for (it, d) in items.iter().zip(&decisions) {
            if *d == Dest::Cut
                && let Some(b) = it.bbox
            {
                bbox = Some(match bbox {
                    Some(o) => R {
                        x0: o.x0.min(b.x0),
                        y0: o.y0.min(b.y0),
                        x1: o.x1.max(b.x1),
                        y1: o.y1.max(b.y1),
                    },
                    None => b,
                });
            }
        }
        let bbox = bbox.unwrap_or(R::new(0.0, 0.0, 1.0, 1.0)).inflate(2.0);
        let mut b_bytes = finish_stream(&b);
        b_bytes.extend_from_slice(&close);
        b_bytes.extend_from_slice(b"Q\n");
        let mut dict = Dictionary::new();
        dict.set("Type", L::Name(b"XObject".to_vec()));
        dict.set("Subtype", L::Name(b"Form".to_vec()));
        dict.set(
            "BBox",
            L::Array(vec![
                L::Real(bbox.x0 as f32),
                L::Real(bbox.y0 as f32),
                L::Real(bbox.x1 as f32),
                L::Real(bbox.y1 as f32),
            ]),
        );
        dict.set("Resources", L::Dictionary(page.res.into_dict()));
        let mut fs = Stream::new(dict, b_bytes);
        fs.allows_compression = true;
        let _ = fs.compress();
        let fid = st.add(L::Stream(fs));
        Some(Cut { form: fid, bbox })
    } else {
        None
    };
    if let Some(d) = crate::native::document::dict_of_mut(st, page_id) {
        d.remove(b"Thumb");
        d.remove(b"PieceInfo");
    }
    st.touch(page_id);
    Ok(Split {
        report,
        changed: true,
        cut,
    })
}

struct Cut {
    form: ObjectId,
    bbox: R,
}

// --- Enlaces (anotaciones /Link) que acompañan al contenido ---

fn rect_of_dict(d: &Dictionary) -> Option<R> {
    let a = d.get(b"Rect").ok()?.as_array().ok()?;
    let v: Vec<f64> = a
        .iter()
        .filter_map(|o| match o {
            L::Integer(i) => Some(*i as f64),
            L::Real(r) => Some(f64::from(*r)),
            _ => None,
        })
        .collect();
    if v.len() != 4 {
        return None;
    }
    Some(R::new(v[0].min(v[2]), v[1].min(v[3]), v[0].max(v[2]), v[1].max(v[3])))
}

fn rect_obj(r: &R) -> L {
    L::Array(vec![
        L::Real(r.x0 as f32),
        L::Real(r.y0 as f32),
        L::Real(r.x1 as f32),
        L::Real(r.y1 as f32),
    ])
}

/// Enlaces de la página: (id del objeto, caja en espacio de usuario). Solo los que son objetos indirectos.
fn page_links(st: &State, page_id: ObjectId) -> Vec<(ObjectId, R)> {
    let Some(page) = st.get(page_id).and_then(|o| st.dict_of(o)) else {
        return Vec::new();
    };
    let Some(L::Array(annots)) = page.get(b"Annots").ok().and_then(|o| st.deref(o)) else {
        return Vec::new();
    };
    annots
        .iter()
        .filter_map(|o| {
            let L::Reference(id) = o else { return None };
            let d = st.dict_of(st.get(*id)?)?;
            if d.get(b"Subtype").ok().and_then(|o| o.as_name().ok()) != Some(b"Link".as_slice()) {
                return None;
            }
            Some((*id, rect_of_dict(d)?))
        })
        .collect()
}

fn link_hit(r: &R, regions: &[Region]) -> Option<Dest> {
    let (cx, cy) = ((r.x0 + r.x1) / 2.0, (r.y0 + r.y1) / 2.0);
    regions.iter().find(|(z, _)| z.contains(cx, cy)).map(|(_, d)| *d)
}

/// Traslada los enlaces cuyo centro cae en una zona de movimiento. Devuelve cuántos.
fn move_links(st: &mut State, page_id: ObjectId, regions: &[Region]) -> usize {
    let mut n = 0;
    for (id, r) in page_links(st, page_id) {
        let Some(Dest::Move(dx, dy)) = link_hit(&r, regions) else {
            continue;
        };
        let moved = R::new(r.x0 + dx, r.y0 + dy, r.x1 + dx, r.y1 + dy);
        if let Some(d) = crate::native::document::dict_of_mut(st, id) {
            d.set("Rect", rect_obj(&moved));
            // Los cuadriláteros (si los hay) se desplazan igual.
            if let Ok(L::Array(q)) = d.get_mut(b"QuadPoints") {
                for (i, o) in q.iter_mut().enumerate() {
                    let v = match o {
                        L::Integer(v) => *v as f64,
                        L::Real(v) => f64::from(*v),
                        _ => continue,
                    };
                    *o = L::Real((v + if i % 2 == 0 { dx } else { dy }) as f32);
                }
            }
            st.touch(id);
            n += 1;
        }
    }
    n
}

/// Quita de la página los enlaces cuyo centro cae en `zone` y devuelve sus diccionarios con su caja.
fn take_links(st: &mut State, page_id: ObjectId, zone: &R) -> Vec<(Dictionary, R)> {
    let hits: Vec<(ObjectId, R)> = page_links(st, page_id)
        .into_iter()
        .filter(|(_, r)| zone.contains((r.x0 + r.x1) / 2.0, (r.y0 + r.y1) / 2.0))
        .collect();
    if hits.is_empty() {
        return Vec::new();
    }
    let out: Vec<(Dictionary, R)> = hits
        .iter()
        .filter_map(|(id, r)| Some((st.dict_of(st.get(*id)?)?.clone(), *r)))
        .collect();
    let gone: Vec<ObjectId> = hits.iter().map(|(id, _)| *id).collect();
    // /Annots puede ser un array directo o una referencia a uno.
    let holder = match st
        .get(page_id)
        .and_then(|o| st.dict_of(o))
        .and_then(|d| d.get(b"Annots").ok())
    {
        Some(L::Reference(a)) => Some(*a),
        _ => None,
    };
    let keep = |o: &L| !matches!(o, L::Reference(id) if gone.contains(id));
    match holder {
        Some(a) => {
            if let Some(L::Array(v)) = st.doc.objects.get_mut(&a) {
                v.retain(keep);
            }
            st.touch(a);
        }
        None => {
            if let Some(L::Array(v)) =
                crate::native::document::dict_of_mut(st, page_id).and_then(|d| d.get_mut(b"Annots").ok())
            {
                v.retain(keep);
            }
            st.touch(page_id);
        }
    }
    out
}

/// Añade a la página los enlaces cortados, con la caja ya en su espacio de usuario.
fn add_links(st: &mut State, page_id: ObjectId, links: Vec<(Dictionary, R)>) {
    if links.is_empty() {
        return;
    }
    let mut ids = Vec::new();
    for (mut d, r) in links {
        d.set("Rect", rect_obj(&r));
        d.remove(b"QuadPoints");
        d.set("P", L::Reference(page_id));
        ids.push(st.add(L::Dictionary(d)));
    }
    let holder = match st
        .get(page_id)
        .and_then(|o| st.dict_of(o))
        .and_then(|d| d.get(b"Annots").ok())
    {
        Some(L::Reference(a)) => Some(*a),
        _ => None,
    };
    let refs = ids.into_iter().map(L::Reference);
    match holder {
        Some(a) => {
            if let Some(L::Array(v)) = st.doc.objects.get_mut(&a) {
                v.extend(refs);
            }
            st.touch(a);
        }
        None => {
            if let Some(d) = crate::native::document::dict_of_mut(st, page_id) {
                match d.get_mut(b"Annots") {
                    Ok(L::Array(v)) => v.extend(refs),
                    _ => d.set("Annots", L::Array(refs.collect())),
                }
            }
            st.touch(page_id);
        }
    }
}

fn page_index(doc: &PdfDocument, page: i32) -> Result<ObjectId> {
    let ids = doc.page_ids();
    usize::try_from(page)
        .ok()
        .and_then(|i| ids.get(i).copied())
        .ok_or_else(|| pdf_err(format!("la página {} no existe", page + 1)))
}

fn m_of(m: &Matrix) -> M {
    M {
        a: f64::from(m.a),
        b: f64::from(m.b),
        c: f64::from(m.c),
        d: f64::from(m.d),
        e: f64::from(m.e),
        f: f64::from(m.f),
    }
}

/// Traslación `(dx, dy)` del espacio de página al de usuario (solo la parte lineal de la inversa).
fn vec_to_user(inv: &M, dx: f64, dy: f64) -> (f64, f64) {
    (inv.a * dx + inv.c * dy, inv.b * dx + inv.d * dy)
}

fn rect_to_user(inv: &M, r: Rect) -> R {
    let u = R::new(f64::from(r.x0), f64::from(r.y0), f64::from(r.x1), f64::from(r.y1)).transform(inv);
    R::new(u.x0, u.y0, u.x1, u.y1)
}

/// Desplazamiento de una zona: `area` y `(dx, dy)` están en espacio de página (origen arriba a la izquierda, con
/// giro aplicado; `dy > 0` baja).
#[derive(Debug, Clone, Copy)]
pub struct ContentMove {
    pub area: Rect,
    pub dx: f32,
    pub dy: f32,
}

impl PdfDocument {
    /// Traslada, sin recomponer nada, el contenido (texto, imágenes, formularios y caminos) cuya caja cae en cada
    /// zona. Véase la documentación del módulo para las reglas de asignación. Devuelve lo que se movió.
    pub fn move_content(&mut self, page: i32, moves: &[ContentMove]) -> Result<MoveReport> {
        let id = page_index(self, page)?;
        let inv = self
            .load_pdf_page(page)?
            .ctm()?
            .invert()
            .ok_or_else(|| pdf_err("la matriz de la página no es invertible"))?;
        let inv = m_of(&inv);
        let regions: Vec<Region> = moves
            .iter()
            .filter(|m| m.dx != 0.0 || m.dy != 0.0)
            .map(|m| {
                let (ux, uy) = vec_to_user(&inv, f64::from(m.dx), f64::from(m.dy));
                (rect_to_user(&inv, m.area), Dest::Move(ux, uy))
            })
            .collect();
        if regions.is_empty() {
            return Ok(MoveReport::default());
        }
        let mut st = self.st.borrow_mut();
        let mut out = split_page(&mut st, id, &regions)?;
        // Los enlaces sobre lo movido se van con ello (aunque la página no tuviera contenido que mover).
        out.report.links_moved = move_links(&mut st, id, &regions);
        if out.changed || out.report.links_moved > 0 {
            finish(&mut st);
        }
        Ok(out.report)
    }

    /// Corta el contenido de `area` (espacio de página) de la página: desaparece de ella y queda en un formulario
    /// XObject que se pega con [`PdfDocument::paste_content`]. `None` si no había nada que cortar.
    pub fn cut_content(&mut self, page: i32, area: Rect) -> Result<Option<CutContent>> {
        let id = page_index(self, page)?;
        let ctm = self.load_pdf_page(page)?.ctm()?;
        let inv = ctm
            .invert()
            .ok_or_else(|| pdf_err("la matriz de la página no es invertible"))?;
        let zone = rect_to_user(&m_of(&inv), area);
        let regions: Vec<Region> = vec![(zone, Dest::Cut)];
        let mut st = self.st.borrow_mut();
        let out = split_page(&mut st, id, &regions)?;
        // Sin `finish`: el formulario cortado no cuelga de nada hasta que se pega y la poda lo borraría.
        let (mut report, Some(cut)) = (out.report, out.cut) else {
            return Ok(None);
        };
        let links = take_links(&mut st, id, &zone);
        report.links_moved = links.len();
        let c = m_of(&ctm);
        let bounds = cut.bbox.transform(&c);
        Ok(Some(CutContent {
            form: cut.form,
            src_ctm: [c.a, c.b, c.c, c.d, c.e, c.f],
            bounds: Rect::new(bounds.x0 as f32, bounds.y0 as f32, bounds.x1 as f32, bounds.y1 as f32),
            report,
            links,
        }))
    }

    /// Pega en `page` (encima del contenido) lo cortado con [`PdfDocument::cut_content`], desplazado `(dx, dy)` en
    /// el espacio de página respecto de donde estaba.
    pub fn paste_content(&mut self, page: i32, cut: &CutContent, dx: f32, dy: f32) -> Result<()> {
        self.guard()?;
        let mut pg = self.load_pdf_page(page)?;
        let dst = m_of(&pg.ctm()?);
        let dst_inv = dst
            .invert()
            .ok_or_else(|| pdf_err("la matriz de la página no es invertible"))?;
        let src = M {
            a: cut.src_ctm[0],
            b: cut.src_ctm[1],
            c: cut.src_ctm[2],
            d: cut.src_ctm[3],
            e: cut.src_ctm[4],
            f: cut.src_ctm[5],
        };
        let m = src.then(M::translate(f64::from(dx), f64::from(dy))).then(dst_inv);

        let res = pg.resources()?;
        let mut xo = match res.get_dict("XObject")? {
            Some(x) if x.is_dict()? => x,
            _ => {
                let mut r: Object = res.clone();
                r.dict_put("XObject", self.new_dict()?)?;
                r.get_dict("XObject")?
                    .ok_or_else(|| pdf_err("no se pudo crear /XObject"))?
            }
        };
        let taken: Vec<String> = xo.dict_iter()?.into_iter().map(|(k, _)| k).collect();
        let name = (1..)
            .map(|n| format!("PlCut{n}"))
            .find(|n| !taken.contains(n))
            .unwrap_or_else(|| "PlCut".into());
        xo.dict_put(&name, Object::reference(self.state(), cut.form))?;
        let mut c = Vec::new();
        c.extend_from_slice(
            format!(
                "q\n{} {} {} {} {} {} cm\n",
                fmt_num(m.a),
                fmt_num(m.b),
                fmt_num(m.c),
                fmt_num(m.d),
                fmt_num(m.e),
                fmt_num(m.f)
            )
            .as_bytes(),
        );
        write_name(&mut c, name.as_bytes());
        c.extend_from_slice(b" Do\nQ\n");
        pg.insert_contents(self, &c, true)?;
        let page_id = page_index(self, page)?;
        let links = cut.links.iter().map(|(d, r)| (d.clone(), r.transform(&m))).collect();
        let mut st = self.st.borrow_mut();
        add_links(&mut st, page_id, links);
        finish(&mut st);
        Ok(())
    }
}
