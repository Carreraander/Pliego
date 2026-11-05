//! Conexión del motor de JavaScript de formularios (crate `pliego-formjs`, W42) con el árbol de campos nativo.
//!
//! El motor trabaja sobre una copia en memoria del formulario ([`pliego_formjs::Form`]); este módulo la **construye**
//! desde los objetos del PDF (campos, valores, banderas, acciones `/AA`, orden de cálculo `/CO`, scripts de
//! documento) y **aplica de vuelta** los cambios que dejan los scripts: valores (`/V`, `/AS`), propiedades
//! (banderas, colores, tamaño de letra, visibilidad, opciones...) y el texto ya formateado de cada campo, que es lo
//! que dibuja la apariencia mientras `/V` conserva lo tecleado.
//!
//! El motor se crea la primera vez que hace falta y se reutiliza mientras nadie más modifique el documento
//! (`State::js.generation` cuenta las modificaciones); si el documento cambia por otra vía, se reconstruye.

use std::collections::{BTreeMap, HashMap, HashSet};

use lopdf::{Dictionary, Object as L, ObjectId};
use pliego_core::Result;
use pliego_formjs as js;

pub use js::Message as JsMessage;

use super::annot::parse_da;
use super::appearance::n;
use super::document::{decode_text, text_string};
use super::widget::{
    acroform_dict, dict_at, field_node, inherited, leaves, name_bytes, put, qualified_name, rebuild_ap, store_value,
    text_of,
};
use super::{Shared, State, tree};
use crate::annot::AnnotationColor;
use crate::widget::WidgetType;

/// Estado del JavaScript de formularios dentro del documento.
#[derive(Default)]
pub(crate) struct JsState {
    pub(crate) enabled: bool,
    /// Se incrementa en cada modificación del documento (`State::touch`).
    pub(crate) generation: u64,
    runtime: Option<Runtime>,
    /// Texto formateado de los campos de texto (lo que se dibuja), por nodo de campo.
    pub(crate) display: HashMap<ObjectId, String>,
    /// Avisos pendientes de recoger con `PdfDocument::take_js_messages`.
    pub(crate) log: Vec<JsMessage>,
}

struct Runtime {
    engine: js::Engine,
    /// Nombre completo del campo → nodo del campo.
    ids: HashMap<String, ObjectId>,
    generation: u64,
}

fn config() -> js::Config {
    js::Config::default()
}

// --- Del PDF al motor --------------------------------------------------------------------------------------------

const MAX_JS_BYTES: usize = 8 << 20;

fn bytes_text(b: &[u8]) -> String {
    if b.starts_with(&[0xFE, 0xFF]) {
        let units: Vec<u16> = b[2..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16::from_be_bytes(*c))
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(b.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(b)).into_owned()
    }
}

/// Código de una acción `/S /JavaScript` (`/JS` es una cadena o un flujo).
fn action_js(st: &State, o: &L) -> Option<String> {
    let d = st.dict_of(o)?;
    if d.get(b"S").ok().and_then(|s| s.as_name().ok())? != b"JavaScript" {
        return None;
    }
    match st.deref(d.get(b"JS").ok()?)? {
        s @ L::String(..) => decode_text(s),
        L::Stream(s) => {
            let b = s
                .decompressed_content_with_limit(MAX_JS_BYTES)
                .unwrap_or_else(|_| s.content.clone());
            Some(bytes_text(&b))
        }
        _ => None,
    }
}

fn color_of(arr: Option<Vec<f32>>) -> js::Color {
    match arr.as_deref() {
        Some([g]) => js::Color::Gray(f64::from(*g)),
        Some([r, g, b]) => js::Color::Rgb(f64::from(*r), f64::from(*g), f64::from(*b)),
        Some([c, m, y, k]) => js::Color::Cmyk(f64::from(*c), f64::from(*m), f64::from(*y), f64::from(*k)),
        _ => js::Color::Transparent,
    }
}

fn kind_of(ft: &[u8], ff: u32) -> Option<js::FieldKind> {
    Some(match ft {
        b"Tx" => js::FieldKind::Text,
        b"Sig" => js::FieldKind::Signature,
        b"Btn" if ff & js::flags::PUSHBUTTON != 0 => js::FieldKind::Button,
        b"Btn" if ff & js::flags::RADIO != 0 => js::FieldKind::Radio,
        b"Btn" => js::FieldKind::Checkbox,
        b"Ch" if ff & js::flags::COMBO != 0 => js::FieldKind::Combo,
        b"Ch" => js::FieldKind::List,
        _ => return None,
    })
}

