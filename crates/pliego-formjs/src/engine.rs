//! El motor: un contexto de Boa con el modelo de objetos de Acrobat (`prelude.js`) y un [`Form`] en memoria.
//!
//! Los scripts leen y escriben los campos a través de `__h(op, ...)` (una sola función nativa); cada escritura
//! queda anotada como [`Change`] para que quien use el motor la aplique al documento.

use std::cell::{Cell, Ref, RefCell};
use std::rc::Rc;

use boa_engine::object::builtins::JsArray;
use boa_engine::{Context, JsError, JsNativeError, JsResult, JsString, JsValue, NativeFunction, Source, js_string};

use crate::format::{self, DateParts, Language, PrintfArg};
use crate::model::{Change, Color, FieldKind, Form, Message, Outcome, Prop, ScriptError, flags};

const PRELUDE: &str = include_str!("prelude.js");

/// Ajustes del motor.
#[derive(Debug, Clone)]
pub struct Config {
    /// Idioma de los nombres de mes y día y de los mensajes de validación.
    pub language: Language,
    /// Reloj fijo en milisegundos desde 1970 (pruebas). `None`: el del sistema.
    pub now_ms: Option<f64>,
    /// Iteraciones máximas de un bucle antes de abortar el script (evita `while (true)`).
    pub loop_limit: u64,
    /// Profundidad máxima de llamadas.
    pub recursion_limit: usize,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            language: Language::Es,
            now_ms: None,
            loop_limit: 5_000_000,
            recursion_limit: 400,
        }
    }
}

struct Host {
    form: RefCell<Form>,
    cfg: Config,
    changes: RefCell<Vec<Change>>,
    messages: RefCell<Vec<Message>>,
    errors: RefCell<Vec<ScriptError>>,
    /// Campos cuyo valor cambió y aún no se han vuelto a formatear.
    touched: RefCell<Vec<String>>,
    calc_requested: Cell<bool>,
}

impl Host {
    fn touch(&self, name: &str) {
        let mut t = self.touched.borrow_mut();
        if !t.iter().any(|n| n == name) {
            t.push(name.to_string());
        }
    }

    fn prop_changed(&self, field: &str, prop: &str) {
        let mut c = self.changes.borrow_mut();
        let ch = Change::Prop {
            field: field.into(),
            prop: prop.into(),
        };
        if !c.contains(&ch) {
            c.push(ch);
        }
    }

    /// Nombres que cuelgan de `prefix` (`prefix.hijo`).
    fn children(&self, prefix: &str) -> Vec<String> {
        let p = format!("{prefix}.");
        self.form
            .borrow()
            .fields
            .iter()
            .filter(|f| f.name.starts_with(&p))
            .map(|f| f.name.clone())
            .collect()
    }

    /// El campo con ese nombre exacto, o los hijos si es un nodo intermedio del árbol.
    fn targets(&self, name: &str) -> Vec<String> {
        if self.form.borrow().field(name).is_some() {
            vec![name.to_string()]
        } else {
            self.children(name)
        }
    }

    /// Fija el valor de un campo (texto ya convertido). Devuelve `false` si el campo no lo admite.
    fn set_value(&self, name: &str, value: String, multi: Vec<String>) -> bool {
        {
            let mut form = self.form.borrow_mut();
            let Some(f) = form.field_mut(name) else { return false };
            match f.kind {
                FieldKind::Button | FieldKind::Signature => return false,
                FieldKind::Checkbox | FieldKind::Radio => {
                    let off = value.is_empty() || value == "Off";
                    if !off && !f.export_values.is_empty() && !f.export_values.contains(&value) {
                        return false;
                    }
                    f.value = if off { "Off".into() } else { value };
                }
                _ => {
                    f.value = value;
                    f.multi = multi;
                }
            }
        }
        self.changes.borrow_mut().push(Change::Value { field: name.into() });
        self.touch(name);
        true
    }
}

// --- Conversiones ------------------------------------------------------------------------------------------------

fn arg_str(args: &[JsValue], i: usize, ctx: &mut Context) -> JsResult<String> {
    match args.get(i) {
        Some(v) if !v.is_undefined() && !v.is_null() => Ok(v.to_string(ctx)?.to_std_string_lossy()),
        _ => Ok(String::new()),
    }
}

fn arg_num(args: &[JsValue], i: usize, ctx: &mut Context) -> JsResult<f64> {
    match args.get(i) {
        Some(v) => v.to_number(ctx),
        None => Ok(0.0),
    }
}

fn arg_bool(args: &[JsValue], i: usize) -> bool {
    args.get(i).is_some_and(JsValue::to_boolean)
}

fn js_str(s: &str) -> JsValue {
    JsValue::from(JsString::from(s))
}

fn js_array(items: Vec<JsValue>, ctx: &mut Context) -> JsValue {
    JsValue::from(JsArray::from_iter(items, ctx))
}

fn list(v: &JsValue, ctx: &mut Context) -> JsResult<Vec<JsValue>> {
    let Some(o) = v.as_object() else { return Ok(Vec::new()) };
    let a = JsArray::from_object(o)?;
    let n = a.length(ctx)?;
    (0..n).map(|i| a.at(i as i64, ctx)).collect()
}

/// Valor de JavaScript a texto de campo: números con hasta 15 cifras, `undefined`/`null` vacío.
fn to_text(v: &JsValue, ctx: &mut Context) -> JsResult<String> {
    if v.is_undefined() || v.is_null() {
        return Ok(String::new());
    }
    if let Some(n) = v.as_number() {
        return Ok(format::number_to_field_text(n));
    }
    Ok(v.to_string(ctx)?.to_std_string_lossy())
}

