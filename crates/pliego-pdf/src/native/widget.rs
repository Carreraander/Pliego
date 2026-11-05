//! Campos de formulario (AcroForm) sobre lopdf: leer, rellenar, restablecer, regenerar su apariencia, borrar y
//! añadir campos de firma. Misma API que [`crate::PdfWidget`] (capa sobre MuPDF).
//!
//! # Modelo
//!
//! Un campo es un diccionario con `/FT` y `/T`; sus widgets (lo que se ve en la página) son el propio campo si
//! está «fusionado» con su anotación, o sus `/Kids`. `/FT`, `/Ff`, `/V`, `/DV`, `/DA`, `/Q`, `/Opt` y `/MaxLen` se
//! heredan de los padres. `set_value` escribe en el nodo del campo (el primero con `/T`) y regenera la apariencia
//! de todos sus widgets.
//!
//! # Lo que no hay
//!
//! JavaScript: con `PdfDocument::enable_js` los scripts de tecleo, validación, cálculo y formato (`/AA`, `/CO`) los ejecuta
//! el motor de [`super::formjs`] (crate `pliego-formjs`); sin `enable_js`, o con `ignore_trigger_events`, no se ejecutan.
//! Rotación de campo (`/MK /R`) sí; campos «rich text» y XFA, no.

use lopdf::{Dictionary, Object as L, ObjectId, StringFormat};
use pliego_core::Result;

use super::annot::{PdfAnnotation, parse_da, write_form};
use super::appearance::Ap;
use super::document::{PdfDocument, decode_text, text_string};
use super::formap::{self, Border, FieldDraw, Kind};
use super::formjs;
use super::metrics::base_font_for;
use super::object::Object;
use super::page::PdfPage;
use super::{Shared, State, pdf_err};
use crate::annot::{AnnotationColor, PdfAnnotationType};
use crate::widget::{FieldFlags, WidgetType};

/// Campo de formulario. Se obtiene de [`PdfPage::widgets`] / [`PdfPage::load_widget`].
pub struct PdfWidget {
    annot: PdfAnnotation,
}

// --- Lectura del árbol de campos ---------------------------------------------------------------------------------

pub(super) fn dict_at(st: &State, id: ObjectId) -> Option<&Dictionary> {
    st.dict_of(st.get(id)?)
}

/// El nodo y sus ascendientes (padre, abuelo…), con límite contra ciclos.
pub(super) fn chain(st: &State, id: ObjectId) -> Vec<ObjectId> {
    let mut out = vec![st.resolve_id(id)];
    for _ in 0..32 {
        let Some(d) = dict_at(st, *out.last().unwrap_or(&id)) else {
            break;
        };
        match d.get(b"Parent") {
            Ok(L::Reference(p)) => {
                let p = st.resolve_id(*p);
                if out.contains(&p) {
                    break;
                }
                out.push(p);
            }
            _ => break,
        }
    }
    out
}

/// Valor de `key` en el nodo o el primer ascendiente que lo tenga (ya resuelto).
pub(super) fn inherited(st: &State, id: ObjectId, key: &str) -> Option<L> {
    chain(st, id).into_iter().find_map(|n| {
        let v = dict_at(st, n)?.get(key.as_bytes()).ok()?;
        st.deref(v).cloned()
    })
}

pub(super) fn text_of(o: &L) -> Option<String> {
    decode_text(o)
}

pub(super) fn name_bytes(o: &L) -> Option<&[u8]> {
    o.as_name().ok()
}

pub(super) fn floats(st: &State, o: &L) -> Vec<f32> {
    o.as_array()
        .map(|a| a.iter().filter_map(|x| st.deref(x)?.as_float().ok()).collect())
        .unwrap_or_default()
}

/// El nodo que es «el campo»: el primero (subiendo desde el widget) con `/T`.
pub(super) fn field_node(st: &State, id: ObjectId) -> ObjectId {
    chain(st, id)
        .into_iter()
        .find(|n| dict_at(st, *n).is_some_and(|d| d.has(b"T")))
        .unwrap_or_else(|| st.resolve_id(id))
}