fn widget_type(k: js::FieldKind) -> WidgetType {
    match k {
        js::FieldKind::Text => WidgetType::Text,
        js::FieldKind::Checkbox => WidgetType::Checkbox,
        js::FieldKind::Radio => WidgetType::RadioButton,
        js::FieldKind::Button => WidgetType::Button,
        js::FieldKind::Combo => WidgetType::Combobox,
        js::FieldKind::List => WidgetType::Listbox,
        js::FieldKind::Signature => WidgetType::Signature,
    }
}

fn read_field(st: &State, node: ObjectId, name: &str, page: usize, acro: Option<&Dictionary>) -> Option<js::Field> {
    let ft = inherited(st, node, "FT").and_then(|o| name_bytes(&o).map(<[u8]>::to_vec))?;
    let ff = inherited(st, node, "Ff")
        .and_then(|o| o.as_float().ok())
        .map_or(0, |f| f as u32);
    let kind = kind_of(&ft, ff)?;
    let widgets = leaves(st, node);
    let first = widgets.first().copied().unwrap_or(node);
    let mut f = js::Field::new(name, kind);
    f.flags = ff;
    f.page = page as u32;

    // Valores. Casillas y radios guardan un nombre; el resto, cadenas (o una matriz en las listas múltiples).
    let value_of = |key: &str| -> (String, Vec<String>) {
        match inherited(st, node, key) {
            Some(L::Array(a)) => {
                let items: Vec<String> = a.iter().filter_map(|x| st.deref(x).and_then(text_of)).collect();
                (
                    items.first().cloned().unwrap_or_default(),
                    if items.len() > 1 { items } else { Vec::new() },
                )
            }
            Some(o) => (text_of(&o).unwrap_or_default(), Vec::new()),
            None => (String::new(), Vec::new()),
        }
    };
    (f.value, f.multi) = value_of("V");
    f.default_value = value_of("DV").0;
    if matches!(kind, js::FieldKind::Checkbox | js::FieldKind::Radio) {
        if f.value.is_empty() {
            f.value = "Off".into();
        }
        for w in &widgets {
            if let Some(s) = super::widget::on_states(st, *w).into_iter().next() {
                f.export_values.push(s);
            }
        }
    }
    if let Some(L::Array(opt)) = inherited(st, node, "Opt") {
        for o in &opt {
            match st.deref(o) {
                Some(L::Array(pair)) if pair.len() >= 2 => {
                    let e = st.deref(&pair[0]).and_then(text_of).unwrap_or_default();
                    let l = st.deref(&pair[1]).and_then(text_of).unwrap_or_default();
                    f.options.push((e, l));
                }
                Some(s) => {
                    let t = text_of(s).unwrap_or_default();
                    f.options.push((t.clone(), t));
                }
                None => {}
            }
        }
    }

    // Aspecto.
    if let Some(d) = dict_at(st, first) {
        let rect = d
            .get(b"Rect")
            .ok()
            .and_then(|r| st.deref(r))
            .map(|r| super::widget::floats(st, r))
            .unwrap_or_default();
        if rect.len() >= 4 {
            let r: Vec<f64> = rect.iter().map(|v| f64::from(*v)).collect();
            f.rect = [r[0].min(r[2]), r[1].min(r[3]), r[0].max(r[2]), r[1].max(r[3])];
        }
        let wf = d.get(b"F").ok().and_then(|v| v.as_float().ok()).map_or(4, |v| v as i64);
        f.display = if wf & 2 != 0 {
            1
        } else if wf & 32 != 0 {
            3
        } else if wf & 4 == 0 {
            2
        } else {
            0
        };
        if let Some(L::Dictionary(mk)) = d.get(b"MK").ok().and_then(|m| st.deref(m)) {
            let col = |k: &str| {
                mk.get(k.as_bytes())
                    .ok()
                    .and_then(|v| st.deref(v))
                    .map(|v| super::widget::floats(st, v))
            };
            f.fill_color = color_of(col("BG"));
            f.stroke_color = color_of(col("BC"));
            if let Some(ca) = mk.get(b"CA").ok().and_then(|v| st.deref(v)).and_then(text_of) {
                f.extra.insert("buttonCaption".into(), js::Prop::Str(ca));
            }
        }
        if let Some(L::Dictionary(bs)) = d.get(b"BS").ok().and_then(|m| st.deref(m)) {
            f.border_style = match bs.get(b"S").ok().and_then(|s| s.as_name().ok()) {
                Some(b"D") => "dashed",
                Some(b"B") => "beveled",
                Some(b"I") => "inset",
                Some(b"U") => "underline",
                _ => "solid",
            }
            .into();
            if let Some(w) = bs.get(b"W").ok().and_then(|w| w.as_float().ok()) {
                f.line_width = f64::from(w);
            }
        }
    }
    let da = inherited(st, node, "DA")
        .as_ref()
        .and_then(text_of)
        .or_else(|| {
            acro.and_then(|a| a.get(b"DA").ok())
                .and_then(|v| st.deref(v))
                .and_then(text_of)
        })
        .as_deref()
        .and_then(parse_da);
    if let Some(da) = da {
        f.text_font = da.font_name;
        f.text_size = f64::from(da.size);
        f.text_color = match da.color {
            Some(AnnotationColor::Gray(g)) => js::Color::Gray(f64::from(g)),
            Some(AnnotationColor::Rgb { red, green, blue }) => {
                js::Color::Rgb(f64::from(red), f64::from(green), f64::from(blue))
            }
            Some(AnnotationColor::Cmyk {
                cyan,
                magenta,
                yellow,
                key,
            }) => js::Color::Cmyk(f64::from(cyan), f64::from(magenta), f64::from(yellow), f64::from(key)),
            None => js::Color::Gray(0.0),
        };
    }
    f.alignment = inherited(st, node, "Q")
        .or_else(|| acro.and_then(|a| a.get(b"Q").ok()).and_then(|v| st.deref(v)).cloned())
        .and_then(|q| q.as_float().ok())
        .map_or(0, |q| (q as i64).clamp(0, 2) as u8);
    f.max_len = inherited(st, node, "MaxLen")
        .and_then(|m| m.as_float().ok())
        .map_or(0, |m| m.max(0.0) as u32);
    f.tooltip = inherited(st, node, "TU").as_ref().and_then(text_of).unwrap_or_default();

    // Acciones: las del campo y las del primer widget (`/AA`), y la del botón (`/A`).
    for holder in [node, first] {
        let Some(d) = dict_at(st, holder) else { continue };
        if let Some(L::Dictionary(aa)) = d.get(b"AA").ok().and_then(|a| st.deref(a)) {
            for (k, v) in aa.iter() {
                if let Some(code) = action_js(st, v) {
                    f.scripts.entry(String::from_utf8_lossy(k).into_owned()).or_insert(code);
                }
            }
        }
        if let Some(code) = d.get(b"A").ok().and_then(|a| action_js(st, a)) {
            f.scripts.entry("A".into()).or_insert(code);
        }
    }
    Some(f)
}

