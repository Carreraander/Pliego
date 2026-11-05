use std::collections::BTreeMap;

use pliego_core::Result;
use pliego_pdf::{FieldFlags, Object, PdfDocument, PdfWidget, WidgetType};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    Text,
    CheckBox,
    RadioButton,
    ComboBox,
    ListBox,
    PushButton,
    Signature,
    Unknown,
}

/// Valor de un campo: casilla (verdadero/falso) o texto (texto, opción de radio/lista/combo).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FieldValue {
    Bool(bool),
    Text(String),
}

/// Valores por nombre completo de campo.
pub type FieldValues = BTreeMap<String, FieldValue>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldInfo {
    pub name: String,
    pub kind: FieldKind,
    /// Valor actual: texto, estado de la casilla/radio (`Off` si está desmarcada) u opción elegida.
    pub value: Option<String>,
    /// Opciones de combo/lista y estados de activación de casilla/radio.
    pub options: Vec<String>,
    /// Página (desde 0) de la primera aparición.
    pub page: usize,
    /// Recuadro `[x0, y0, x1, y1]` del primer botón/caja del campo, en puntos de página (origen arriba a la izquierda).
    #[serde(default)]
    pub rect: Option<[f32; 4]>,
    pub read_only: bool,
    pub required: bool,
    pub multiline: bool,
    pub password: bool,
    pub signed: bool,
}

fn kind_of(t: WidgetType, flags: FieldFlags) -> FieldKind {
    match t {
        WidgetType::Text => FieldKind::Text,
        WidgetType::Checkbox => FieldKind::CheckBox,
        WidgetType::RadioButton => FieldKind::RadioButton,
        WidgetType::Combobox => FieldKind::ComboBox,
        WidgetType::Listbox => FieldKind::ListBox,
        WidgetType::Button if flags.contains(FieldFlags::PUSHBUTTON) => FieldKind::PushButton,
        WidgetType::Button => FieldKind::PushButton,
        WidgetType::Signature => FieldKind::Signature,
        WidgetType::Unknown => FieldKind::Unknown,
    }
}

/// Estados de activación (distintos de `Off`) del aspecto normal de un widget de botón.
fn on_states(w: &PdfWidget) -> Vec<String> {
    let mut out = Vec::new();
    let obj = w.annotation().object();
    if let Some(n) = obj
        .get_dict("AP")
        .ok()
        .flatten()
        .and_then(|ap| ap.get_dict("N").ok().flatten())
        && n.is_dict().unwrap_or(false)
    {
        for i in 0..n.dict_len().unwrap_or(0) {
            if let Ok(Some(name)) = n.get_dict_key(i as i32) {
                if name != "Off" {
                    out.push(name);
                }
            }
        }
    }
    out
}

fn string_of(o: &Object) -> Option<String> {
    if o.is_string().ok()? {
        o.as_string().ok()
    } else if o.is_name().ok()? {
        o.as_name().ok().map(|n| String::from_utf8_lossy(&n).into_owned())
    } else {
        None
    }
}

/// Opciones de combo/lista: `/Opt` del campo (o de su padre); cada elemento es una cadena o `[exportado, mostrado]`.
fn choice_options(w: &PdfWidget) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = Some(w.annotation().object());
    while let Some(o) = cur {
        if let Ok(Some(opt)) = o.get_dict("Opt")
            && opt.is_array().unwrap_or(false)
        {
            for i in 0..opt.len().unwrap_or(0) {
                if let Ok(Some(item)) = opt.get_array(i as i32) {
                    if item.is_array().unwrap_or(false) {
                        if let Ok(Some(first)) = item.get_array(0)
                            && let Some(s) = string_of(&first)
                        {
                            out.push(s);
                        }
                    } else if let Some(s) = string_of(&item) {
                        out.push(s);
                    }
                }
            }
            return out;
        }
        cur = o.get_dict("Parent").ok().flatten();
    }
    out
}