/// Widgets (hojas) de un campo.
pub(super) fn leaves(st: &State, field: ObjectId) -> Vec<ObjectId> {
    fn walk(st: &State, id: ObjectId, out: &mut Vec<ObjectId>, depth: usize) {
        let Some(d) = dict_at(st, id) else { return };
        let kids: Vec<ObjectId> = match d.get(b"Kids").ok().and_then(|k| st.deref(k)) {
            Some(L::Array(a)) => a
                .iter()
                .filter_map(|k| {
                    if let L::Reference(r) = k {
                        Some(st.resolve_id(*r))
                    } else {
                        None
                    }
                })
                .collect(),
            _ => Vec::new(),
        };
        if kids.is_empty() || depth > 16 {
            out.push(id);
            return;
        }
        for k in kids {
            let is_widget =
                dict_at(st, k).is_some_and(|kd| matches!(kd.get(b"Subtype"), Ok(L::Name(n)) if n == b"Widget"));
            if is_widget && !dict_at(st, k).is_some_and(|kd| kd.has(b"Kids")) {
                out.push(k);
            } else {
                walk(st, k, out, depth + 1);
            }
        }
    }
    let mut out = Vec::new();
    walk(st, field, &mut out, 0);
    out
}

pub(super) fn qualified_name(st: &State, id: ObjectId) -> Option<String> {
    let mut parts: Vec<String> = chain(st, id)
        .into_iter()
        .filter_map(|n| {
            dict_at(st, n)?
                .get(b"T")
                .ok()
                .and_then(|t| st.deref(t))
                .and_then(text_of)
        })
        .collect();
    if parts.is_empty() {
        return None;
    }
    parts.reverse();
    Some(parts.join("."))
}

pub(super) fn put(st: &mut State, id: ObjectId, key: &str, v: Option<L>) {
    let id = st.resolve_id(id);
    if let Some(obj) = st.doc.objects.get_mut(&id) {
        let d = match obj {
            L::Dictionary(d) => d,
            L::Stream(s) => &mut s.dict,
            _ => return,
        };
        match v {
            Some(v) => d.set(key, v),
            None => {
                d.remove(key.as_bytes());
            }
        }
        st.touch(id);
    }
}

/// Estados de `/AP /N` (claves del diccionario) de un widget, sin `Off`.
pub(super) fn on_states(st: &State, id: ObjectId) -> Vec<String> {
    let Some(d) = dict_at(st, id) else { return Vec::new() };
    let Some(L::Dictionary(ap)) = d.get(b"AP").ok().and_then(|a| st.deref(a)) else {
        return Vec::new();
    };
    match ap.get(b"N").ok().and_then(|n| st.deref(n)) {
        Some(L::Dictionary(n)) => n
            .iter()
            .map(|(k, _)| String::from_utf8_lossy(k).into_owned())
            .filter(|k| k != "Off")
            .collect(),
        _ => Vec::new(),
    }
}

pub(super) fn has_state_dict(st: &State, id: ObjectId) -> bool {
    dict_at(st, id)
        .and_then(|d| d.get(b"AP").ok())
        .and_then(|a| st.deref(a))
        .and_then(|a| {
            if let L::Dictionary(d) = a {
                d.get(b"N").ok()
            } else {
                None
            }
        })
        .and_then(|n| st.deref(n))
        .is_some_and(|n| matches!(n, L::Dictionary(_)))
}

pub(super) fn acroform_dict(st: &State) -> Option<Dictionary> {
    let root = super::tree::catalog_id(st)?;
    let cat = dict_at(st, root)?;
    match cat.get(b"AcroForm").ok().and_then(|a| st.deref(a))? {
        L::Dictionary(d) => Some(d.clone()),
        _ => None,
    }
}

// --- PdfWidget ---------------------------------------------------------------------------------------------------

impl PdfWidget {
    pub(crate) fn new(annot: PdfAnnotation) -> Self {
        Self { annot }
    }

    fn shared(&self) -> Result<Shared> {
        self.annot
            .obj
            .st
            .clone()
            .ok_or_else(|| pdf_err("el campo no pertenece a ningún documento"))
    }

    fn id(&self) -> Result<ObjectId> {
        self.annot
            .obj
            .obj_id()
            .ok_or_else(|| pdf_err("el campo no es un objeto indirecto"))
    }

    /// Anotación `/Widget` del campo, en solo lectura (rectángulo, diccionario…). Para modificarla,
    /// [`PdfWidget::into_annotation`].
    pub fn annotation(&self) -> &PdfAnnotation {
        &self.annot
    }

    pub fn into_annotation(self) -> Result<PdfAnnotation> {
        Ok(self.annot)
    }

    pub fn xref(&self) -> Result<i32> {
        Ok(self.id()?.0 as i32)
    }

    fn flags_raw(&self) -> Result<i32> {
        let shared = self.shared()?;
        let st = shared.borrow();
        Ok(inherited(&st, self.id()?, "Ff")
            .and_then(|o| o.as_float().ok())
            .map_or(0, |f| f as i32))
    }