/// Pares (nombre, acción) de un árbol de nombres (`/Names` y `/Kids`).
fn name_tree(st: &State, node: &Dictionary, out: &mut Vec<(String, String)>, depth: usize) {
    if depth > 8 {
        return;
    }
    if let Some(L::Array(names)) = node.get(b"Names").ok().and_then(|n| st.deref(n)) {
        for pair in names.chunks(2) {
            if let [k, v] = pair
                && let Some(code) = action_js(st, v)
            {
                out.push((st.deref(k).and_then(text_of).unwrap_or_default(), code));
            }
        }
    }
    if let Some(L::Array(kids)) = node.get(b"Kids").ok().and_then(|n| st.deref(n)) {
        for k in kids {
            if let Some(d) = st.dict_of(k) {
                name_tree(st, d, out, depth + 1);
            }
        }
    }
}

/// Construye el formulario del motor y el índice nombre → nodo de campo.
fn build(st: &State, pages: &[ObjectId]) -> (js::Form, HashMap<String, ObjectId>) {
    let acro = acroform_dict(st);
    let mut form = js::Form {
        page_count: pages.len() as u32,
        ..Default::default()
    };
    let mut ids: HashMap<String, ObjectId> = HashMap::new();
    let mut seen: HashSet<ObjectId> = HashSet::new();
    for (pi, pid) in pages.iter().enumerate() {
        let Some(pd) = dict_at(st, *pid) else { continue };
        let Some(L::Array(annots)) = pd.get(b"Annots").ok().and_then(|a| st.deref(a)) else {
            continue;
        };
        for a in annots {
            let L::Reference(r) = a else { continue };
            let r = st.resolve_id(*r);
            let Some(d) = dict_at(st, r) else { continue };
            if !matches!(d.get(b"Subtype"), Ok(L::Name(s)) if s == b"Widget") {
                continue;
            }
            let node = field_node(st, r);
            if !seen.insert(node) {
                continue;
            }
            let Some(name) = qualified_name(st, node) else { continue };
            if ids.contains_key(&name) {
                continue;
            }
            if let Some(f) = read_field(st, node, &name, pi, acro.as_ref()) {
                ids.insert(name, node);
                form.fields.push(f);
            }
        }
    }
    if let Some(L::Array(co)) = acro.as_ref().and_then(|a| a.get(b"CO").ok()).and_then(|c| st.deref(c)) {
        for c in co {
            if let L::Reference(r) = c
                && let Some(name) = qualified_name(st, st.resolve_id(*r))
            {
                form.calc_order.push(name);
            }
        }
    }
    if let Some(cat) = super::document::catalog_dict(st)
        && let Some(L::Dictionary(names)) = cat.get(b"Names").ok().and_then(|n| st.deref(n))
        && let Some(L::Dictionary(jsn)) = names.get(b"JavaScript").ok().and_then(|n| st.deref(n))
    {
        name_tree(st, jsn, &mut form.doc_scripts, 0);
    }
    (form, ids)
}

