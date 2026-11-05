//! Redacción real sobre el modelo propio (W38): reescribe los flujos de contenido de la página para que el texto,
//! las imágenes y los vectores bajo las zonas **dejen de existir** en el archivo, no solo de verse.
//!
//! # Qué hace
//!
//! - **Texto**: sigue el estado gráfico y de texto del flujo (CTM, `Tm`, `Tf`, `Tc`, `Tw`, `Tz`, `Ts`…), mide cada
//!   glifo con los anchos de la fuente y quita de la cadena los códigos cuya caja toca una zona. Los huecos se
//!   cubren con ajustes de `TJ` para que el resto del texto no se mueva. El texto invisible (`Tr 3`, capas de OCR) y
//!   el de capas opcionales apagadas también se quita: es lo seguro.
//! - **Formularios (XObjects `Form`)** que tocan una zona: se copian, se reescriben recursivamente y la página pasa a
//!   usar la copia (el original puede estar compartido con otras páginas).
//! - **Imágenes**: borra los píxeles cubiertos en una copia (Flate/sin filtro/JPEG de 1 o 3 componentes) o quita la
//!   imagen entera si el formato no se sabe tratar (JPX, JBIG2, CCITT…). Las imágenes en línea se quitan enteras.
//! - **Vectores**: quita los caminos pintados cuya caja está cubierta (o tocada) por la zona; los recortes se conservan.
//! - **Anotaciones** bajo la zona (y las propias marcas de redacción) se eliminan; los campos de formulario
//!   pierden `/V`, `/DV` y su apariencia. Se borran también la miniatura (`/Thumb`) y `/PieceInfo` de la página.
//! - **Texto alternativo** (`/ActualText`, `/Alt`) de los contenidos marcados donde se quitó texto.
//! - Opcionalmente dibuja cajas negras encima.
//!
//! # Regla de seguridad
//!
//! Si no se puede medir con certeza (fuente sin anchos conocidos, CMap predefinido, escritura vertical, posición
//! incierta tras una operación opaca), se trata la operación **entera** como sospechosa y se quita si su caja posible
//! toca la zona. Nunca se adivina un ancho. Lo que no se sabe tratar se quita o se avisa en el informe.
//!
//! # Límites conocidos (W38)
//!
//! Los programas de fuente incrustados conservan los glifos de las letras quitadas y los `ToUnicode`/`CIDToGIDMap`
//! no se tocan; el árbol de estructura (`/StructTreeRoot`) conserva `/ActualText` y `/Alt` propios; XMP, `/Info`,
//! marcadores y destinos con nombre no se limpian; los degradados (`sh`), patrones y glifos Type3 no se recortan;
//! el texto rasterizado dentro de imágenes JPX/JBIG2/CCITT se resuelve quitando la imagen.

mod geom;
mod imgs;
mod lex;
mod metrics;
mod shift;
mod std14;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::rc::Rc;

use lopdf::{Dictionary, Object as L, ObjectId, Stream};
use pliego_core::Result;

use self::geom::{M, R};
use self::imgs::{Erased, Target, erase};
use self::lex::{Instr, Operand, fmt_num, parse, write_name, write_operand, write_string};
use self::metrics::FontInfo;
pub use self::shift::{ContentMove, CutContent, MoveReport};
use super::object::decode_stream;
use super::{State, pdf_err};
use crate::geom::Rect;
use crate::{RedactImageMethod, RedactLineArtMethod, RedactOptions, RedactTextMethod};

/// Qué hizo la redacción (para el informe y las pruebas).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RedactReport {
    /// Páginas cuyo contenido o anotaciones cambiaron.
    pub pages_changed: usize,
    /// Códigos de texto quitados (aproximado: una operación opaca cuenta los bytes de su cadena).
    pub glyphs_removed: usize,
    /// Imágenes con píxeles borrados en una copia.
    pub images_erased: usize,
    /// Imágenes quitadas enteras (método `Remove`, imágenes en línea o formato no soportado).
    pub images_removed: usize,
    /// Caminos vectoriales quitados.
    pub paths_removed: usize,
    /// Formularios XObject reescritos.
    pub forms_rewritten: usize,
    /// Anotaciones eliminadas (incluidas las marcas de redacción).
    pub annotations_removed: usize,
    /// Cosas que el usuario debería saber (formatos no soportados, fuentes opacas…).
    pub warnings: Vec<String>,
}

const MAX_FORM_DEPTH: u8 = 8;
const SCRUBBED_KEYS: [&[u8]; 3] = [b"ActualText", b"Alt", b"E"];

// --- Recursos ---

struct Res {
    base: Dictionary,
    fonts: Dictionary,
    xobjects: Dictionary,
    props: Dictionary,
    modified: bool,
}

fn sub_dict(st: &State, d: &Dictionary, key: &[u8]) -> Dictionary {
    d.get(key).ok().and_then(|o| st.dict_of(o)).cloned().unwrap_or_default()
}

impl Res {
    fn new(st: &State, d: Option<&Dictionary>) -> Res {
        let base = d.cloned().unwrap_or_default();
        Res {
            fonts: sub_dict(st, &base, b"Font"),
            xobjects: sub_dict(st, &base, b"XObject"),
            props: sub_dict(st, &base, b"Properties"),
            base,
            modified: false,
        }
    }

    fn into_dict(self) -> Dictionary {
        let mut d = self.base;
        d.set("XObject", L::Dictionary(self.xobjects));
        d
    }
}