fn to_prop(v: &JsValue, ctx: &mut Context) -> JsResult<Prop> {
    Ok(if v.is_undefined() || v.is_null() {
        Prop::Null
    } else if let Some(b) = v.as_boolean() {
        Prop::Bool(b)
    } else if let Some(n) = v.as_number() {
        Prop::Num(n)
    } else if v.as_object().is_some_and(|o| o.is_array()) {
        Prop::List(list(v, ctx)?.iter().map(|x| to_prop(x, ctx)).collect::<JsResult<_>>()?)
    } else {
        Prop::Str(v.to_string(ctx)?.to_std_string_lossy())
    })
}

fn from_prop(p: &Prop, ctx: &mut Context) -> JsValue {
    match p {
        Prop::Null => JsValue::null(),
        Prop::Bool(b) => JsValue::from(*b),
        Prop::Num(n) => JsValue::from(*n),
        Prop::Str(s) => js_str(s),
        Prop::List(l) => {
            let items = l.iter().map(|x| from_prop(x, ctx)).collect();
            js_array(items, ctx)
        }
    }
}

fn color_to_js(c: &Color, ctx: &mut Context) -> JsValue {
    let items = match c {
        Color::Transparent => vec![js_str("T")],
        Color::Gray(g) => vec![js_str("G"), JsValue::from(*g)],
        Color::Rgb(r, g, b) => vec![js_str("RGB"), JsValue::from(*r), JsValue::from(*g), JsValue::from(*b)],
        Color::Cmyk(c, m, y, k) => vec![
            js_str("CMYK"),
            JsValue::from(*c),
            JsValue::from(*m),
            JsValue::from(*y),
            JsValue::from(*k),
        ],
    };
    js_array(items, ctx)
}

fn color_from_js(v: &JsValue, ctx: &mut Context) -> JsResult<Color> {
    let items = list(v, ctx)?;
    let tag = match items.first() {
        Some(t) => t.to_string(ctx)?.to_std_string_lossy(),
        None => return Ok(Color::Transparent),
    };
    let mut nums = Vec::new();
    for i in items.iter().skip(1) {
        nums.push(i.to_number(ctx)?.clamp(0.0, 1.0));
    }
    Ok(match (tag.as_str(), nums.as_slice()) {
        ("G", [g]) => Color::Gray(*g),
        ("RGB", [r, g, b]) => Color::Rgb(*r, *g, *b),
        ("CMYK", [c, m, y, k]) => Color::Cmyk(*c, *m, *y, *k),
        _ => Color::Transparent,
    })
}

// --- Propiedades de campo ----------------------------------------------------------------------------------------

const FLAG_PROPS: &[(&str, u32)] = &[
    ("readonly", flags::READ_ONLY),
    ("required", flags::REQUIRED),
    ("noExport", flags::NO_EXPORT),
    ("multiline", flags::MULTILINE),
    ("password", flags::PASSWORD),
    ("fileSelect", flags::FILE_SELECT),
    ("doNotSpellCheck", flags::DO_NOT_SPELL_CHECK),
    ("doNotScroll", flags::DO_NOT_SCROLL),
    ("comb", flags::COMB),
    ("richText", flags::RICH_TEXT),
    ("commitOnSelChange", flags::COMMIT_ON_SEL_CHANGE),
    ("editable", flags::EDIT),
    ("multipleSelection", flags::MULTI_SELECT),
    ("sort", flags::SORT),
    ("radiosInUnison", 1 << 25),
];

fn get_prop(host: &Host, name: &str, prop: &str, ctx: &mut Context) -> JsResult<JsValue> {
    let targets = host.targets(name);
    let Some(first) = targets.first() else {
        return Ok(JsValue::undefined());
    };
    let form = host.form.borrow();
    let Some(f) = form.field(first) else {
        return Ok(JsValue::undefined());
    };
    if let Some((_, bit)) = FLAG_PROPS.iter().find(|(p, _)| *p == prop) {
        return Ok(JsValue::from(f.has_flag(*bit)));
    }
    Ok(match prop {
        "name" => js_str(name),
        "type" => js_str(f.kind.js_name()),
        "value" => match f.kind {
            _ if !f.multi.is_empty() => {
                let items = f.multi.iter().map(|s| js_str(s)).collect();
                js_array(items, ctx)
            }
            FieldKind::Text | FieldKind::Combo if f.is_numeric() => match format::make_number(&f.value) {
                Some(n) if !f.value.trim().is_empty() => JsValue::from(n),
                _ => js_str(""),
            },
            FieldKind::Button | FieldKind::Signature => js_str(""),
            _ => js_str(&f.value),
        },
        "valueAsString" => js_str(&f.value),
        "defaultValue" => js_str(&f.default_value),
        "display" => JsValue::from(i32::from(f.display)),
        "hidden" => JsValue::from(f.display == 1),
        "textColor" | "fgColor" => color_to_js(&f.text_color, ctx),
        "fillColor" | "bgColor" => color_to_js(&f.fill_color, ctx),
        "strokeColor" => color_to_js(&f.stroke_color, ctx),
        "textSize" => JsValue::from(f.text_size),
        "textFont" => js_str(&f.text_font),
        "alignment" => js_str(match f.alignment {
            1 => "center",
            2 => "right",
            _ => "left",
        }),
        "charLimit" => JsValue::from(f64::from(f.max_len)),
        "userName" => js_str(&f.tooltip),
        "page" => JsValue::from(f64::from(f.page)),
        "rect" => {
            let r = f.rect;
            let items = [r[0], r[3], r[2], r[1]].iter().map(|v| JsValue::from(*v)).collect();
            js_array(items, ctx)
        }
        "borderStyle" => js_str(&f.border_style),
        "lineWidth" => JsValue::from(f.line_width),
        "numItems" => JsValue::from(f.options.len() as f64),
        "exportValues" => {
            let items = f.export_values.iter().map(|s| js_str(s)).collect();
            js_array(items, ctx)
        }
        "currentValueIndices" => {
            let sel: Vec<&String> = if f.multi.is_empty() {
                vec![&f.value]
            } else {
                f.multi.iter().collect()
            };
            let idx: Vec<JsValue> = sel
                .iter()
                .filter_map(|v| f.options.iter().position(|(e, _)| e == *v))
                .map(|i| JsValue::from(i as f64))
                .collect();
            if f.multi.is_empty() {
                idx.first().cloned().unwrap_or_else(|| JsValue::from(-1))
            } else {
                js_array(idx, ctx)
            }
        }
        "calcOrderIndex" => JsValue::from(
            form.calc_order
                .iter()
                .position(|n| n == &f.name)
                .map_or(-1.0, |i| i as f64),
        ),
        "doc" | "source" => JsValue::undefined(),
        other => match f.extra.get(other) {
            Some(p) => from_prop(p, ctx),
            None => JsValue::undefined(),
        },
    })
}