/// Lista los campos del formulario (uno por nombre; los radios agrupan sus botones).
pub fn fields(doc: &mut PdfDocument) -> Result<Vec<FieldInfo>> {
    let n = doc.page_count()?;
    let mut out: Vec<FieldInfo> = Vec::new();
    for p in 0..n {
        let page = doc.load_pdf_page(p)?;
        for w in page.widgets() {
            let Some(name) = w.name()? else { continue };
            let flags = w.field_flags()?;
            let kind = kind_of(w.r#type()?, flags);
            if let Some(existing) = out.iter_mut().find(|f| f.name == name) {
                if matches!(kind, FieldKind::RadioButton | FieldKind::CheckBox) {
                    for s in on_states(&w) {
                        if !existing.options.contains(&s) {
                            existing.options.push(s);
                        }
                    }
                }
                continue;
            }
            let options = match kind {
                FieldKind::ComboBox | FieldKind::ListBox => choice_options(&w),
                FieldKind::RadioButton | FieldKind::CheckBox => on_states(&w),
                _ => Vec::new(),
            };
            out.push(FieldInfo {
                name,
                kind,
                value: w.value()?,
                options,
                page: p as usize,
                rect: w.annotation().bounds().ok().map(|r| [r.x0, r.y0, r.x1, r.y1]),
                read_only: flags.contains(FieldFlags::READ_ONLY) || w.is_readonly().unwrap_or(false),
                required: flags.contains(FieldFlags::REQUIRED),
                multiline: flags.contains(FieldFlags::MULTILINE),
                password: flags.contains(FieldFlags::PASSWORD),
                signed: w.is_signed().unwrap_or(false),
            });
        }
    }
    Ok(out)
}

/// Resultado de `fill`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FillResult {
    pub filled: Vec<String>,
    /// Nombres pedidos que no existen en el documento.
    pub not_found: Vec<String>,
    /// Campos de solo lectura o valores que el campo no aceptó.
    pub rejected: Vec<String>,
}

/// Rellena campos. Las casillas aceptan `Bool` o el nombre del estado; los radios, el nombre de la opción.
pub fn fill(doc: &mut PdfDocument, values: &FieldValues) -> Result<FillResult> {
    let mut res = FillResult::default();
    let n = doc.page_count()?;
    let mut seen: Vec<String> = Vec::new();
    for p in 0..n {
        let page = doc.load_pdf_page(p)?;
        let widgets: Vec<PdfWidget> = page.widgets().collect();
        for mut w in widgets {
            let Some(name) = w.name()? else { continue };
            let Some(val) = values.get(&name) else { continue };
            let kind = kind_of(w.r#type()?, w.field_flags()?);
            if w.is_readonly().unwrap_or(false) {
                if !res.rejected.contains(&name) {
                    res.rejected.push(name.clone());
                }
                continue;
            }
            let states = on_states(&w);
            let text = match (kind, val) {
                (FieldKind::CheckBox, FieldValue::Bool(b)) => {
                    if *b {
                        states.first().cloned().unwrap_or_else(|| "Yes".into())
                    } else {
                        "Off".into()
                    }
                }
                (FieldKind::RadioButton, FieldValue::Bool(b)) => {
                    // Un radio con Bool activa o desactiva este botón concreto.
                    if *b {
                        states.first().cloned().unwrap_or_else(|| "Yes".into())
                    } else {
                        "Off".into()
                    }
                }
                (FieldKind::RadioButton, FieldValue::Text(t)) => {
                    // Solo actúa el botón cuyo estado coincide; los demás se dejan como están.
                    if !states.contains(t) && t != "Off" {
                        seen.push(name.clone());
                        continue;
                    }
                    if t == "Off" { "Off".into() } else { t.clone() }
                }
                (_, FieldValue::Text(t)) => t.clone(),
                (_, FieldValue::Bool(b)) => b.to_string(),
            };
            let ok = w.set_value(doc, &text, true)?;
            seen.push(name.clone());
            if ok {
                if !res.filled.contains(&name) {
                    res.filled.push(name);
                }
            } else if !res.rejected.contains(&name) {
                res.rejected.push(name);
            }
        }
    }
    for k in values.keys() {
        if !seen.contains(k) && !res.rejected.contains(k) {
            res.not_found.push(k.clone());
        }
    }
    Ok(res)
}

/// Restablece los campos indicados (o todos si `names` es `None`) a su valor por defecto.
pub fn reset(doc: &mut PdfDocument, names: Option<&[String]>) -> Result<usize> {
    let n = doc.page_count()?;
    let mut count = 0;
    for p in 0..n {
        let page = doc.load_pdf_page(p)?;
        let widgets: Vec<PdfWidget> = page.widgets().collect();
        for mut w in widgets {
            let name = w.name()?.unwrap_or_default();
            if names.is_none_or(|ns| ns.contains(&name)) && !w.is_readonly().unwrap_or(false) {
                w.reset(doc)?;
                count += 1;
            }
        }
    }
    Ok(count)
}

/// Aplana los campos: su apariencia pasa a ser contenido fijo de la página. Devuelve cuántos había.
pub fn flatten(doc: &mut PdfDocument) -> Result<usize> {
    let before = fields(doc)?.len();
    doc.bake(false, true)?;
    Ok(before)
}