// --- Del motor al PDF --------------------------------------------------------------------------------------------

fn color_ops(c: &js::Color, stroke: bool) -> Option<String> {
    let (g, rg, k) = if stroke { ("G", "RG", "K") } else { ("g", "rg", "k") };
    match c {
        js::Color::Transparent => None,
        js::Color::Gray(v) => Some(format!("{} {g}", n(*v as f32))),
        js::Color::Rgb(r, gr, b) => Some(format!("{} {} {} {rg}", n(*r as f32), n(*gr as f32), n(*b as f32))),
        js::Color::Cmyk(c, m, y, kk) => Some(format!(
            "{} {} {} {} {k}",
            n(*c as f32),
            n(*m as f32),
            n(*y as f32),
            n(*kk as f32)
        )),
    }
}

fn color_array(c: &js::Color) -> Option<L> {
    let v: Vec<f64> = match c {
        js::Color::Transparent => return None,
        js::Color::Gray(g) => vec![*g],
        js::Color::Rgb(r, g, b) => vec![*r, *g, *b],
        js::Color::Cmyk(c, m, y, k) => vec![*c, *m, *y, *k],
    };
    Some(L::Array(v.into_iter().map(|x| L::Real(x as f32)).collect()))
}

fn mk_put(st: &mut State, widgets: &[ObjectId], key: &str, v: Option<L>) {
    for w in widgets {
        let id = st.resolve_id(*w);
        let mk = match dict_at(st, id)
            .and_then(|d| d.get(b"MK").ok())
            .and_then(|m| st.deref(m))
        {
            Some(L::Dictionary(m)) => m.clone(),
            _ => Dictionary::new(),
        };
        let mut mk = mk;
        match &v {
            Some(v) => mk.set(key, v.clone()),
            None => {
                mk.remove(key.as_bytes());
            }
        }
        put(st, id, "MK", Some(L::Dictionary(mk)));
    }
}