fn set_prop(host: &Host, name: &str, prop: &str, v: &JsValue, ctx: &mut Context) -> JsResult<()> {
    let targets = host.targets(name);
    if prop == "value" {
        let (text, multi) = if v.as_object().is_some_and(|o| o.is_array()) {
            let items: Vec<String> = list(v, ctx)?.iter().map(|x| to_text(x, ctx)).collect::<JsResult<_>>()?;
            (
                items.first().cloned().unwrap_or_default(),
                if items.len() > 1 { items } else { Vec::new() },
            )
        } else {
            (to_text(v, ctx)?, Vec::new())
        };
        for t in targets {
            host.set_value(&t, text.clone(), multi.clone());
        }
        return Ok(());
    }
    for t in targets {
        let mut changed = true;
        {
            let mut form = host.form.borrow_mut();
            let Some(f) = form.field_mut(&t) else { continue };
            if let Some((_, bit)) = FLAG_PROPS.iter().find(|(p, _)| *p == prop) {
                let on = v.to_boolean();
                if f.has_flag(*bit) == on {
                    changed = false;
                }
                if on {
                    f.flags |= *bit;
                } else {
                    f.flags &= !*bit;
                }
            } else {
                match prop {
                    "defaultValue" => f.default_value = to_text(v, ctx)?,
                    "display" => f.display = (v.to_number(ctx)? as i64).clamp(0, 3) as u8,
                    "hidden" => f.display = u8::from(v.to_boolean()),
                    "textColor" | "fgColor" => f.text_color = color_from_js(v, ctx)?,
                    "fillColor" | "bgColor" => f.fill_color = color_from_js(v, ctx)?,
                    "strokeColor" => f.stroke_color = color_from_js(v, ctx)?,
                    "textSize" => f.text_size = v.to_number(ctx)?.max(0.0),
                    "textFont" => f.text_font = to_text(v, ctx)?,
                    "alignment" => {
                        f.alignment = match to_text(v, ctx)?.as_str() {
                            "center" => 1,
                            "right" => 2,
                            _ => 0,
                        }
                    }
                    "charLimit" => f.max_len = v.to_number(ctx)?.max(0.0) as u32,
                    "userName" => f.tooltip = to_text(v, ctx)?,
                    "rect" => {
                        let r = list(v, ctx)?;
                        if r.len() == 4 {
                            let n: Vec<f64> = r.iter().map(|x| x.to_number(ctx)).collect::<JsResult<_>>()?;
                            f.rect = [n[0].min(n[2]), n[3].min(n[1]), n[0].max(n[2]), n[3].max(n[1])];
                        }
                    }
                    "borderStyle" => f.border_style = to_text(v, ctx)?,
                    "lineWidth" => f.line_width = v.to_number(ctx)?.max(0.0),
                    "exportValues" => {
                        f.export_values = list(v, ctx)?.iter().map(|x| to_text(x, ctx)).collect::<JsResult<_>>()?;
                    }
                    "currentValueIndices" => {
                        let idx: Vec<usize> = if v.as_object().is_some_and(|o| o.is_array()) {
                            list(v, ctx)?
                                .iter()
                                .map(|x| x.to_number(ctx).map(|n| n as usize))
                                .collect::<JsResult<_>>()?
                        } else {
                            vec![v.to_number(ctx)?.max(0.0) as usize]
                        };
                        let picked: Vec<String> = idx
                            .iter()
                            .filter_map(|i| f.options.get(*i).map(|(e, _)| e.clone()))
                            .collect();
                        f.value = picked.first().cloned().unwrap_or_default();
                        f.multi = if picked.len() > 1 { picked } else { Vec::new() };
                        host.changes.borrow_mut().push(Change::Value { field: t.clone() });
                        host.touch(&t);
                    }
                    "name" | "type" | "valueAsString" | "numItems" | "page" | "calcOrderIndex" | "doc" | "source" => {
                        changed = false;
                    }
                    other => {
                        f.extra.insert(other.to_string(), to_prop(v, ctx)?);
                    }
                }
            }
        }
        if changed {
            host.prop_changed(&t, prop);
        }
    }
    Ok(())
}