    pub fn r#type(&self) -> Result<WidgetType> {
        let shared = self.shared()?;
        let ft = {
            let st = shared.borrow();
            inherited(&st, self.id()?, "FT").and_then(|o| name_bytes(&o).map(<[u8]>::to_vec))
        };
        let ff = FieldFlags(self.flags_raw()?);
        Ok(match ft.as_deref() {
            Some(b"Tx") => WidgetType::Text,
            Some(b"Sig") => WidgetType::Signature,
            Some(b"Btn") if ff.contains(FieldFlags::PUSHBUTTON) => WidgetType::Button,
            Some(b"Btn") if ff.contains(FieldFlags::RADIO) => WidgetType::RadioButton,
            Some(b"Btn") => WidgetType::Checkbox,
            Some(b"Ch") if ff.contains(FieldFlags::COMBO) => WidgetType::Combobox,
            Some(b"Ch") => WidgetType::Listbox,
            _ => WidgetType::Unknown,
        })
    }

    /// Regenera la apariencia. `true` si cambió.
    pub fn update(&mut self) -> Result<bool> {
        let shared = self.shared()?;
        let id = self.id()?;
        match self.r#type()? {
            WidgetType::Text | WidgetType::Combobox | WidgetType::Listbox | WidgetType::Button => {
                rebuild_ap(&shared, id)
            }
            WidgetType::Checkbox | WidgetType::RadioButton => ensure_toggle_ap(&shared, id),
            _ => Ok(false),
        }
    }

    pub fn is_signed(&self) -> Result<bool> {
        let shared = self.shared()?;
        let st = shared.borrow();
        let id = self.id()?;
        let sig = inherited(&st, id, "FT").is_some_and(|o| name_bytes(&o) == Some(b"Sig"));
        Ok(sig && inherited(&st, id, "V").is_some_and(|v| !matches!(v, L::Null)))
    }

    pub fn is_readonly(&self) -> Result<bool> {
        Ok(self.flags_raw()? & 1 != 0)
    }

    /// Nombre calificado del campo (`persona.nombre`).
    pub fn name(&self) -> Result<Option<String>> {
        let shared = self.shared()?;
        let st = shared.borrow();
        Ok(qualified_name(&st, self.id()?))
    }

    /// Tipo de campo en texto: `"text"`, `"checkbox"`, `"radiobutton"`, `"combobox"`, `"listbox"`, `"button"`,
    /// `"signature"` (como la capa sobre MuPDF; no es el `/FT` crudo).
    pub fn field_type_name(&self) -> Result<Option<String>> {
        Ok(match self.r#type()? {
            WidgetType::Text => Some("text"),
            WidgetType::Checkbox => Some("checkbox"),
            WidgetType::RadioButton => Some("radiobutton"),
            WidgetType::Combobox => Some("combobox"),
            WidgetType::Listbox => Some("listbox"),
            WidgetType::Button => Some("button"),
            WidgetType::Signature => Some("signature"),
            WidgetType::Unknown => None,
        }
        .map(str::to_string))
    }

    pub fn field_flags(&self) -> Result<FieldFlags> {
        Ok(FieldFlags(self.flags_raw()?))
    }

    pub fn value(&self) -> Result<Option<String>> {
        let shared = self.shared()?;
        let st = shared.borrow();
        Ok(Some(match inherited(&st, self.id()?, "V") {
            Some(L::Array(a)) => a
                .first()
                .and_then(|x| st.deref(x))
                .and_then(text_of)
                .unwrap_or_default(),
            Some(o) => text_of(&o).unwrap_or_default(),
            None => String::new(),
        }))
    }

    /// Texto de ayuda (`/TU`); si no hay, el nombre del campo.
    pub fn label(&self) -> Result<Option<String>> {
        let shared = self.shared()?;
        {
            let st = shared.borrow();
            if let Some(tu) = inherited(&st, self.id()?, "TU").as_ref().and_then(text_of) {
                return Ok(Some(tu));
            }
        }
        self.name()
    }

    /// Fija el valor y regenera las apariencias del campo. Con JavaScript habilitado (`PdfDocument::enable_js`) y
    /// `ignore_trigger_events == false` pasa antes por el tecleo (`/AA /K`) y la validación (`/AA /V`), recalcula los
    /// campos con cálculo (`/AA /C`, en el orden de `/CO`) y da formato (`/AA /F`); un script puede rechazar el valor.
    /// Devuelve `false` si el campo no admite ese valor (botón de acción, firma, estado que la casilla o el radio no
    /// tienen) o si un script lo rechaza.
    pub fn set_value(&mut self, _doc: &mut PdfDocument, value: &str, ignore_trigger_events: bool) -> Result<bool> {
        let shared = self.shared()?;
        let id = self.id()?;
        if !ignore_trigger_events
            && shared.borrow().js.enabled
            && let Some(accepted) = formjs::commit(&shared, id, value)?
        {
            return Ok(accepted);
        }
        let ty = self.r#type()?;
        let (field, widgets) = {
            let st = shared.borrow();
            let f = field_node(&st, id);
            (f, leaves(&st, f))
        };
        let ok = store_value(&shared, field, &widgets, ty, value, &[])?;
        if ok {
            shared.borrow_mut().js.display.remove(&field);
            for w in widgets {
                if matches!(ty, WidgetType::Text | WidgetType::Combobox | WidgetType::Listbox) {
                    rebuild_ap(&shared, w)?;
                }
            }
        }
        Ok(ok)
    }

    /// Devuelve el campo a su valor por defecto (`/DV`, o vacío si no tiene).
    pub fn reset(&mut self, _doc: &mut PdfDocument) -> Result<()> {
        let shared = self.shared()?;
        let id = self.id()?;
        if shared.borrow().js.enabled && formjs::reset(&shared, id)? {
            return Ok(());
        }
        let ty = self.r#type()?;
        let (field, widgets, dv) = {
            let mut st = shared.borrow_mut();
            let f = field_node(&st, id);
            st.js.display.remove(&f);
            (f, leaves(&st, f), inherited(&st, f, "DV"))
        };
        match ty {
            WidgetType::Text | WidgetType::Combobox | WidgetType::Listbox => {
                {
                    let mut st = shared.borrow_mut();
                    put(&mut st, field, "V", dv);
                    put(&mut st, field, "I", None);
                }
                for w in widgets {
                    rebuild_ap(&shared, w)?;
                }
            }
            WidgetType::Checkbox | WidgetType::RadioButton => {
                let mut st = shared.borrow_mut();
                let dvname = dv
                    .as_ref()
                    .and_then(|d| name_bytes(d).map(|n| String::from_utf8_lossy(n).into_owned()));
                put(&mut st, field, "V", dvname.clone().map(|n| L::Name(n.into_bytes())));
                for w in widgets {
                    let state = dvname
                        .as_deref()
                        .filter(|n| on_states(&st, w).iter().any(|s| s == n))
                        .unwrap_or("Off")
                        .to_string();
                    put(&mut st, w, "AS", Some(L::Name(state.into_bytes())));
                }
            }
            _ => {}
        }
        Ok(())
    }
}