/// Escribe en el PDF una propiedad de campo que cambió un script (la lectura sale del campo del motor).
fn write_prop(st: &mut State, node: ObjectId, widgets: &[ObjectId], f: &js::Field, prop: &str) {
    match prop {
        // Todas las banderas viven en /Ff.
        "readonly" | "required" | "noExport" | "multiline" | "password" | "fileSelect" | "doNotSpellCheck"
        | "doNotScroll" | "comb" | "richText" | "commitOnSelChange" | "editable" | "multipleSelection" | "sort"
        | "radiosInUnison" => put(st, node, "Ff", Some(L::Integer(i64::from(f.flags)))),
        "display" | "hidden" => {
            for w in widgets {
                let cur = dict_at(st, *w)
                    .and_then(|d| d.get(b"F").ok())
                    .and_then(|v| v.as_float().ok())
                    .map_or(4, |v| v as i64);
                // Bits: 2 oculto, 4 imprimir, 32 no mostrar en pantalla.
                let base = cur & !(2 | 4 | 32);
                let flags = match f.display {
                    1 => base | 2,
                    2 => base,
                    3 => base | 4 | 32,
                    _ => base | 4,
                };
                put(st, *w, "F", Some(L::Integer(flags)));
            }
        }
        "textColor" | "fgColor" | "textSize" => {
            let da = inherited(st, node, "DA")
                .as_ref()
                .and_then(text_of)
                .and_then(|s| parse_da(&s));
            let font = da.map_or_else(|| f.text_font.clone(), |d| d.font_name);
            let color = color_ops(&f.text_color, false).unwrap_or_else(|| "0 g".into());
            let s = format!("/{font} {} Tf {color}", n(f.text_size as f32));
            put(st, node, "DA", Some(text_string(&s)));
        }
        "fillColor" | "bgColor" => mk_put(st, widgets, "BG", color_array(&f.fill_color)),
        "strokeColor" => mk_put(st, widgets, "BC", color_array(&f.stroke_color)),
        "rect" => {
            if let Some(w) = widgets.first() {
                let r = f.rect;
                let arr = r.iter().map(|v| L::Real(*v as f32)).collect();
                put(st, *w, "Rect", Some(L::Array(arr)));
            }
        }
        "userName" => put(st, node, "TU", Some(text_string(&f.tooltip))),
        "charLimit" => put(
            st,
            node,
            "MaxLen",
            (f.max_len > 0).then(|| L::Integer(i64::from(f.max_len))),
        ),
        "alignment" => put(st, node, "Q", Some(L::Integer(i64::from(f.alignment)))),
        "defaultValue" => put(st, node, "DV", Some(text_string(&f.default_value))),
        "items" => {
            let items = f
                .options
                .iter()
                .map(|(e, l)| {
                    if e == l {
                        text_string(l)
                    } else {
                        L::Array(vec![text_string(e), text_string(l)])
                    }
                })
                .collect();
            put(st, node, "Opt", Some(L::Array(items)));
        }
        "borderStyle" | "lineWidth" => {
            let s = match f.border_style.as_str() {
                "dashed" => "D",
                "beveled" => "B",
                "inset" => "I",
                "underline" => "U",
                _ => "S",
            };
            let mut bs = Dictionary::new();
            bs.set("S", L::Name(s.as_bytes().to_vec()));
            bs.set("W", L::Real(f.line_width as f32));
            for w in widgets {
                put(st, *w, "BS", Some(L::Dictionary(bs.clone())));
            }
        }
        "buttonCaption" => {
            if let Some(js::Prop::Str(s)) = f.extra.get("buttonCaption") {
                mk_put(st, widgets, "CA", Some(text_string(s)));
            }
        }
        _ => {}
    }
}

fn write_script(st: &mut State, node: ObjectId, widgets: &[ObjectId], f: &js::Field, trigger: &str) {
    let Some(code) = f.scripts.get(trigger) else { return };
    let mut act = Dictionary::new();
    act.set("S", L::Name(b"JavaScript".to_vec()));
    act.set("JS", text_string(code));
    if trigger == "A" {
        for w in widgets {
            put(st, *w, "A", Some(L::Dictionary(act.clone())));
        }
        return;
    }
    let mut aa = match dict_at(st, node)
        .and_then(|d| d.get(b"AA").ok())
        .and_then(|a| st.deref(a))
    {
        Some(L::Dictionary(a)) => a.clone(),
        _ => Dictionary::new(),
    };
    aa.set(trigger, L::Dictionary(act));
    put(st, node, "AA", Some(L::Dictionary(aa)));
}