fn items_op(host: &Host, name: &str, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let op = arg_str(args, 2, ctx)?;
    let mut form = host.form.borrow_mut();
    let Some(f) = form.field_mut(name) else {
        return Ok(JsValue::undefined());
    };
    if !matches!(f.kind, FieldKind::Combo | FieldKind::List) {
        return Ok(JsValue::undefined());
    }
    let mut changed = true;
    let out = match op.as_str() {
        "clear" => {
            f.options.clear();
            f.value.clear();
            f.multi.clear();
            JsValue::undefined()
        }
        "insert" => {
            let label = arg_str(args, 3, ctx)?;
            let exp = arg_str(args, 4, ctx)?;
            let at = arg_num(args, 5, ctx)?;
            let pos = if at < 0.0 || at as usize >= f.options.len() {
                f.options.len()
            } else {
                at as usize
            };
            f.options.insert(pos, (exp, label));
            JsValue::undefined()
        }
        "delete" => {
            let at = arg_num(args, 3, ctx)?;
            if at < 0.0 {
                f.options.pop();
            } else if (at as usize) < f.options.len() {
                f.options.remove(at as usize);
            }
            JsValue::undefined()
        }
        "get" => {
            changed = false;
            let at = arg_num(args, 3, ctx)?.max(0.0) as usize;
            let want_export = arg_bool(args, 4);
            match f.options.get(at) {
                Some((e, l)) => js_str(if want_export { e } else { l }),
                None => JsValue::null(),
            }
        }
        "set" => {
            let items = list(args.get(3).unwrap_or(&JsValue::undefined()), ctx)?;
            f.options.clear();
            for it in items {
                if it.as_object().is_some_and(|o| o.is_array()) {
                    let pair = list(&it, ctx)?;
                    let label = pair.first().map(|x| to_text(x, ctx)).transpose()?.unwrap_or_default();
                    let exp = pair
                        .get(1)
                        .map(|x| to_text(x, ctx))
                        .transpose()?
                        .unwrap_or_else(|| label.clone());
                    f.options.push((exp, label));
                } else {
                    let label = to_text(&it, ctx)?;
                    f.options.push((label.clone(), label));
                }
            }
            JsValue::undefined()
        }
        _ => {
            changed = false;
            JsValue::undefined()
        }
    };
    drop(form);
    if changed {
        host.prop_changed(name, "items");
    }
    Ok(out)
}

fn trigger_key(name: &str) -> String {
    match name.to_ascii_lowercase().replace([' ', '_', '-'], "").as_str() {
        "mouseup" => "U",
        "mousedown" => "D",
        "mouseenter" => "E",
        "mouseexit" => "X",
        "onfocus" | "focus" => "Fo",
        "onblur" | "blur" => "Bl",
        "keystroke" => "K",
        "validate" => "V",
        "calculate" => "C",
        "format" => "F",
        _ => name,
    }
    .to_string()
}

// --- La función nativa -------------------------------------------------------------------------------------------

fn host_of(ctx: &Context) -> JsResult<Rc<Host>> {
    ctx.get_data::<Rc<Host>>()
        .cloned()
        .ok_or_else(|| JsNativeError::error().with_message("motor sin formulario").into())
}