// --- Estado gráfico ---

#[derive(Clone)]
struct Gs {
    ctm: M,
    font: Option<Rc<FontInfo>>,
    size: f64,
    tc: f64,
    tw: f64,
    th: f64,
    tl: f64,
    rise: f64,
    lw: f64,
}

struct Pending {
    raw: Vec<u8>,
    bbox: Option<R>,
    clip: bool,
}

struct Frame {
    chunk: Option<usize>,
    instr: Option<Instr>,
    tainted: bool,
}

struct Run {
    gs: Gs,
    stack: Vec<Gs>,
    tm: M,
    tlm: M,
    uncertain: bool,
    in_text: bool,
    out: Vec<Vec<u8>>,
    pending: Option<Pending>,
    frames: Vec<Frame>,
    changed: bool,
    /// Recursos `XObject` cuyo `Do` se sustituyó o quitó, y los que se conservaron tal cual.
    replaced: Vec<Vec<u8>>,
    kept: Vec<Vec<u8>>,
}

struct RunOut {
    bytes: Vec<u8>,
    changed: bool,
    open_q: usize,
    in_text: bool,
}

enum Outcome {
    Keep,
    Drop,
    Replace(ObjectId),
}

enum Item {
    S(Vec<u8>),
    Adj(f64),
}

enum Elem {
    S(Vec<u8>),
    N(f64),
}

struct Redactor<'a> {
    st: &'a mut State,
    opts: RedactOptions,
    report: RedactReport,
    fonts: HashMap<ObjectId, Rc<FontInfo>>,
    counter: u32,
}

fn nums(ops: &[Operand]) -> Vec<f64> {
    ops.iter().filter_map(Operand::num).collect()
}

fn to_m(ops: &[Operand]) -> Option<M> {
    M::from_slice(&nums(ops))
}

fn push_chunk(out: &mut Vec<Vec<u8>>, bytes: &[u8]) -> usize {
    let mut v = bytes.to_vec();
    v.push(b'\n');
    out.push(v);
    out.len() - 1
}

/// Zonas (en espacio de usuario) que tocan la caja.
fn hit(areas: &[R], b: &R) -> bool {
    areas.iter().any(|a| a.intersects(b))
}

impl<'a> Redactor<'a> {
    fn new(st: &'a mut State, opts: RedactOptions) -> Self {
        Redactor {
            st,
            opts,
            report: RedactReport::default(),
            fonts: HashMap::new(),
            counter: 0,
        }
    }

    fn into_report(self) -> RedactReport {
        self.report
    }

    fn warn(&mut self, msg: String) {
        if !self.report.warnings.contains(&msg) {
            self.report.warnings.push(msg);
        }
    }