/// Aplica al documento lo que el motor cambió.
fn apply(shared: &Shared, ids: &HashMap<String, ObjectId>, engine: &js::Engine, out: &js::Outcome) -> Result<()> {
    let form = engine.form();
    let mut redraw: Vec<ObjectId> = Vec::new();
    let mut props: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    let mut scripts: Vec<(&str, &str)> = Vec::new();
    let touch_redraw = |node: ObjectId, redraw: &mut Vec<ObjectId>| {
        if !redraw.contains(&node) {
            redraw.push(node);
        }
    };

    for c in &out.changes {
        match c {
            js::Change::Display { field, text } => {
                if let Some(node) = ids.get(field) {
                    let mut st = shared.borrow_mut();
                    match text {
                        Some(t) => st.js.display.insert(*node, t.clone()),
                        None => st.js.display.remove(node),
                    };
                    drop(st);
                    touch_redraw(*node, &mut redraw);
                }
            }
            js::Change::Prop { field, prop } => props.entry(field).or_default().push(prop),
            js::Change::Script { field, trigger } => scripts.push((field, trigger)),
            js::Change::Value { .. } => {}
        }
    }
    for field in out.changed_values() {
        let (Some(node), Some(f)) = (ids.get(field), form.field(field)) else {
            continue;
        };
        let widgets = leaves(&shared.borrow(), *node);
        store_value(shared, *node, &widgets, widget_type(f.kind), &f.value, &f.multi)?;
        touch_redraw(*node, &mut redraw);
    }
    for (field, list) in &props {
        let (Some(node), Some(f)) = (ids.get(*field), form.field(field)) else {
            continue;
        };
        let mut st = shared.borrow_mut();
        let widgets = leaves(&st, *node);
        for p in list {
            write_prop(&mut st, *node, &widgets, f, p);
        }
        drop(st);
        touch_redraw(*node, &mut redraw);
    }
    for (field, trigger) in scripts {
        let (Some(node), Some(f)) = (ids.get(field), form.field(field)) else {
            continue;
        };
        let mut st = shared.borrow_mut();
        let widgets = leaves(&st, *node);
        write_script(&mut st, *node, &widgets, f, trigger);
    }
    // Apariencias de texto, combinado, lista y botón. Las casillas y los radios cambian de estado con /AS.
    for node in redraw {
        let widgets = leaves(&shared.borrow(), node);
        let redraws = ids
            .iter()
            .find(|(_, id)| **id == node)
            .and_then(|(name, _)| form.field(name))
            .is_some_and(|f| {
                !matches!(
                    f.kind,
                    js::FieldKind::Checkbox | js::FieldKind::Radio | js::FieldKind::Signature
                )
            });
        if redraws {
            for w in widgets {
                rebuild_ap(shared, w)?;
            }
        }
    }
    Ok(())
}

fn log(shared: &Shared, out: &js::Outcome) {
    let mut st = shared.borrow_mut();
    st.js.log.extend(out.messages.iter().cloned());
    for e in &out.errors {
        st.js.log.push(JsMessage::Console(format!(
            "Error de script ({}{}): {}",
            e.field.as_deref().map_or(String::new(), |f| format!("campo «{f}», ")),
            e.trigger,
            e.message
        )));
    }
}

// --- Ciclo de vida -----------------------------------------------------------------------------------------------

/// Saca el motor del documento (lo reconstruye si el documento cambió por otra vía). `None` si el JavaScript no está
/// habilitado.
fn take_runtime(shared: &Shared) -> Option<Runtime> {
    let mut st = shared.borrow_mut();
    if !st.js.enabled {
        return None;
    }
    if let Some(rt) = st.js.runtime.take()
        && rt.generation == st.js.generation
    {
        return Some(rt);
    }
    let pages = tree::page_list(&mut st);
    let (form, ids) = build(&st, &pages);
    let engine = js::Engine::new(form, config());
    for e in engine.load_errors() {
        st.js.log.push(JsMessage::Console(format!(
            "Error en un script de documento ({}): {}",
            e.trigger, e.message
        )));
    }
    let generation = st.js.generation;
    Some(Runtime {
        engine,
        ids,
        generation,
    })
}

fn give_back(shared: &Shared, mut rt: Runtime) {
    let mut st = shared.borrow_mut();
    rt.generation = st.js.generation;
    st.js.runtime = Some(rt);
}

pub(super) fn enable(shared: &Shared) -> Result<()> {
    shared.borrow_mut().js.enabled = true;
    let Some(mut rt) = take_runtime(shared) else {
        return Ok(());
    };
    // Texto formateado de los campos que ya tienen valor: se dibuja con él, pero no se toca el PDF.
    let out = rt.engine.format_all();
    {
        let mut st = shared.borrow_mut();
        for c in &out.changes {
            if let js::Change::Display { field, text: Some(t) } = c
                && let Some(node) = rt.ids.get(field)
            {
                st.js.display.insert(*node, t.clone());
            }
        }
    }
    log(shared, &out);
    give_back(shared, rt);
    Ok(())
}