fn host_call(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let host = host_of(ctx)?;
    let op = arg_str(args, 0, ctx)?;
    let lang = host.cfg.language;
    match op.as_str() {
        "has" => {
            let n = arg_str(args, 1, ctx)?;
            Ok(JsValue::from(!host.targets(&n).is_empty()))
        }
        "names" => {
            let names: Vec<JsValue> = host.form.borrow().fields.iter().map(|f| js_str(&f.name)).collect();
            Ok(js_array(names, ctx))
        }
        "children" => {
            let n = arg_str(args, 1, ctx)?;
            let exact = host.form.borrow().field(&n).is_some();
            let names: Vec<JsValue> = if exact {
                Vec::new()
            } else {
                host.children(&n).iter().map(|s| js_str(s)).collect()
            };
            Ok(js_array(names, ctx))
        }
        "get" => {
            let (n, p) = (arg_str(args, 1, ctx)?, arg_str(args, 2, ctx)?);
            get_prop(&host, &n, &p, ctx)
        }
        "set" => {
            let (n, p) = (arg_str(args, 1, ctx)?, arg_str(args, 2, ctx)?);
            let v = args.get(3).cloned().unwrap_or_default();
            set_prop(&host, &n, &p, &v, ctx)?;
            Ok(JsValue::undefined())
        }
        "box" => {
            let n = arg_str(args, 1, ctx)?;
            let i = arg_num(args, 2, ctx)?.max(0.0) as usize;
            let default = arg_str(args, 3, ctx)? == "default";
            let form = host.form.borrow();
            let Some(f) = form.field(&n) else {
                return Ok(JsValue::from(false));
            };
            let cur = if default { &f.default_value } else { &f.value };
            let on = match f.export_values.get(i) {
                Some(e) => cur == e,
                None => i == 0 && !cur.is_empty() && cur != "Off",
            };
            Ok(JsValue::from(on))
        }
        "check" => {
            let n = arg_str(args, 1, ctx)?;
            let i = arg_num(args, 2, ctx)?.max(0.0) as usize;
            let on = arg_bool(args, 3);
            let default = arg_bool(args, 4);
            let export = {
                let form = host.form.borrow();
                let Some(f) = form.field(&n) else {
                    return Ok(JsValue::undefined());
                };
                f.export_values.get(i).cloned().unwrap_or_else(|| "Yes".into())
            };
            if default {
                if let Some(f) = host.form.borrow_mut().field_mut(&n) {
                    f.default_value = if on { export } else { "Off".into() };
                }
            } else {
                host.set_value(&n, if on { export } else { "Off".into() }, Vec::new());
            }
            Ok(JsValue::undefined())
        }
        "items" => {
            let n = arg_str(args, 1, ctx)?;
            items_op(&host, &n, args, ctx)
        }
        "setaction" => {
            let (n, t, code) = (
                arg_str(args, 1, ctx)?,
                trigger_key(&arg_str(args, 2, ctx)?),
                arg_str(args, 3, ctx)?,
            );
            if let Some(f) = host.form.borrow_mut().field_mut(&n) {
                f.scripts.insert(t.clone(), code);
            }
            host.changes.borrow_mut().push(Change::Script { field: n, trigger: t });
            Ok(JsValue::undefined())
        }
        "reset" => {
            let all = args.get(1).is_none_or(JsValue::is_null);
            let wanted: Vec<String> = if all {
                host.form.borrow().fields.iter().map(|f| f.name.clone()).collect()
            } else {
                let mut v = Vec::new();
                for x in list(&args[1], ctx)? {
                    v.extend(host.targets(&to_text(&x, ctx)?));
                }
                v
            };
            for n in wanted {
                let (def, multi) = {
                    let form = host.form.borrow();
                    let Some(f) = form.field(&n) else { continue };
                    (f.default_value.clone(), Vec::<String>::new())
                };
                host.set_value(&n, def, multi);
            }
            Ok(JsValue::undefined())
        }
        "calcnow" => {
            host.calc_requested.set(true);
            Ok(JsValue::undefined())
        }
        "alert" => {
            let text = arg_str(args, 1, ctx)?;
            let icon = arg_num(args, 2, ctx)? as i64;
            let kind = arg_num(args, 3, ctx)? as i64;
            let title = arg_str(args, 4, ctx)?;
            host.messages.borrow_mut().push(Message::Alert {
                text,
                title,
                icon: icon.clamp(0, 3) as u8,
                kind: kind.clamp(0, 3) as u8,
            });
            // Sin interfaz no hay quien responda: «Aceptar»/«Sí».
            Ok(JsValue::from(if kind >= 2 { 4 } else { 1 }))
        }
        "console" => {
            let t = arg_str(args, 1, ctx)?;
            host.messages.borrow_mut().push(Message::Console(t));
            Ok(JsValue::undefined())
        }
        "blocked" => {
            let (action, detail) = (arg_str(args, 1, ctx)?, arg_str(args, 2, ctx)?);
            host.messages.borrow_mut().push(Message::Blocked { action, detail });
            Ok(JsValue::undefined())
        }
        "info" => {
            let k = arg_str(args, 1, ctx)?;
            let form = host.form.borrow();
            Ok(match k.as_str() {
                "pages" => JsValue::from(f64::from(form.page_count)),
                "file" => js_str(&form.file_name),
                "title" => js_str(&form.title),
                _ => JsValue::undefined(),
            })
        }
        "lang" => Ok(js_str(if lang == Language::Es { "ESP" } else { "ENU" })),
        "msg" => {
            let key = arg_str(args, 1, ctx)?;
            let a: Vec<String> = (2..args.len())
                .map(|i| arg_str(args, i, ctx))
                .collect::<JsResult<_>>()?;
            Ok(js_str(&message(lang, &key, &a)))
        }
        "num_text" => Ok(js_str(&format::number_to_field_text(arg_num(args, 1, ctx)?))),
        "fmt_number" => {
            let (v, d, s) = (arg_num(args, 1, ctx)?, arg_num(args, 2, ctx)?, arg_num(args, 3, ctx)?);
            Ok(js_str(&format::format_number(v, d.max(0.0) as usize, s as i64)))
        }
        "make_number" => match format::make_number(&arg_str(args, 1, ctx)?) {
            Some(n) => Ok(JsValue::from(n)),
            None => Ok(JsValue::null()),
        },
        "is_numeric" => Ok(JsValue::from(format::is_numeric_text(
            &arg_str(args, 1, ctx)?,
            &arg_str(args, 2, ctx)?,
        ))),
        "printf" => {
            let fmt = arg_str(args, 1, ctx)?;
            let mut a = Vec::new();
            for x in list(args.get(2).unwrap_or(&JsValue::undefined()), ctx)? {
                a.push(match x.as_number() {
                    Some(n) => PrintfArg::Num(n),
                    None => PrintfArg::Str(to_text(&x, ctx)?),
                });
            }
            Ok(js_str(&format::printf(&fmt, &a)))
        }
        "fmt_date" => {
            let pat = arg_str(args, 1, ctx)?;
            let n: Vec<f64> = (2..8).map(|i| arg_num(args, i, ctx)).collect::<JsResult<_>>()?;
            let p = DateParts {
                year: n[0] as i32,
                month: n[1].max(0.0) as u32 % 12,
                day: n[2].max(1.0) as u32,
                hour: n[3].max(0.0) as u32,
                minute: n[4].max(0.0) as u32,
                second: n[5].max(0.0) as u32,
            };
            Ok(js_str(&format::format_date(&pat, &p, lang)))
        }
        "parse_date" => {
            let (s, pat) = (arg_str(args, 1, ctx)?, arg_str(args, 2, ctx)?);
            match format::parse_date(&s, &pat, lang, host.cfg.now_ms) {
                Some(p) => {
                    let v = [
                        p.year,
                        p.month as i32,
                        p.day as i32,
                        p.hour as i32,
                        p.minute as i32,
                        p.second as i32,
                    ];
                    let items = v.iter().map(|x| JsValue::from(*x)).collect();
                    Ok(js_array(items, ctx))
                }
                None => Ok(JsValue::null()),
            }
        }
        "mask" => {
            let (mask, text) = (arg_str(args, 1, ctx)?, arg_str(args, 2, ctx)?);
            Ok(JsValue::from(format::mask_matches(&mask, &text, arg_bool(args, 3))))
        }
        "special" => {
            let (k, t) = (arg_num(args, 1, ctx)?, arg_str(args, 2, ctx)?);
            Ok(js_str(&format::special_format(k as i64, &t)))
        }
        _ => Ok(JsValue::undefined()),
    }
}