/// Escribe el valor de un campo (`/V`, y `/AS` de sus widgets si es casilla o radio) sin regenerar apariencias de texto.
/// `multi`: selección múltiple de una lista. `false` si el campo no admite ese valor.
pub(super) fn store_value(
    shared: &Shared,
    field: ObjectId,
    widgets: &[ObjectId],
    ty: WidgetType,
    value: &str,
    multi: &[String],
) -> Result<bool> {
    match ty {
        WidgetType::Text | WidgetType::Combobox | WidgetType::Listbox => {
            let mut st = shared.borrow_mut();
            let v = if multi.len() > 1 {
                L::Array(multi.iter().map(|m| text_string(m)).collect())
            } else {
                text_string(value)
            };
            put(&mut st, field, "V", Some(v));
            if ty == WidgetType::Listbox {
                put(&mut st, field, "I", None);
            }
            Ok(true)
        }
        WidgetType::Checkbox | WidgetType::RadioButton => {
            let off = value == "Off" || value.is_empty();
            {
                let st = shared.borrow();
                if !off
                    && widgets
                        .iter()
                        .all(|w| has_state_dict(&st, *w) && !on_states(&st, *w).iter().any(|s| s == value))
                {
                    return Ok(false);
                }
            }
            let mut st = shared.borrow_mut();
            let v = if off { "Off" } else { value };
            put(&mut st, field, "V", Some(L::Name(v.as_bytes().to_vec())));
            for w in widgets {
                let has = !off && on_states(&st, *w).iter().any(|s| s == value);
                let state = if has { value } else { "Off" };
                put(&mut st, *w, "AS", Some(L::Name(state.as_bytes().to_vec())));
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

// --- Apariencias -------------------------------------------------------------------------------------------------

fn border_of(st: &State, bs: Option<&Dictionary>) -> Border {
    match bs
        .and_then(|b| b.get(b"S").ok())
        .and_then(|s| st.deref(s))
        .and_then(|s| name_bytes(s))
    {
        Some(b"D") => Border::Dashed,
        Some(b"B") => Border::Beveled,
        Some(b"I") => Border::Inset,
        Some(b"U") => Border::Underline,
        _ => Border::Solid,
    }
}

struct Collected {
    ap: Ap,
    matrix: Option<[f32; 6]>,
    font_refs: Vec<(String, ObjectId)>,
}

fn collect_field_ap(st: &State, id: ObjectId) -> Option<Collected> {
    let d = dict_at(st, id)?;
    let rect = floats(st, d.get(b"Rect").ok().and_then(|r| st.deref(r))?);
    if rect.len() < 4 {
        return None;
    }
    let (w, h) = ((rect[2] - rect[0]).abs(), (rect[3] - rect[1]).abs());
    if w < 1.0 || h < 1.0 {
        return None;
    }
    let mk = match d.get(b"MK").ok().and_then(|m| st.deref(m)) {
        Some(L::Dictionary(m)) => Some(m.clone()),
        _ => None,
    };
    let mkf = |k: &str| {
        mk.as_ref()
            .and_then(|m| m.get(k.as_bytes()).ok())
            .map(|v| floats(st, st.deref(v).unwrap_or(v)))
    };
    let bg = mkf("BG").filter(|v| !v.is_empty());
    let bc = mkf("BC").filter(|v| !v.is_empty());
    let rotation = mk
        .as_ref()
        .and_then(|m| m.get(b"R").ok())
        .and_then(|r| r.as_float().ok())
        .map_or(0, |r| (r as i32).rem_euclid(360));
    let bs = match d.get(b"BS").ok().and_then(|b| st.deref(b)) {
        Some(L::Dictionary(b)) => Some(b.clone()),
        _ => None,
    };
    let bw = bs
        .as_ref()
        .and_then(|b| b.get(b"W").ok())
        .and_then(|w| w.as_float().ok())
        .unwrap_or(1.0);
    let border = border_of(st, bs.as_ref());

    let ft = inherited(st, id, "FT").and_then(|o| name_bytes(&o).map(<[u8]>::to_vec))?;
    let ff = inherited(st, id, "Ff")
        .and_then(|o| o.as_float().ok())
        .map_or(0, |f| f as i32);
    let ff = FieldFlags(ff);
    let kind = match ft.as_slice() {
        b"Tx" => Kind::Text,
        b"Ch" if ff.contains(FieldFlags::COMBO) => Kind::Combo,
        b"Ch" => Kind::List,
        b"Btn" if ff.contains(FieldFlags::PUSHBUTTON) => Kind::Push,
        _ => return None,
    };
    let acro = acroform_dict(st);
    let da_text = inherited(st, id, "DA").as_ref().and_then(text_of).or_else(|| {
        acro.as_ref()
            .and_then(|a| a.get(b"DA").ok())
            .and_then(|v| st.deref(v))
            .and_then(text_of)
    });
    let da = da_text.as_deref().and_then(parse_da);
    let (font_res, size, color) = match da {
        Some(d) => (
            d.font_name,
            d.size,
            d.color.map(|c| match c {
                AnnotationColor::Gray(g) => vec![g],
                AnnotationColor::Rgb { red, green, blue } => vec![red, green, blue],
                AnnotationColor::Cmyk {
                    cyan,
                    magenta,
                    yellow,
                    key,
                } => vec![cyan, magenta, yellow, key],
            }),
        ),
        None => ("Helv".to_string(), 0.0, None),
    };
    let quadding = inherited(st, id, "Q")
        .or_else(|| {
            acro.as_ref()
                .and_then(|a| a.get(b"Q").ok())
                .and_then(|v| st.deref(v))
                .cloned()
        })
        .and_then(|q| q.as_float().ok())
        .map_or(0, |q| q as i32);

    // Fuente: la de /DR del formulario si existe.
    let mut font_refs = Vec::new();
    let mut base_font = base_font_for(&font_res);
    if let Some(L::Dictionary(dr)) = acro.as_ref().and_then(|a| a.get(b"DR").ok()).and_then(|v| st.deref(v))
        && let Some(L::Dictionary(fonts)) = dr.get(b"Font").ok().and_then(|v| st.deref(v))
        && let Ok(L::Reference(fid)) = fonts.get(font_res.as_bytes())
    {
        font_refs.push((font_res.clone(), *fid));
        if let Some(fd) = dict_at(st, *fid)
            && let Some(bf) = fd.get(b"BaseFont").ok().and_then(|b| b.as_name().ok())
        {
            let bf = String::from_utf8_lossy(bf).into_owned();
            base_font = base_font_for(if bf.contains("Bold") && bf.contains("Helvetica") {
                "HeBo"
            } else if bf.starts_with("Times") {
                "TiRo"
            } else if bf.starts_with("Courier") {
                "Cour"
            } else {
                "Helv"
            });
        }
    }

    // Opciones y valor.
    let v = inherited(st, id, "V");
    let mut options: Vec<(String, String)> = Vec::new();
    if let Some(L::Array(opt)) = inherited(st, id, "Opt") {
        for o in &opt {
            match st.deref(o) {
                Some(L::Array(pair)) if pair.len() >= 2 => {
                    let e = st.deref(&pair[0]).and_then(text_of).unwrap_or_default();
                    let l = st.deref(&pair[1]).and_then(text_of).unwrap_or_default();
                    options.push((e, l));
                }
                Some(s) => {
                    let t = text_of(s).unwrap_or_default();
                    options.push((t.clone(), t));
                }
                None => {}
            }
        }
    }
    let selected: Vec<String> = match &v {
        Some(L::Array(a)) => a.iter().filter_map(|x| st.deref(x).and_then(text_of)).collect(),
        Some(o) => text_of(o).into_iter().collect(),
        None => Vec::new(),
    };
    let first = selected.first().cloned().unwrap_or_default();
    // Texto formateado por un script de formato (`/AA /F`): lo que se ve, distinto del valor guardado.
    let first = match (kind, st.js.display.get(&field_node(st, id))) {
        (Kind::Text, Some(shown)) => shown.clone(),
        _ => first,
    };
    let text = match kind {
        Kind::Push => mk
            .as_ref()
            .and_then(|m| m.get(b"CA").ok())
            .and_then(|c| st.deref(c))
            .and_then(text_of)
            .unwrap_or_default(),
        Kind::Combo => options
            .iter()
            .find(|(e, _)| *e == first)
            .map_or(first.clone(), |(_, l)| l.clone()),
        _ => first.clone(),
    };
    let top_index = inherited(st, id, "TI")
        .and_then(|t| t.as_float().ok())
        .map_or(0, |t| t.max(0.0) as usize);
    let maxlen = inherited(st, id, "MaxLen")
        .and_then(|m| m.as_float().ok())
        .map(|m| m as usize);
    let comb = if ff.contains(FieldFlags::COMB) { maxlen } else { None };

    let (cw, ch, matrix) = match rotation {
        90 => (h, w, Some([0.0, 1.0, -1.0, 0.0, w, 0.0])),
        180 => (w, h, Some([-1.0, 0.0, 0.0, -1.0, w, h])),
        270 => (h, w, Some([0.0, -1.0, 1.0, 0.0, 0.0, h])),
        _ => (w, h, None),
    };
    let draw = FieldDraw {
        kind,
        width: cw,
        height: ch,
        background: bg.as_deref(),
        border_color: bc.as_deref(),
        border_width: if bc.is_some() { bw } else { 0.0 },
        border,
        text: &text,
        font_res: &font_res,
        base_font,
        size,
        color: color.as_deref().unwrap_or(&[]),
        quadding,
        multiline: ff.contains(FieldFlags::MULTILINE),
        password: ff.contains(FieldFlags::PASSWORD),
        comb,
        options: &options,
        selected: &selected,
        top_index,
    };
    Some(Collected {
        ap: formap::draw_field(&draw),
        matrix,
        font_refs,
    })
}

/// `/AP /N` de un widget: id del flujo actual si lo hay.
fn current_n(st: &State, id: ObjectId) -> Option<ObjectId> {
    let d = dict_at(st, id)?;
    let ap = match d.get(b"AP").ok().and_then(|a| st.deref(a))? {
        L::Dictionary(a) => a,
        _ => return None,
    };
    match ap.get(b"N").ok()? {
        L::Reference(r) => Some(st.resolve_id(*r)).filter(|r| matches!(st.doc.objects.get(r), Some(L::Stream(_)))),
        _ => None,
    }
}

pub(super) fn rebuild_ap(shared: &Shared, id: ObjectId) -> Result<bool> {
    let (built, existing) = {
        let st = shared.borrow();
        (collect_field_ap(&st, id), current_n(&st, id))
    };
    let Some(c) = built else { return Ok(false) };
    let nid = write_form(shared, existing, &c.ap, c.matrix, &c.font_refs);
    let mut ap = Dictionary::new();
    ap.set("N", L::Reference(nid));
    put(&mut shared.borrow_mut(), id, "AP", Some(L::Dictionary(ap)));
    Ok(true)
}

/// Si una casilla o un radio no tiene estados en `/AP /N`, los crea (`Yes`/`Off`) para que se vea.
fn ensure_toggle_ap(shared: &Shared, id: ObjectId) -> Result<bool> {
    let (rect, radio, bg, bc, bw, on_name, checked) = {
        let st = shared.borrow();
        if has_state_dict(&st, id) {
            return Ok(false);
        }
        let Some(d) = dict_at(&st, id) else { return Ok(false) };
        let rect = d
            .get(b"Rect")
            .ok()
            .and_then(|r| st.deref(r))
            .map(|r| floats(&st, r))
            .unwrap_or_default();
        if rect.len() < 4 {
            return Ok(false);
        }
        let ff = inherited(&st, id, "Ff")
            .and_then(|o| o.as_float().ok())
            .map_or(0, |f| f as i32);
        let radio = FieldFlags(ff).contains(FieldFlags::RADIO);
        let mk = match d.get(b"MK").ok().and_then(|m| st.deref(m)) {
            Some(L::Dictionary(m)) => Some(m.clone()),
            _ => None,
        };
        let mkf = |k: &str| {
            mk.as_ref()
                .and_then(|m| m.get(k.as_bytes()).ok())
                .map(|v| floats(&st, st.deref(v).unwrap_or(v)))
        };
        let v = inherited(&st, id, "V").and_then(|v| name_bytes(&v).map(|n| String::from_utf8_lossy(n).into_owned()));
        let on = v.clone().filter(|v| v != "Off").unwrap_or_else(|| "Yes".into());
        (
            rect,
            radio,
            mkf("BG"),
            mkf("BC"),
            1.0f32,
            on,
            v.is_some_and(|v| v != "Off"),
        )
    };
    let (w, h) = ((rect[2] - rect[0]).abs(), (rect[3] - rect[1]).abs());
    let t = formap::ToggleDraw {
        radio,
        width: w,
        height: h,
        background: bg.as_deref(),
        border_color: bc.as_deref(),
        border_width: bw,
        color: &[0.0],
    };
    let on = formap::draw_toggle(&t, true);
    let off = formap::draw_toggle(&t, false);
    let on_id = write_form(shared, None, &on, None, &[]);
    let off_id = write_form(shared, None, &off, None, &[]);
    let mut n = Dictionary::new();
    n.set(on_name.as_str(), L::Reference(on_id));
    n.set("Off", L::Reference(off_id));
    let mut ap = Dictionary::new();
    ap.set("N", L::Dictionary(n));
    let mut st = shared.borrow_mut();
    put(&mut st, id, "AP", Some(L::Dictionary(ap)));
    let state = if checked { on_name } else { "Off".into() };
    put(&mut st, id, "AS", Some(L::Name(state.into_bytes())));
    Ok(true)
}

// --- Página ------------------------------------------------------------------------------------------------------

impl PdfPage {
    /// Campos de la página.
    pub fn widgets(&self) -> std::vec::IntoIter<PdfWidget> {
        let items = match self.page.get_dict("Annots") {
            Ok(Some(arr)) if arr.is_array().unwrap_or(false) => arr.array_iter().unwrap_or_default(),
            _ => Vec::new(),
        };
        items
            .into_iter()
            .filter(|o| o.obj_id().is_some())
            .map(|o| PdfAnnotation::new(o, self.page.clone()))
            .filter(|a| matches!(a.r#type(), Ok(PdfAnnotationType::Widget)))
            .map(PdfWidget::new)
            .collect::<Vec<_>>()
            .into_iter()
    }

    pub fn load_widget(&self, xref: i32) -> Result<Option<PdfWidget>> {
        Ok(self.widgets().find(|w| w.xref().is_ok_and(|x| x == xref)))
    }

    /// Quita el campo de la página y del árbol de campos (`/Fields` o `/Kids` del padre).
    pub fn delete_widget(&mut self, widget: PdfWidget) -> Result<()> {
        let shared = widget.shared()?;
        let id = widget.id()?;
        // Fuera de /Annots.
        if let Some(arr) = self.page.get_dict("Annots")?
            && arr.is_array()?
        {
            arr.modify(|o| {
                if let L::Array(a) = o {
                    a.retain(|x| *x != L::Reference(id));
                }
                Ok(())
            })?;
        }
        // Fuera del árbol: se sube mientras el padre se quede sin hijos.
        let mut st = shared.borrow_mut();
        let mut node = id;
        loop {
            let parent = match dict_at(&st, node).and_then(|d| d.get(b"Parent").ok()) {
                Some(L::Reference(p)) => Some(st.resolve_id(*p)),
                _ => None,
            };
            match parent {
                Some(p) => {
                    let mut empty = false;
                    if let Some(L::Dictionary(pd)) = st.doc.objects.get_mut(&p)
                        && let Ok(L::Array(kids)) = pd.get_mut(b"Kids")
                    {
                        kids.retain(|k| *k != L::Reference(node));
                        empty = kids.is_empty();
                    }
                    st.touch(p);
                    if !empty {
                        break;
                    }
                    node = p;
                }
                None => {
                    // Campo de primer nivel: sale de /AcroForm /Fields.
                    if let Some(root) = super::tree::catalog_id(&st)
                        && let Some(af) = dict_at(&st, root).and_then(|c| c.get(b"AcroForm").ok()).cloned()
                    {
                        let af_id = match af {
                            L::Reference(r) => Some(st.resolve_id(r)),
                            _ => None,
                        };
                        if let Some(af_id) = af_id {
                            if let Some(L::Dictionary(ad)) = st.doc.objects.get_mut(&af_id)
                                && let Ok(L::Array(fields)) = ad.get_mut(b"Fields")
                            {
                                fields.retain(|f| *f != L::Reference(node));
                            }
                            st.touch(af_id);
                        } else if let Some(L::Dictionary(cd)) = st.doc.objects.get_mut(&root)
                            && let Ok(L::Dictionary(ad)) = cd.get_mut(b"AcroForm")
                            && let Ok(L::Array(fields)) = ad.get_mut(b"Fields")
                        {
                            fields.retain(|f| *f != L::Reference(node));
                            st.touch(root);
                        }
                    }
                    break;
                }
            }
        }
        Ok(())
    }

    /// Añade un campo de firma vacío (sin rectángulo: colócalo con su anotación).
    pub fn add_signature_widget(&mut self, name: &str) -> Result<PdfWidget> {
        let shared = self
            .page
            .st
            .clone()
            .ok_or_else(|| pdf_err("la página no pertenece a ningún documento"))?;
        let mut d = Dictionary::new();
        d.set("Type", L::Name(b"Annot".to_vec()));
        d.set("Subtype", L::Name(b"Widget".to_vec()));
        d.set("FT", L::Name(b"Sig".to_vec()));
        d.set("T", L::String(name.as_bytes().to_vec(), StringFormat::Literal));
        d.set("F", L::Integer(4));
        d.set(
            "Rect",
            L::Array(vec![L::Integer(0), L::Integer(0), L::Integer(0), L::Integer(0)]),
        );
        if let Some(pid) = self.page.obj_id() {
            d.set("P", L::Reference(pid));
        }
        let id = shared.borrow_mut().add(L::Dictionary(d));
        {
            let mut p = self.page.clone();
            match p.get_dict("Annots")? {
                Some(arr) if arr.is_array()? => arr.modify(|o| {
                    if let L::Array(a) = o {
                        a.push(L::Reference(id));
                    }
                    Ok(())
                })?,
                _ => p.dict_put(
                    "Annots",
                    Object::free(Some(shared.clone()), L::Array(vec![L::Reference(id)])),
                )?,
            }
        }
        // AcroForm: se crea si falta y el campo entra en /Fields.
        let mut st = shared.borrow_mut();
        let root = super::tree::catalog_id(&st).ok_or_else(|| pdf_err("el documento no tiene catálogo"))?;
        let af = dict_at(&st, root).and_then(|c| c.get(b"AcroForm").ok()).cloned();
        match af {
            Some(L::Reference(r)) => {
                let r = st.resolve_id(r);
                if let Some(L::Dictionary(ad)) = st.doc.objects.get_mut(&r) {
                    match ad.get_mut(b"Fields") {
                        Ok(L::Array(f)) => f.push(L::Reference(id)),
                        _ => ad.set("Fields", L::Array(vec![L::Reference(id)])),
                    }
                }
                st.touch(r);
            }
            Some(L::Dictionary(_)) => {
                if let Some(L::Dictionary(cd)) = st.doc.objects.get_mut(&root)
                    && let Ok(L::Dictionary(ad)) = cd.get_mut(b"AcroForm")
                {
                    match ad.get_mut(b"Fields") {
                        Ok(L::Array(f)) => f.push(L::Reference(id)),
                        _ => ad.set("Fields", L::Array(vec![L::Reference(id)])),
                    }
                }
                st.touch(root);
            }
            _ => {
                let mut ad = Dictionary::new();
                ad.set("Fields", L::Array(vec![L::Reference(id)]));
                let af_id = st.add(L::Dictionary(ad));
                if let Some(L::Dictionary(cd)) = st.doc.objects.get_mut(&root) {
                    cd.set("AcroForm", L::Reference(af_id));
                }
                st.touch(root);
            }
        }
        drop(st);
        Ok(PdfWidget::new(PdfAnnotation::new(
            Object::reference(&shared, id),
            self.page.clone(),
        )))
    }
}