pub(super) fn disable(shared: &Shared) {
    let mut st = shared.borrow_mut();
    st.js.enabled = false;
    st.js.runtime = None;
    st.js.display.clear();
}

pub(super) fn calculate(shared: &Shared) -> Result<()> {
    let Some(mut rt) = take_runtime(shared) else {
        return Ok(());
    };
    let out = rt.engine.calculate();
    let r = apply(shared, &rt.ids, &rt.engine, &out);
    log(shared, &out);
    give_back(shared, rt);
    r
}

/// Confirma `value` en el campo del widget `widget_id`. `Some(aceptado)`, o `None` si el campo no es de los que
/// conoce el motor (sin nombre, tipo desconocido): entonces se usa la ruta sin scripts.
pub(super) fn commit(shared: &Shared, widget_id: ObjectId, value: &str) -> Result<Option<bool>> {
    let Some(mut rt) = take_runtime(shared) else {
        return Ok(None);
    };
    let (node, name) = {
        let st = shared.borrow();
        let node = field_node(&st, widget_id);
        (node, qualified_name(&st, node))
    };
    let Some(name) = name.filter(|n| rt.ids.get(n) == Some(&node)) else {
        give_back(shared, rt);
        return Ok(None);
    };
    let out = rt.engine.commit(&name, value);
    let r = if out.accepted {
        apply(shared, &rt.ids, &rt.engine, &out)
    } else {
        Ok(())
    };
    log(shared, &out);
    give_back(shared, rt);
    r?;
    Ok(Some(out.accepted))
}

/// Restablece el campo al valor por defecto ejecutando los scripts de cálculo y formato. `false` si el motor no
/// conoce el campo.
pub(super) fn reset(shared: &Shared, widget_id: ObjectId) -> Result<bool> {
    let Some(mut rt) = take_runtime(shared) else {
        return Ok(false);
    };
    let (node, name) = {
        let st = shared.borrow();
        let node = field_node(&st, widget_id);
        (node, qualified_name(&st, node))
    };
    let Some(name) = name.filter(|n| rt.ids.get(n) == Some(&node)) else {
        give_back(shared, rt);
        return Ok(false);
    };
    let out = rt.engine.reset(Some(std::slice::from_ref(&name)));
    let r = apply(shared, &rt.ids, &rt.engine, &out);
    log(shared, &out);
    give_back(shared, rt);
    r.map(|()| true)
}

/// Pulsación en curso en un campo (`/AA /K` con `willCommit` falso). `None` si el motor no está activo o no conoce el
/// campo; si no, `(aceptada, cambio filtrado)`.
pub(super) fn keystroke(
    shared: &Shared,
    widget_id: ObjectId,
    change: &str,
    current: &str,
    sel: (usize, usize),
) -> Result<Option<(bool, String)>> {
    let Some(mut rt) = take_runtime(shared) else {
        return Ok(None);
    };
    let (node, name) = {
        let st = shared.borrow();
        let node = field_node(&st, widget_id);
        (node, qualified_name(&st, node))
    };
    let Some(name) = name.filter(|n| rt.ids.get(n) == Some(&node)) else {
        give_back(shared, rt);
        return Ok(None);
    };
    let out = rt.engine.keystroke(&name, change, current, sel);
    log(shared, &out);
    give_back(shared, rt);
    Ok(Some((out.accepted, out.text.unwrap_or_else(|| change.to_string()))))
}

/// Ejecuta la acción `trigger` del campo (`U` ratón arriba, `D`, `E`, `X`, `Fo`, `Bl`; `A` es la del botón). `false`
/// si el motor no está activo o no conoce el campo.
pub(super) fn run_action(shared: &Shared, widget_id: ObjectId, trigger: &str) -> Result<bool> {
    let Some(mut rt) = take_runtime(shared) else {
        return Ok(false);
    };
    let (node, name) = {
        let st = shared.borrow();
        let node = field_node(&st, widget_id);
        (node, qualified_name(&st, node))
    };
    let Some(name) = name.filter(|n| rt.ids.get(n) == Some(&node)) else {
        give_back(shared, rt);
        return Ok(false);
    };
    let has_action = rt.engine.form().field(&name).is_some_and(|f| {
        let alt = if trigger == "U" { "A" } else { "U" };
        f.scripts.contains_key(trigger) || (trigger == "A" || trigger == "U") && f.scripts.contains_key(alt)
    });
    if !has_action {
        give_back(shared, rt);
        return Ok(false);
    }
    let out = rt.engine.run_trigger(&name, trigger);
    let r = apply(shared, &rt.ids, &rt.engine, &out);
    log(shared, &out);
    give_back(shared, rt);
    r.map(|()| true)
}