/// Mensajes de validación de las funciones AF*.
fn message(lang: Language, key: &str, a: &[String]) -> String {
    let arg = |i: usize| a.get(i).map_or("", String::as_str);
    match (lang, key) {
        (Language::Es, "num_invalid") => "El valor no es válido: debe ser un número.".into(),
        (Language::En, "num_invalid") => "Invalid value: must be a number.".into(),
        (Language::Es, "range_both") => format!(
            "El valor no es válido: debe ser mayor o igual que {} y menor o igual que {}.",
            arg(0),
            arg(1)
        ),
        (Language::En, "range_both") => format!(
            "Invalid value: must be greater than or equal to {} and less than or equal to {}.",
            arg(0),
            arg(1)
        ),
        (Language::Es, "range_min") => format!("El valor no es válido: debe ser mayor o igual que {}.", arg(0)),
        (Language::En, "range_min") => format!("Invalid value: must be greater than or equal to {}.", arg(0)),
        (Language::Es, "range_max") => format!("El valor no es válido: debe ser menor o igual que {}.", arg(0)),
        (Language::En, "range_max") => format!("Invalid value: must be less than or equal to {}.", arg(0)),
        (Language::Es, "date_invalid" | "time_invalid") => format!(
            "La fecha u hora no es válida: compruebe que existe. El campo [ {} ] debe tener el formato {}.",
            arg(0),
            arg(1)
        ),
        (Language::En, "date_invalid" | "time_invalid") => format!(
            "Invalid date/time: please ensure that the date/time exists. Field [ {} ] should match format {}.",
            arg(0),
            arg(1)
        ),
        (Language::Es, "special_invalid") => format!("El valor no es válido: debe tener la forma {}.", arg(0)),
        (Language::En, "special_invalid") => format!("Invalid value: must be of the form {}.", arg(0)),
        _ => key.to_string(),
    }
}

// --- El motor ----------------------------------------------------------------------------------------------------

/// Un formulario con su intérprete de JavaScript. No es `Send` (Boa usa `Rc`): se crea y se usa en un solo hilo.
pub struct Engine {
    ctx: Context,
    host: Rc<Host>,
    load_errors: Vec<ScriptError>,
}

/// Datos de un evento para `event.*`.
#[derive(Debug, Clone, Default)]
struct Ev {
    value: String,
    will_commit: bool,
    change: String,
    sel: (usize, usize),
}

struct RunResult {
    rc: bool,
    value: String,
    change: String,
}

impl Engine {
    /// Crea el motor y ejecuta los scripts de documento. Los errores de esos scripts quedan en
    /// [`Engine::load_errors`]; no impiden usarlo.
    pub fn new(form: Form, cfg: Config) -> Engine {
        let doc_scripts = form.doc_scripts.clone();
        let host = Rc::new(Host {
            form: RefCell::new(form),
            cfg: cfg.clone(),
            changes: RefCell::default(),
            messages: RefCell::default(),
            errors: RefCell::default(),
            touched: RefCell::default(),
            calc_requested: Cell::new(false),
        });
        let mut ctx = Context::default();
        {
            let limits = ctx.runtime_limits_mut();
            limits.set_loop_iteration_limit(cfg.loop_limit);
            limits.set_recursion_limit(cfg.recursion_limit);
        }
        ctx.insert_data(host.clone());
        let registered =
            ctx.register_global_builtin_callable(js_string!("__h"), 1, NativeFunction::from_fn_ptr(host_call));
        let loaded = registered.and_then(|()| ctx.eval(Source::from_bytes(PRELUDE)).map(|_| ()));
        let mut engine = Engine {
            ctx,
            host,
            load_errors: Vec::new(),
        };
        if let Err(e) = loaded {
            engine.load_errors.push(ScriptError {
                field: None,
                trigger: "prelude".into(),
                message: e.to_string(),
            });
            return engine;
        }
        for (name, code) in doc_scripts {
            if let Some(msg) = engine.global_eval(&code) {
                engine.load_errors.push(ScriptError {
                    field: None,
                    trigger: format!("doc:{name}"),
                    message: msg,
                });
            }
        }
        // Lo que los scripts de documento hayan escrito (mensajes, valores) es parte de la carga.
        engine.host.changes.borrow_mut().clear();
        engine
    }

    /// Errores al cargar los scripts de documento.
    pub fn load_errors(&self) -> &[ScriptError] {
        &self.load_errors
    }