    fn font_for(&mut self, res: &Res, name: &[u8]) -> Option<Rc<FontInfo>> {
        let entry = res.fonts.get(name).ok()?.clone();
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

    fn register_xobject(&mut self, res: &mut Res, id: ObjectId) -> Vec<u8> {
        loop {
            self.counter += 1;
            let name = format!("PlRed{}", self.counter).into_bytes();
            if !res.xobjects.has(&name) {
                res.xobjects.set(name.clone(), L::Reference(id));
                res.modified = true;
                return name;
            }
        }
    }

    // --- Intérprete ---

    fn run(&mut self, content: &[u8], res: &mut Res, base: M, areas: &[R], depth: u8) -> RunOut {
        let instrs = parse(content);
        let mut r = Run {
            gs: Gs {
                ctm: base,
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
            tm: M::IDENT,
            tlm: M::IDENT,
            uncertain: false,
            in_text: false,
            out: Vec::new(),
            pending: None,
            frames: Vec::new(),
            changed: false,
            replaced: Vec::new(),
            kept: Vec::new(),
        };
        for ins in &instrs {
            let raw = &content[ins.start..ins.end];
            let op = ins.op.as_slice();
            // Los caminos pendientes solo admiten más construcción, recortes y el operador de pintado.
            let is_path = matches!(op, b"m" | b"l" | b"c" | b"v" | b"y" | b"h" | b"re" | b"W" | b"W*");
            let is_paint = matches!(
                op,
                b"S" | b"s" | b"f" | b"F" | b"f*" | b"B" | b"B*" | b"b" | b"b*" | b"n"
            );
            if r.pending.is_some() && !is_path && !is_paint {
                Self::flush_pending(&mut r);
            }
            match op {
                b"q" => {
                    r.stack.push(r.gs.clone());
                    push_chunk(&mut r.out, raw);
                }
                b"Q" => {
                    if let Some(g) = r.stack.pop() {
                        r.gs = g;
                    }
                    push_chunk(&mut r.out, raw);
                }
                b"cm" => {
                    if let Some(m) = to_m(&ins.operands) {
                        r.gs.ctm = m.then(r.gs.ctm);
                    }
                    push_chunk(&mut r.out, raw);
                }
                b"w" => {
                    if let Some(w) = ins.operands.first().and_then(Operand::num) {
                        r.gs.lw = w;
                    }
                    push_chunk(&mut r.out, raw);
                }
                b"BT" => {
                    r.tm = M::IDENT;
                    r.tlm = M::IDENT;
                    r.uncertain = false;
                    r.in_text = true;
                    push_chunk(&mut r.out, raw);
                }
                b"ET" => {
                    r.in_text = false;
                    push_chunk(&mut r.out, raw);
                }
                b"Tc" | b"Tw" | b"Tz" | b"TL" | b"Ts" | b"Tr" => {
                    if let Some(v) = ins.operands.first().and_then(Operand::num) {
                        match op {
                            b"Tc" => r.gs.tc = v,
                            b"Tw" => r.gs.tw = v,
                            b"Tz" => r.gs.th = v / 100.0,
                            b"TL" => r.gs.tl = v,
                            b"Ts" => r.gs.rise = v,
                            _ => {}
                        }
                    }
                    push_chunk(&mut r.out, raw);
                }
                b"Tf" => {
                    if let (Some(Operand::Name(n)), Some(sz)) =
                        (ins.operands.first(), ins.operands.get(1).and_then(Operand::num))
                    {
                        r.gs.font = self.font_for(res, n);
                        r.gs.size = sz;
                    }
                    push_chunk(&mut r.out, raw);
                }
                b"Td" | b"TD" => {
                    if let (Some(tx), Some(ty)) = (
                        ins.operands.first().and_then(Operand::num),
                        ins.operands.get(1).and_then(Operand::num),
                    ) {
                        if op == b"TD" {
                            r.gs.tl = -ty;
                        }
                        r.tlm = M::translate(tx, ty).then(r.tlm);
                        r.tm = r.tlm;
                        r.uncertain = false;
                    }
                    push_chunk(&mut r.out, raw);
                }
                b"T*" => {
                    r.tlm = M::translate(0.0, -r.gs.tl).then(r.tlm);
                    r.tm = r.tlm;
                    r.uncertain = false;
                    push_chunk(&mut r.out, raw);
                }
                b"Tm" => {
                    if let Some(m) = to_m(&ins.operands) {
                        r.tm = m;
                        r.tlm = m;
                        r.uncertain = false;
                    }
                    push_chunk(&mut r.out, raw);
                }
                b"Tj" | b"TJ" | b"'" | b"\"" => self.show(&mut r, ins, raw, areas),
                b"m" | b"l" | b"c" | b"v" | b"y" | b"re" | b"h" => {
                    self.path_op(&mut r, ins, raw);
                }
                b"W" | b"W*" => {
                    let p = r.pending.get_or_insert(Pending {
                        raw: Vec::new(),
                        bbox: None,
                        clip: false,
                    });
                    p.clip = true;
                    p.raw.extend_from_slice(raw);
                    p.raw.push(b'\n');
                }
                b"S" | b"s" | b"f" | b"F" | b"f*" | b"B" | b"B*" | b"b" | b"b*" | b"n" => {
                    self.paint(&mut r, op, raw, areas);
                }
                b"Do" => self.do_xobject(&mut r, ins, raw, res, areas, depth),
                b"BI" => self.inline_image(&mut r, ins, raw, areas),
                b"BMC" => {
                    let c = push_chunk(&mut r.out, raw);
                    r.frames.push(Frame {
                        chunk: Some(c),
                        instr: None,
                        tainted: false,
                    });
                }
                b"BDC" => {
                    let c = push_chunk(&mut r.out, raw);
                    r.frames.push(Frame {
                        chunk: Some(c),
                        instr: Some(ins.clone()),
                        tainted: false,
                    });
                }
                b"EMC" => {
                    if let Some(f) = r.frames.pop() {
                        self.close_frame(&mut r, f, res);
                    }
                    push_chunk(&mut r.out, raw);
                }
                _ => {
                    push_chunk(&mut r.out, raw);
                }
            }
        }
        if r.pending.is_some() {
            Self::flush_pending(&mut r);
        }
        while let Some(f) = r.frames.pop() {
            self.close_frame(&mut r, f, res);
        }
        // Los recursos que ya no se usan no pueden conservar la imagen o el formulario original.
        for n in &r.replaced {
            if !r.kept.contains(n) && res.xobjects.remove(n).is_some() {
                res.modified = true;
            }
        }
        let mut bytes = Vec::new();
        for c in &r.out {
            bytes.extend_from_slice(c);
        }
        RunOut {
            bytes,
            changed: r.changed,
            open_q: r.stack.len(),
            in_text: r.in_text,
        }
    }

    fn flush_pending(r: &mut Run) {
        if let Some(p) = r.pending.take() {
            r.out.push(p.raw);
        }
    }

    /// Reescribe el `BDC` de un contenido marcado donde se quitó texto, sin `/ActualText`, `/Alt` ni `/E`.
    fn close_frame(&mut self, r: &mut Run, f: Frame, res: &Res) {
        let (Some(chunk), Some(ins), true) = (f.chunk, f.instr, f.tainted) else {
            return;
        };
        let (Some(Operand::Name(tag)), Some(props)) = (ins.operands.first(), ins.operands.get(1)) else {
            return;
        };
        let dict: Vec<(Vec<u8>, Operand)> = match props {
            Operand::Dict(d) => d.clone(),
            Operand::Name(n) => match res.props.get(n).ok().and_then(|o| self.st.dict_of(o)) {
                Some(d) => d
                    .iter()
                    .filter_map(|(k, v)| {
                        let v = self.st.deref(v)?;
                        let o = match v {
                            L::Integer(i) => Operand::Num(*i as f64),
                            L::Real(x) => Operand::Num(f64::from(*x)),
                            L::Boolean(b) => Operand::Bool(*b),
                            L::Name(n) => Operand::Name(n.clone()),
                            _ => return None,
                        };
                        Some((k.clone(), o))
                    })
                    .collect(),
                None => return,
            },
            _ => return,
        };
        let had = match props {
            Operand::Name(n) => res
                .props
                .get(n)
                .ok()
                .and_then(|o| self.st.dict_of(o))
                .is_some_and(|d| SCRUBBED_KEYS.iter().any(|k| d.has(k))),
            _ => dict.iter().any(|(k, _)| SCRUBBED_KEYS.contains(&k.as_slice())),
        };
        if !had {
            return;
        }
        let kept: Vec<_> = dict
            .into_iter()
            .filter(|(k, _)| !SCRUBBED_KEYS.contains(&k.as_slice()))
            .collect();
        let mut b = Vec::new();
        write_name(&mut b, tag);
        b.push(b' ');
        write_operand(&mut b, &Operand::Dict(kept));
        b.extend_from_slice(b" BDC\n");
        r.out[chunk] = b;
    }

    // --- Caminos ---

    fn path_op(&mut self, r: &mut Run, ins: &Instr, raw: &[u8]) {
        let p = r.pending.get_or_insert(Pending {
            raw: Vec::new(),
            bbox: None,
            clip: false,
        });
        p.raw.extend_from_slice(raw);
        p.raw.push(b'\n');
        let v = nums(&ins.operands);
        let ctm = r.gs.ctm;
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

    fn paint(&mut self, r: &mut Run, op: &[u8], raw: &[u8], areas: &[R]) {
        let Some(p) = r.pending.take() else {
            push_chunk(&mut r.out, raw);
            return;
        };
        let stroked = matches!(op, b"S" | b"s" | b"B" | b"B*" | b"b" | b"b*");
        let drop_it = match (self.opts.line_art, p.bbox) {
            (RedactLineArtMethod::None, _) | (_, None) => false,
            (method, Some(b)) => {
                let b = if stroked {
                    b.inflate(r.gs.lw.abs() * r.gs.ctm.scale() / 2.0)
                } else {
                    b
                };
                match method {
                    RedactLineArtMethod::RemoveIfCovered => areas.iter().any(|a| b.inside(a)),
                    RedactLineArtMethod::RemoveIfTouched => hit(areas, &b),
                    RedactLineArtMethod::None => false,
                }
            }
        };
        if drop_it && op != b"n" {
            self.report.paths_removed += 1;
            r.changed = true;
            if p.clip {
                // Se conserva el recorte, se quita el pintado.
                r.out.push(p.raw);
                r.out.push(b"n\n".to_vec());
            }
        } else {
            r.out.push(p.raw);
            push_chunk(&mut r.out, raw);
        }
    }

    // --- Texto ---

    fn show(&mut self, r: &mut Run, ins: &Instr, raw: &[u8], areas: &[R]) {
        let op = ins.op.as_slice();
        let mut prefix = Vec::new();
        let items: Vec<Item> = match (op, ins.operands.as_slice()) {
            (b"Tj", [Operand::Str(s)]) => vec![Item::S(s.clone())],
            (b"TJ", [Operand::Array(a)]) => a
                .iter()
                .filter_map(|o| match o {
                    Operand::Str(s) => Some(Item::S(s.clone())),
                    Operand::Num(n) => Some(Item::Adj(*n)),
                    _ => None,
                })
                .collect(),
            (b"'", [Operand::Str(s)]) => {
                r.tlm = M::translate(0.0, -r.gs.tl).then(r.tlm);
                r.tm = r.tlm;
                r.uncertain = false;
                prefix.extend_from_slice(b"T*\n");
                vec![Item::S(s.clone())]
            }
            (b"\"", [Operand::Num(aw), Operand::Num(ac), Operand::Str(s)]) => {
                r.gs.tw = *aw;
                r.gs.tc = *ac;
                r.tlm = M::translate(0.0, -r.gs.tl).then(r.tlm);
                r.tm = r.tlm;
                r.uncertain = false;
                prefix.extend_from_slice(format!("{} Tw {} Tc T*\n", fmt_num(*aw), fmt_num(*ac)).as_bytes());
                vec![Item::S(s.clone())]
            }
            _ => {
                push_chunk(&mut r.out, raw);
                return;
            }
        };
        let remove_on = !areas.is_empty() && self.opts.text != RedactTextMethod::None;
        let (size, th) = (r.gs.size, r.gs.th);
        let font = r.gs.font.clone();
        let (asc, desc) = font.as_ref().map_or((0.9, -0.25), |f| (f.ascent, f.descent));
        let mut elems: Vec<Elem> = Vec::new();
        let mut cur: Vec<u8> = Vec::new();
        let mut removed_any = false;
        let flush = |cur: &mut Vec<u8>, elems: &mut Vec<Elem>| {
            if !cur.is_empty() {
                elems.push(Elem::S(std::mem::take(cur)));
            }
        };
        for item in items {
            match item {
                Item::Adj(n) => {
                    flush(&mut cur, &mut elems);
                    elems.push(Elem::N(n));
                    r.tm = M::translate(-n / 1000.0 * size * th, 0.0).then(r.tm);
                }
                Item::S(s) => {
                    let glyphs = font
                        .as_ref()
                        .and_then(|f| f.decode(&s))
                        .filter(|g| g.iter().all(|x| x.width.is_some()))
                        .filter(|_| !r.uncertain);
                    match glyphs {
                        Some(glyphs) => {
                            for g in glyphs {
                                let w0 = g.width.unwrap_or(0.0);
                                let adv = (w0 * size + r.gs.tc + if g.space { r.gs.tw } else { 0.0 }) * th;
                                let m = r.tm.then(r.gs.ctm);
                                let wx = w0 * size * th;
                                let (x0, x1) = if wx >= 0.0 {
                                    (wx * 0.03, wx * 0.97)
                                } else {
                                    (wx * 0.97, wx * 0.03)
                                };
                                let y0 = r.gs.rise + desc * size;
                                let y1 = r.gs.rise + asc * size;
                                let b = R::new(x0, y0, x1, y1).transform(&m);
                                let remove = remove_on && hit(areas, &b);
                                let bytes = &s[g.start..g.start + g.len];
                                if remove {
                                    removed_any = true;
                                    self.report.glyphs_removed += 1;
                                    flush(&mut cur, &mut elems);
                                    let denom = size * th;
                                    if denom.abs() > 1e-9 {
                                        let adj = -adv * 1000.0 / denom;
                                        if let Some(Elem::N(prev)) = elems.last_mut() {
                                            *prev += adj;
                                        } else {
                                            elems.push(Elem::N(adj));
                                        }
                                    }
                                } else {
                                    cur.extend_from_slice(bytes);
                                }
                                r.tm = M::translate(adv, 0.0).then(r.tm);
                            }
                        }
                        None => {
                            // Operación opaca: caja posible desde el origen del texto.
                            let ext = if r.uncertain {
                                1.0e6
                            } else {
                                s.len() as f64 * 2.0 * size.abs() * th.abs() + 1.0
                            };
                            let y0 = r.gs.rise + (desc - 0.25) * size;
                            let y1 = r.gs.rise + (asc + 0.25) * size;
                            let b = R::new(0.0, y0, ext, y1).transform(&r.tm.then(r.gs.ctm));
                            if remove_on && hit(areas, &b) {
                                removed_any = true;
                                self.report.glyphs_removed += s.len();
                                flush(&mut cur, &mut elems);
                                if font.as_ref().is_none_or(|f| f.is_opaque()) {
                                    let name = "una fuente cuyos anchos no se pueden medir";
                                    self.warn(format!(
                                        "Se quitó texto entero en {name}; puede haberse quitado más de lo marcado."
                                    ));
                                }
                            } else {
                                cur.extend_from_slice(&s);
                            }
                            r.uncertain = true;
                        }
                    }
                }
            }
        }
        flush(&mut cur, &mut elems);
        if removed_any {
            r.changed = true;
            for f in &mut r.frames {
                f.tainted = true;
            }
            let mut b = prefix;
            b.push(b'[');
            for e in &elems {
                match e {
                    Elem::S(s) => write_string(&mut b, s),
                    Elem::N(n) => b.extend_from_slice(fmt_num(*n).as_bytes()),
                }
            }
            b.extend_from_slice(b"] TJ\n");
            r.out.push(b);
        } else {
            push_chunk(&mut r.out, raw);
        }
    }

    // --- Imágenes y formularios ---

    fn inline_image(&mut self, r: &mut Run, _ins: &Instr, raw: &[u8], areas: &[R]) {
        let b = R::new(0.0, 0.0, 1.0, 1.0).transform(&r.gs.ctm);
        if self.opts.image_method != RedactImageMethod::None && hit(areas, &b) {
            self.report.images_removed += 1;
            r.changed = true;
        } else {
            push_chunk(&mut r.out, raw);
        }
    }

    fn do_xobject(&mut self, r: &mut Run, ins: &Instr, raw: &[u8], res: &mut Res, areas: &[R], depth: u8) {
        let Some(Operand::Name(name)) = ins.operands.first() else {
            push_chunk(&mut r.out, raw);
            return;
        };
        match self.xobject_outcome(r.gs.ctm, name, res, areas, depth) {
            Outcome::Keep => {
                r.kept.push(name.clone());
                push_chunk(&mut r.out, raw);
            }
            Outcome::Drop => {
                r.replaced.push(name.clone());
                r.changed = true;
            }
            Outcome::Replace(id) => {
                let n = self.register_xobject(res, id);
                let mut b = Vec::new();
                write_name(&mut b, &n);
                b.extend_from_slice(b" Do\n");
                r.out.push(b);
                r.replaced.push(name.clone());
                r.changed = true;
            }
        }
    }

    fn xobject_outcome(&mut self, ctm: M, name: &[u8], res: &mut Res, areas: &[R], depth: u8) -> Outcome {
        let id = match res.xobjects.get(name) {
            Ok(L::Reference(id)) => self.st.resolve_id(*id),
            _ => return Outcome::Keep,
        };
        let (subtype, stream) = match self.st.doc.objects.get(&id) {
            Some(L::Stream(s)) => (
                s.dict.get(b"Subtype").ok().and_then(|o| match o {
                    L::Name(n) => Some(n.clone()),
                    _ => None,
                }),
                s.clone(),
            ),
            _ => return Outcome::Keep,
        };
        match subtype.as_deref() {
            Some(b"Image") => {
                let b = R::new(0.0, 0.0, 1.0, 1.0).transform(&ctm);
                if self.opts.image_method == RedactImageMethod::None || !hit(areas, &b) {
                    return Outcome::Keep;
                }
                if self.opts.image_method == RedactImageMethod::Remove {
                    self.report.images_removed += 1;
                    return Outcome::Drop;
                }
                match self.erase_image(&stream, &ctm, areas) {
                    Some(Some(new_id)) => {
                        self.report.images_erased += 1;
                        Outcome::Replace(new_id)
                    }
                    Some(None) => Outcome::Keep,
                    None => {
                        self.report.images_removed += 1;
                        self.warn(
                            "Se quitó una imagen entera porque su formato (JPX, JBIG2, CCITT…) no permite borrar \
                             solo los píxeles cubiertos."
                                .into(),
                        );
                        Outcome::Drop
                    }
                }
            }
            Some(b"Form") => {
                let fm = form_matrix(self.st, &stream).then(ctm);
                let bbox = form_bbox(self.st, &stream).map(|b| b.transform(&fm));
                if bbox.is_some_and(|b| !hit(areas, &b)) {
                    return Outcome::Keep;
                }
                if depth >= MAX_FORM_DEPTH {
                    self.warn("Se quitó un formulario XObject anidado a demasiada profundidad.".into());
                    return Outcome::Drop;
                }
                match self.redact_form(&stream, fm, res, areas, depth + 1) {
                    Some(new_id) => {
                        self.report.forms_rewritten += 1;
                        Outcome::Replace(new_id)
                    }
                    None => Outcome::Keep,
                }
            }
            _ => Outcome::Keep,
        }
    }

    /// `Some(Some(id))` = imagen nueva; `Some(None)` = sin cambios; `None` = no se sabe tratar (quitarla).
    fn erase_image(&mut self, img: &Stream, ctm: &M, areas: &[R]) -> Option<Option<ObjectId>> {
        let is_mask = matches!(img.dict.get(b"ImageMask"), Ok(L::Boolean(true)));
        let target = if is_mask { Target::Unpainted } else { Target::Blank };
        let mut new = match erase(self.st, img, ctm, areas, target)? {
            Erased::Same => return Some(None),
            Erased::New(s) => s,
        };
        for (key, target) in [(&b"SMask"[..], Target::Opaque), (&b"Mask"[..], Target::Painted)] {
            let Some(L::Reference(mid)) = img.dict.get(key).ok().cloned() else {
                continue;
            };
            let mid = self.st.resolve_id(mid);
            let Some(L::Stream(ms)) = self.st.doc.objects.get(&mid).cloned() else {
                continue;
            };
            match erase(self.st, &ms, ctm, areas, target)? {
                Erased::Same => {}
                Erased::New(ns) => {
                    let nid = self.st.add(L::Stream(ns));
                    new.dict.set(key, L::Reference(nid));
                }
            }
        }
        Some(Some(self.st.add(L::Stream(new))))
    }

    /// Copia un formulario con el contenido reescrito. `None` si no cambia nada.
    fn redact_form(&mut self, form: &Stream, fm: M, parent: &Res, areas: &[R], depth: u8) -> Option<ObjectId> {
        let content = decode_stream(form).ok()?;
        let mut res = match form.dict.get(b"Resources").ok().and_then(|o| self.st.dict_of(o)) {
            Some(d) => {
                let d = d.clone();
                Res::new(self.st, Some(&d))
            }
            None => Res {
                base: parent.base.clone(),
                fonts: parent.fonts.clone(),
                xobjects: parent.xobjects.clone(),
                props: parent.props.clone(),
                modified: false,
            },
        };
        let out = self.run(&content, &mut res, fm, areas, depth);
        if !out.changed {
            return None;
        }
        let mut dict = form.dict.clone();
        dict.remove(b"Filter");
        dict.remove(b"DecodeParms");
        dict.remove(b"PieceInfo");
        dict.remove(b"Metadata");
        dict.set("Resources", L::Dictionary(res.into_dict()));
        let mut bytes = b"q\n".to_vec();
        bytes.extend_from_slice(&out.bytes);
        if out.in_text {
            bytes.extend_from_slice(b"ET\n");
        }
        for _ in 0..out.open_q {
            bytes.extend_from_slice(b"Q\n");
        }
        bytes.extend_from_slice(b"Q\n");
        let mut s = Stream::new(dict, bytes);
        s.allows_compression = true;
        let _ = s.compress();
        Some(self.st.add(L::Stream(s)))
    }

    // --- Página ---

    /// Redacta una página. `areas` en espacio de usuario. Devuelve si algo cambió.
    fn redact_page(&mut self, page_id: ObjectId, areas: &[R]) -> Result<bool> {
        if areas.is_empty() {
            return Ok(false);
        }
        let page_dict = self
            .st
            .dict_of(self.st.get(page_id).ok_or_else(|| pdf_err("la página no existe"))?)
            .ok_or_else(|| pdf_err("la página no es un diccionario"))?
            .clone();
        // Contenido: uno o varios flujos, unidos con un espacio en blanco (un operador no cruza flujos).
        let mut content = Vec::new();
        let parts: Vec<L> = match page_dict.get(b"Contents").ok().and_then(|o| self.st.deref(o)) {
            Some(L::Array(a)) => a.clone(),
            Some(s @ L::Stream(_)) => vec![s.clone()],
            _ => Vec::new(),
        };
        for p in &parts {
            if let Some(L::Stream(s)) = self.st.deref(p) {
                content.extend_from_slice(&decode_stream(s)?);
                content.push(b'\n');
            }
        }
        // Recursos propios o heredados.
        let res_dict = page_dict
            .get(b"Resources")
            .ok()
            .and_then(|o| self.st.dict_of(o))
            .cloned()
            .or_else(|| inherited_resources(self.st, &page_dict));
        let mut res = Res::new(self.st, res_dict.as_ref());
        let out = self.run(&content, &mut res, M::IDENT, areas, 0);

        let mut changed = out.changed;
        if self.opts.black_boxes || out.changed {
            let mut bytes = b"q\n".to_vec();
            bytes.extend_from_slice(&out.bytes);
            if out.in_text {
                bytes.extend_from_slice(b"ET\n");
            }
            for _ in 0..out.open_q {
                bytes.extend_from_slice(b"Q\n");
            }
            bytes.extend_from_slice(b"Q\n");
            if self.opts.black_boxes {
                for a in areas {
                    bytes.extend_from_slice(
                        format!(
                            "q 0 g {} {} {} {} re f Q\n",
                            fmt_num(a.x0),
                            fmt_num(a.y0),
                            fmt_num(a.x1 - a.x0),
                            fmt_num(a.y1 - a.y0)
                        )
                        .as_bytes(),
                    );
                }
                changed = true;
            }
            let mut s = Stream::new(Dictionary::new(), bytes);
            s.allows_compression = true;
            let _ = s.compress();
            let sid = self.st.add(L::Stream(s));
            super::document::set_dict_entry(self.st, page_id, "Contents", L::Reference(sid))?;
            if res.modified {
                super::document::set_dict_entry(self.st, page_id, "Resources", L::Dictionary(res.into_dict()))?;
            }
        }
        // Anotaciones y rastros de la página original.
        let removed = self.redact_annotations(page_id, &page_dict, areas)?;
        if removed > 0 {
            changed = true;
        }
        if changed {
            if let Some(d) = super::document::dict_of_mut(self.st, page_id) {
                d.remove(b"Thumb");
                d.remove(b"PieceInfo");
            }
            self.st.touch(page_id);
        }
        Ok(changed)
    }

    fn redact_annotations(&mut self, page_id: ObjectId, page: &Dictionary, areas: &[R]) -> Result<usize> {
        let Some(annots) = page.get(b"Annots").ok().and_then(|o| self.st.deref(o)).cloned() else {
            return Ok(0);
        };
        let L::Array(items) = annots else { return Ok(0) };
        let mut keep = Vec::with_capacity(items.len());
        let mut removed = 0;
        for item in items {
            let hit_it = {
                let d = self.st.dict_of(&item);
                d.and_then(|d| {
                    let rect = d.get(b"Rect").ok().and_then(|o| self.st.deref(o))?;
                    let L::Array(a) = rect else { return None };
                    let v: Vec<f64> = a
                        .iter()
                        .filter_map(|o| match self.st.deref(o)? {
                            L::Integer(i) => Some(*i as f64),
                            L::Real(r) => Some(f64::from(*r)),
                            _ => None,
                        })
                        .collect();
                    (v.len() == 4).then(|| R::new(v[0], v[1], v[2], v[3]))
                })
                .is_some_and(|r| hit(areas, &r))
            };
            let is_mark = self
                .st
                .dict_of(&item)
                .and_then(|d| d.get(b"Subtype").ok())
                .and_then(|o| self.st.deref(o))
                .is_some_and(|o| matches!(o, L::Name(n) if n == b"Redact"));
            if hit_it || is_mark {
                removed += 1;
                if let L::Reference(id) = item {
                    self.scrub_annotation(id);
                }
            } else {
                keep.push(item);
            }
        }
        if removed > 0 {
            self.report.annotations_removed += removed;
            super::document::set_dict_entry(self.st, page_id, "Annots", L::Array(keep))?;
        }
        Ok(removed)
    }

    /// Una anotación eliminada puede seguir alcanzable (campos de formulario, estructura): se vacía.
    fn scrub_annotation(&mut self, id: ObjectId) {
        let id = self.st.resolve_id(id);
        let mut chain = vec![id];
        if let Some(d) = self.st.doc.objects.get(&id).and_then(|o| match o {
            L::Dictionary(d) => Some(d),
            _ => None,
        }) && let Ok(L::Reference(p)) = d.get(b"Parent")
        {
            chain.push(*p);
        }
        for (i, oid) in chain.into_iter().enumerate() {
            if let Some(d) = super::document::dict_of_mut(self.st, oid) {
                for k in [
                    &b"V"[..],
                    b"DV",
                    b"RV",
                    b"AP",
                    b"Contents",
                    b"RC",
                    b"OverlayText",
                    b"RO",
                ] {
                    d.remove(k);
                }
                if i == 0 {
                    d.remove(b"Popup");
                }
                self.st.touch(oid);
            }
        }
    }
}

fn form_matrix(st: &State, s: &Stream) -> M {
    match s.dict.get(b"Matrix").ok().and_then(|o| st.deref(o)) {
        Some(L::Array(a)) => {
            let v: Vec<f64> = a.iter().filter_map(|o| num(st, o)).collect();
            M::from_slice(&v).unwrap_or(M::IDENT)
        }
        _ => M::IDENT,
    }
}

fn form_bbox(st: &State, s: &Stream) -> Option<R> {
    match s.dict.get(b"BBox").ok().and_then(|o| st.deref(o)) {
        Some(L::Array(a)) => {
            let v: Vec<f64> = a.iter().filter_map(|o| num(st, o)).collect();
            (v.len() == 4).then(|| R::new(v[0], v[1], v[2], v[3]))
        }
        _ => None,
    }
}

fn num(st: &State, o: &L) -> Option<f64> {
    match st.deref(o)? {
        L::Integer(i) => Some(*i as f64),
        L::Real(r) => Some(f64::from(*r)),
        _ => None,
    }
}

/// `/Resources` heredado de los nodos `/Pages` ancestros.
fn inherited_resources(st: &State, page: &Dictionary) -> Option<Dictionary> {
    let mut cur = page.get(b"Parent").ok().cloned();
    for _ in 0..32 {
        let c = cur?;
        let node = st.dict_of(&c)?;
        if let Some(r) = node.get(b"Resources").ok().and_then(|o| st.dict_of(o)) {
            return Some(r.clone());
        }
        cur = node.get(b"Parent").ok().cloned();
    }
    None
}

/// Tras redactar: quita del documento todo lo que ya no cuelga del `trailer` (los flujos de contenido y las imágenes
/// originales) y desactiva el guardado incremental, que conservaría los bytes originales.
fn finish(st: &mut State) {
    let before: Vec<ObjectId> = st.doc.objects.keys().copied().collect();
    super::save::prune(&mut st.doc);
    for id in before {
        if !st.doc.objects.contains_key(&id) {
            st.dirty.remove(&id);
            st.deleted.insert(id);
        }
    }
    st.no_incremental = true;
    st.mark_changed();
}

// --- API pública ---

impl super::PdfDocument {
    /// Aplica las marcas de redacción (anotaciones `Redact`) de todas las páginas: elimina de verdad el texto, las
    /// imágenes y los vectores bajo ellas y quita las marcas. Cada marca usa sus `QuadPoints` o, si no los tiene,
    /// su `Rect` (espacio de usuario, como en el archivo).
    pub fn apply_redactions(&mut self, options: RedactOptions) -> Result<RedactReport> {
        let ids = self.page_ids();
        let mut st = self.st.borrow_mut();
        let mut per_page: Vec<(ObjectId, Vec<R>)> = Vec::new();
        for id in ids {
            let areas = redact_marks(&st, id);
            if !areas.is_empty() {
                per_page.push((id, areas));
            }
        }
        let mut red = Redactor::new(&mut st, options);
        for (id, areas) in per_page {
            if red.redact_page(id, &areas)? {
                red.report.pages_changed += 1;
            }
        }
        let report = red.into_report();
        finish(&mut st);
        Ok(report)
    }

    /// Redacta zonas dadas en espacio de página (el de `Page::bounds`: origen arriba a la izquierda, con giro
    /// aplicado). Cada elemento es (página desde 0, zona).
    pub fn redact_areas(&mut self, areas: &[(i32, Rect)], options: RedactOptions) -> Result<RedactReport> {
        let ids = self.page_ids();
        let mut per_page: Vec<(ObjectId, Vec<R>)> = Vec::new();
        for (page, rect) in areas {
            let idx = usize::try_from(*page)
                .ok()
                .filter(|i| *i < ids.len())
                .ok_or_else(|| pdf_err(format!("la página {} no existe", page + 1)))?;
            let pg = self.load_pdf_page(*page)?;
            let inv = pg
                .ctm()?
                .invert()
                .ok_or_else(|| pdf_err("la matriz de la página no es invertible"))?;
            let u = rect.transform(&inv);
            let r = R::new(f64::from(u.x0), f64::from(u.y0), f64::from(u.x1), f64::from(u.y1));
            match per_page.iter_mut().find(|(id, _)| *id == ids[idx]) {
                Some((_, v)) => v.push(r),
                None => per_page.push((ids[idx], vec![r])),
            }
        }
        let mut st = self.st.borrow_mut();
        let mut red = Redactor::new(&mut st, options);
        for (id, areas) in per_page {
            if red.redact_page(id, &areas)? {
                red.report.pages_changed += 1;
            }
        }
        let report = red.into_report();
        finish(&mut st);
        Ok(report)
    }
}

/// Zonas de las anotaciones `Redact` de una página, en espacio de usuario.
fn redact_marks(st: &State, page_id: ObjectId) -> Vec<R> {
    let mut out = Vec::new();
    let Some(page) = st.get(page_id).and_then(|o| st.dict_of(o)) else {
        return out;
    };
    let Some(L::Array(annots)) = page.get(b"Annots").ok().and_then(|o| st.deref(o)) else {
        return out;
    };
    for a in annots {
        let Some(d) = st.dict_of(a) else { continue };
        if !matches!(d.get(b"Subtype").ok().and_then(|o| st.deref(o)), Some(L::Name(n)) if n == b"Redact") {
            continue;
        }
        let read = |key: &[u8]| -> Vec<f64> {
            match d.get(key).ok().and_then(|o| st.deref(o)) {
                Some(L::Array(v)) => v.iter().filter_map(|o| num(st, o)).collect(),
                _ => Vec::new(),
            }
        };
        let quads = read(b"QuadPoints");
        if quads.len() >= 8 {
            for q in quads.chunks(8).filter(|q| q.len() == 8) {
                let pts: Vec<(f64, f64)> = q.chunks(2).map(|p| (p[0], p[1])).collect();
                out.push(R::of_points(&pts));
            }
            continue;
        }
        let rect = read(b"Rect");
        if rect.len() == 4 {
            out.push(R::new(rect[0], rect[1], rect[2], rect[3]));
        }
    }
    out
}