/// Claves de las acciones con script del campo (`K`, `V`, `C`, `F`, `U`, `A`...), ordenadas. Vacío si el motor no está
/// activo o no conoce el campo.
pub(super) fn script_triggers(shared: &Shared, widget_id: ObjectId) -> Vec<String> {
    let Some(rt) = take_runtime(shared) else {
        return Vec::new();
    };
    let name = {
        let st = shared.borrow();
        let node = field_node(&st, widget_id);
        qualified_name(&st, node).filter(|n| rt.ids.get(n) == Some(&node))
    };
    let mut keys: Vec<String> = name
        .and_then(|n| rt.engine.form().field(&n).map(|f| f.scripts.keys().cloned().collect()))
        .unwrap_or_default();
    keys.sort();
    give_back(shared, rt);
    keys
}

/// Eventos de documento y de página que pueden tener un script.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsDocEvent {
    /// `/OpenAction` del catálogo.
    Open,
    /// `/AA /WC`: el documento se va a cerrar.
    WillClose,
    /// `/AA /WS`: se va a guardar.
    WillSave,
    /// `/AA /DS`: se ha guardado.
    DidSave,
    /// `/AA /WP`: se va a imprimir.
    WillPrint,
    /// `/AA /DP`: se ha impreso.
    DidPrint,
    /// `/AA /O` de la página (desde 0): la página se muestra.
    PageOpen(usize),
    /// `/AA /C` de la página: la página se deja de mostrar.
    PageClose(usize),
}

impl JsDocEvent {
    fn name(self) -> &'static str {
        match self {
            JsDocEvent::Open => "Open",
            JsDocEvent::WillClose => "WillClose",
            JsDocEvent::WillSave => "WillSave",
            JsDocEvent::DidSave => "DidSave",
            JsDocEvent::WillPrint => "WillPrint",
            JsDocEvent::DidPrint => "DidPrint",
            JsDocEvent::PageOpen(_) => "PageOpen",
            JsDocEvent::PageClose(_) => "PageClose",
        }
    }
}

/// Código del script de un evento de documento o de página, si lo hay.
fn aa_script(st: &State, key: &str, holder: &Dictionary) -> Option<String> {
    let aa = match holder.get(b"AA").ok().and_then(|a| st.deref(a))? {
        L::Dictionary(a) => a,
        _ => return None,
    };
    action_js(st, aa.get(key.as_bytes()).ok()?)
}

fn doc_event_script(st: &mut State, ev: JsDocEvent) -> Option<String> {
    let page_id = match ev {
        JsDocEvent::PageOpen(i) | JsDocEvent::PageClose(i) => Some(*tree::page_list(st).get(i)?),
        _ => None,
    };
    let st = &*st;
    match ev {
        JsDocEvent::Open => {
            let cat = super::document::catalog_dict(st)?;
            action_js(st, cat.get(b"OpenAction").ok()?)
        }
        JsDocEvent::PageOpen(_) => aa_script(st, "O", dict_at(st, page_id?)?),
        JsDocEvent::PageClose(_) => aa_script(st, "C", dict_at(st, page_id?)?),
        other => {
            let key = match other {
                JsDocEvent::WillClose => "WC",
                JsDocEvent::WillSave => "WS",
                JsDocEvent::DidSave => "DS",
                JsDocEvent::WillPrint => "WP",
                _ => "DP",
            };
            aa_script(st, key, super::document::catalog_dict(st)?)
        }
    }
}

/// Ejecuta el script de un evento de documento o de página. `false` si el motor no está activo o no hay script.
pub(super) fn run_document_event(shared: &Shared, ev: JsDocEvent) -> Result<bool> {
    if !shared.borrow().js.enabled {
        return Ok(false);
    }
    let Some(code) = doc_event_script(&mut shared.borrow_mut(), ev) else {
        return Ok(false);
    };
    let Some(mut rt) = take_runtime(shared) else {
        return Ok(false);
    };
    let out = rt.engine.run_document_script(&code, ev.name());
    let r = apply(shared, &rt.ids, &rt.engine, &out);
    log(shared, &out);
    give_back(shared, rt);
    r.map(|()| true)
}
