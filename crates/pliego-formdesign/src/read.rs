use mupdf::pdf::{PdfDocument, PdfObject, PdfWidget, WidgetType};
use pliego_core::Result;

use crate::objects::*;
use crate::pe;
use crate::spec::{FieldSpec, FieldType};

fn kind_of(t: WidgetType) -> Option<FieldType> {
    Some(match t {
        WidgetType::Text => FieldType::Text,
        WidgetType::Checkbox => FieldType::CheckBox,
        WidgetType::RadioButton => FieldType::RadioButton,
        WidgetType::Combobox => FieldType::ComboBox,
        WidgetType::Listbox => FieldType::ListBox,
        WidgetType::Button => FieldType::PushButton,
        WidgetType::Signature => FieldType::Signature,
        WidgetType::Unknown => return None,
    })
}

fn text_of(o: &PdfObject) -> Option<String> {
    if o.is_string().ok()? {
        o.as_string().ok()
    } else if o.is_name().ok()? {
        o.as_name().ok().map(|n| String::from_utf8_lossy(&n).into_owned())
    } else {
        None
    }
}

/// Estado «marcado» de un widget de casilla/radio: la clave de `/AP /N` que no es `Off`.
pub fn on_state(widget: &PdfObject) -> Option<String> {
    let n = widget.get_dict("AP").ok().flatten()?.get_dict("N").ok().flatten()?;
    if !n.is_dict().ok()? {
        return None;
    }
    for i in 0..n.dict_len().ok()? {
        if let Ok(Some(k)) = n.get_dict_key(i as i32)
            && let Ok(name) = k.as_name()
        {
            let name = String::from_utf8_lossy(&name).into_owned();
            if name != "Off" {
                return Some(name);
            }
        }
    }
    None
}

pub fn options_of(field: &PdfObject) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(Some(opt)) = field.get_dict_inheritable("Opt")
        && opt.is_array().unwrap_or(false)
    {
        for i in 0..opt.len().unwrap_or(0) {
            if let Ok(Some(item)) = opt.get_array(i as i32) {
                let s = if item.is_array().unwrap_or(false) {
                    item.get_array(0).ok().flatten().and_then(|f| text_of(&f))
                } else {
                    text_of(&item)
                };
                out.extend(s);
            }
        }
    }
    out
}

/// Widgets de una página con su nombre completo.
pub fn page_widgets(doc: &PdfDocument, page: i32) -> Result<Vec<(String, PdfWidget)>> {
    let p = doc.load_pdf_page(page).map_err(pe)?;
    let mut out = Vec::new();
    for w in p.widgets() {
        if let Some(name) = w.name().map_err(pe)? {
            out.push((name, w));
        }
    }
    Ok(out)
}

/// Lista los campos del formulario como `FieldSpec` (un elemento por nombre; los radios, uno por botón).
pub fn list_fields(doc: &PdfDocument) -> Result<Vec<FieldSpec>> {
    let mut out: Vec<FieldSpec> = Vec::new();
    for p in 0..doc.page_count().map_err(pe)? {
        for (name, w) in page_widgets(doc, p)? {
            let Some(kind) = kind_of(w.r#type().map_err(pe)?) else {
                continue;
            };
            let wobj = w.annotation().object();
            let field = field_of(&wobj)?;
            if kind != FieldType::RadioButton && out.iter().any(|f| f.name == name) {
                continue;
            }
            let ff = flags_of(&field);
            let is_btn = matches!(kind, FieldType::CheckBox | FieldType::RadioButton);
            out.push(FieldSpec {
                name,
                kind,
                page: p as usize,
                rect: raw_rect(&wobj),
                required: ff & F_REQUIRED != 0,
                read_only: ff & F_READ_ONLY != 0,
                value: w.value().map_err(pe)?.filter(|v| !v.is_empty()),
                options: if matches!(kind, FieldType::ComboBox | FieldType::ListBox) {
                    options_of(&field)
                } else {
                    Vec::new()
                },
                export_value: if is_btn { on_state(&wobj) } else { None },
                multiline: ff & F_MULTILINE != 0,
                password: ff & F_PASSWORD != 0,
                max_len: field
                    .get_dict_inheritable("MaxLen")
                    .ok()
                    .flatten()
                    .and_then(|m| m.as_int().ok())
                    .map(|m| m as u32),
                font_size: 0.0,
                tooltip: field.get_dict("TU").ok().flatten().and_then(|t| text_of(&t)),
                caption: None,
                format: Default::default(),
                validation: Default::default(),
                calculation: crate::js::read_calc(&field),
            });
        }
    }
    Ok(out)
}