    /// Estado actual del formulario (valores, textos de visualización y propiedades).
    pub fn form(&self) -> Ref<'_, Form> {
        self.host.form.borrow()
    }

    pub fn into_form(self) -> Form {
        self.host.form.borrow().clone()
    }

    fn global_eval(&mut self, code: &str) -> Option<String> {
        let f = self
            .ctx
            .global_object()
            .get(js_string!("__global_eval"), &mut self.ctx)
            .ok()?;
        let f = f.as_callable()?;
        match f.call(&JsValue::undefined(), &[js_str(code)], &mut self.ctx) {
            Ok(v) => {
                let s = v.to_string(&mut self.ctx).ok()?.to_std_string_lossy();
                (!s.is_empty()).then_some(s)
            }
            Err(e) => Some(e.to_string()),
        }
    }

    /// Ejecuta un script con `event` preparado. `None` si el motor no se pudo arrancar.
    fn run(&mut self, code: &str, field: Option<&str>, trigger: &str, ev: &Ev) -> Option<RunResult> {
        let f = self.ctx.global_object().get(js_string!("__run"), &mut self.ctx).ok()?;
        let f = f.as_callable()?;
        let args = [
            js_str(code),
            field.map_or_else(JsValue::null, js_str),
            js_str(trigger),
            js_str(&ev.value),
            JsValue::from(ev.will_commit),
            js_str(&ev.change),
            JsValue::from(ev.sel.0 as f64),
            JsValue::from(ev.sel.1 as f64),
            JsValue::from(false),
        ];
        let out = match f.call(&JsValue::undefined(), &args, &mut self.ctx) {
            Ok(v) => v,
            Err(e) => {
                self.host.errors.borrow_mut().push(ScriptError {
                    field: field.map(String::from),
                    trigger: trigger.into(),
                    message: js_error_text(&e),
                });
                return None;
            }
        };
        let items = list(&out, &mut self.ctx).ok()?;
        let rc = items.first().is_none_or(JsValue::to_boolean);
        let value = items
            .get(1)
            .map(|v| to_text(v, &mut self.ctx).unwrap_or_default())
            .unwrap_or_default();
        let change = items
            .get(2)
            .map(|v| to_text(v, &mut self.ctx).unwrap_or_default())
            .unwrap_or_default();
        let err = items
            .get(3)
            .map(|v| to_text(v, &mut self.ctx).unwrap_or_default())
            .unwrap_or_default();
        if !err.is_empty() {
            self.host.errors.borrow_mut().push(ScriptError {
                field: field.map(String::from),
                trigger: trigger.into(),
                message: err,
            });
        }
        Some(RunResult { rc, value, change })
    }

    fn script_of(&self, field: &str, key: &str) -> Option<String> {
        self.host.form.borrow().field(field)?.scripts.get(key).cloned()
    }

    /// Recalcula los campos con acción de cálculo, en el orden de cálculo.
    fn calc_pass(&mut self) {
        let order: Vec<String> = {
            let form = self.host.form.borrow();
            let mut order: Vec<String> = form
                .calc_order
                .iter()
                .filter(|n| form.field(n).is_some_and(|f| f.scripts.contains_key("C")))
                .cloned()
                .collect();
            for f in &form.fields {
                if f.scripts.contains_key("C") && !order.contains(&f.name) {
                    order.push(f.name.clone());
                }
            }
            order
        };
        for name in order {
            let Some(code) = self.script_of(&name, "C") else {
                continue;
            };
            let current = self
                .host
                .form
                .borrow()
                .field(&name)
                .map(|f| f.value.clone())
                .unwrap_or_default();
            let ev = Ev {
                value: current.clone(),
                ..Default::default()
            };
            if let Some(r) = self.run(&code, Some(&name), "C", &ev)
                && r.rc
                && r.value != current
            {
                self.host.set_value(&name, r.value, Vec::new());
            }
        }
    }

    /// Aplica el formato (`/AA /F`) a los campos tocados y guarda el texto que debe verse.
    fn format_pass(&mut self, extra: &[&str]) {
        let mut names: Vec<String> = std::mem::take(&mut *self.host.touched.borrow_mut());
        for e in extra {
            if !names.iter().any(|n| n == e) {
                names.push((*e).to_string());
            }
        }
        for name in names {
            let (code, value) = {
                let form = self.host.form.borrow();
                let Some(f) = form.field(&name) else { continue };
                (f.scripts.get("F").cloned(), f.value.clone())
            };
            let display = match code {
                Some(code) => {
                    let ev = Ev {
                        value,
                        ..Default::default()
                    };
                    self.run(&code, Some(&name), "F", &ev).filter(|r| r.rc).map(|r| r.value)
                }
                None => None,
            };
            if let Some(f) = self.host.form.borrow_mut().field_mut(&name) {
                f.display_text = display.clone();
            }
            self.host.changes.borrow_mut().push(Change::Display {
                field: name,
                text: display,
            });
        }
    }

    /// Cierra una operación: cálculo si hace falta, formato de lo tocado y recogida de cambios y avisos.
    fn finish(&mut self, mut out: Outcome, formatted: &[&str]) -> Outcome {
        if self.host.calc_requested.replace(false) || !self.host.touched.borrow().is_empty() {
            self.calc_pass();
        }
        self.format_pass(formatted);
        out.changes.append(&mut self.host.changes.borrow_mut());
        out.messages.append(&mut self.host.messages.borrow_mut());
        out.errors.append(&mut self.host.errors.borrow_mut());
        out
    }

    /// Descarta lo acumulado sin aplicarlo (una operación rechazada).
    fn abort(&mut self, mut out: Outcome) -> Outcome {
        self.host.touched.borrow_mut().clear();
        self.host.calc_requested.set(false);
        self.host.changes.borrow_mut().clear();
        out.accepted = false;
        out.messages.append(&mut self.host.messages.borrow_mut());
        out.errors.append(&mut self.host.errors.borrow_mut());
        out
    }

    /// El usuario confirma un valor: tecleo (`K` con `willCommit`), validación (`V`), valor nuevo, cálculo de los
    /// demás campos y formato (`F`). Si algo lo rechaza, no cambia nada y `accepted` es `false`.
    pub fn commit(&mut self, field: &str, value: &str) -> Outcome {
        let mut out = Outcome::accepted();
        let (kind, k, v) = {
            let form = self.host.form.borrow();
            let Some(f) = form.field(field) else {
                out.accepted = false;
                out.errors.push(ScriptError {
                    field: Some(field.into()),
                    trigger: "commit".into(),
                    message: format!("el campo «{field}» no existe"),
                });
                return out;
            };
            (f.kind, f.scripts.get("K").cloned(), f.scripts.get("V").cloned())
        };
        if matches!(kind, FieldKind::Button | FieldKind::Signature) {
            return self.abort(out);
        }
        let mut candidate = value.to_string();
        let ev = |value: &str| Ev {
            value: value.to_string(),
            will_commit: true,
            ..Default::default()
        };
        if let Some(code) = k {
            match self.run(&code, Some(field), "K", &ev(&candidate)) {
                Some(r) if !r.rc => return self.abort(out),
                Some(r) => candidate = r.value,
                None => {}
            }
        }
        if let Some(code) = v {
            match self.run(&code, Some(field), "V", &ev(&candidate)) {
                Some(r) if !r.rc => return self.abort(out),
                Some(r) => candidate = r.value,
                None => {}
            }
        }
        if !self.host.set_value(field, candidate, Vec::new()) {
            return self.abort(out);
        }
        out.accepted = true;
        self.finish(out, &[field])
    }

    /// Tecleo en curso (sin confirmar): ejecuta `K` con `event.change` y `event.value`. Devuelve en `text` el cambio
    /// ya filtrado (el script puede cambiarlo) y `accepted` en `false` si se rechaza la pulsación.
    pub fn keystroke(&mut self, field: &str, change: &str, current: &str, sel: (usize, usize)) -> Outcome {
        let mut out = Outcome::accepted();
        out.text = Some(change.to_string());
        let Some(code) = self.script_of(field, "K") else {
            return out;
        };
        let ev = Ev {
            value: current.to_string(),
            change: change.to_string(),
            sel,
            will_commit: false,
        };
        if let Some(r) = self.run(&code, Some(field), "K", &ev) {
            out.accepted = r.rc;
            out.text = Some(r.change);
        }
        self.host.touched.borrow_mut().clear();
        out.changes.append(&mut self.host.changes.borrow_mut());
        out.messages.append(&mut self.host.messages.borrow_mut());
        out.errors.append(&mut self.host.errors.borrow_mut());
        out
    }

    /// Recalcula todos los campos con acción de cálculo (`doc.calculateNow()`) y los formatea.
    pub fn calculate(&mut self) -> Outcome {
        self.calc_pass();
        self.finish(Outcome::accepted(), &[])
    }

    /// Da formato a todos los campos que tienen acción `F` (al abrir el documento, para dibujar el texto formateado).
    pub fn format_all(&mut self) -> Outcome {
        let names: Vec<String> = self
            .host
            .form
            .borrow()
            .fields
            .iter()
            .filter(|f| f.scripts.contains_key("F"))
            .map(|f| f.name.clone())
            .collect();
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        self.format_pass(&refs);
        self.finish(Outcome::accepted(), &[])
    }

    /// Ejecuta una acción del campo (`U` ratón arriba, `D`, `E`, `X`, `Fo`, `Bl`, `A` del botón...).
    pub fn run_trigger(&mut self, field: &str, trigger: &str) -> Outcome {
        let key = if trigger == "A" { "U" } else { trigger };
        let code = self.script_of(field, trigger).or_else(|| self.script_of(field, key));
        if let Some(code) = code {
            self.run(&code, Some(field), key, &Ev::default());
        }
        self.finish(Outcome::accepted(), &[])
    }

    /// Devuelve campos a su valor por defecto (`None`: todos) y recalcula.
    pub fn reset(&mut self, names: Option<&[String]>) -> Outcome {
        let targets: Vec<String> = match names {
            Some(n) => n.to_vec(),
            None => self.host.form.borrow().fields.iter().map(|f| f.name.clone()).collect(),
        };
        for n in targets {
            let def = self.host.form.borrow().field(&n).map(|f| f.default_value.clone());
            if let Some(d) = def {
                self.host.set_value(&n, d, Vec::new());
            }
        }
        self.finish(Outcome::accepted(), &[])
    }

    /// Evalúa código suelto en el ámbito global (consola de depuración, scripts del documento).
    pub fn eval(&mut self, code: &str) -> Result<String, String> {
        match self.ctx.eval(Source::from_bytes(code)) {
            Ok(v) => Ok(v
                .to_string(&mut self.ctx)
                .map(|s| s.to_std_string_lossy())
                .unwrap_or_default()),
            Err(e) => Err(js_error_text(&e)),
        }
    }

    /// Recoge cambios y avisos pendientes de `eval` (los scripts de consola pueden escribir campos).
    pub fn take_outcome(&mut self) -> Outcome {
        self.finish(Outcome::accepted(), &[])
    }
}

fn js_error_text(e: &JsError) -> String {
    e.to_string()
}

impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine")
            .field("fields", &self.host.form.borrow().fields.len())
            .finish()
    }
}
