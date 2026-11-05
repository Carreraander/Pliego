//! Formato, validación y cálculo con acciones JavaScript de AcroForm.
//!
//! Se usan las funciones estándar (`AFNumber_Format`, `AFRange_Validate`, `AFSimple_Calculate`...) que entienden los
//! lectores habituales y MuPDF (mujs, sin acceso a archivos ni red). Los textos de usuario se insertan como literales
//! JSON, que son literales JavaScript válidos, para que ningún nombre de campo o rótulo pueda inyectar código.

use pliego_core::{Error, Result};
use pliego_pdf::{Object, PdfDocument};
use serde::{Deserialize, Serialize};

use crate::objects::*;
use crate::read::page_widgets;

/// Número: decimales, separadores al estilo español (`1.234,56`) y moneda opcional.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NumberFormat {
    pub decimals: u8,
    /// Separador de miles.
    pub thousands: bool,
    /// Coma decimal (`1,5`); si es falso, punto (`1.5`).
    pub decimal_comma: bool,
    /// Símbolo de moneda, p. ej. `€`.
    pub currency: Option<String>,
    /// Moneda antes de la cifra (por defecto, después: `1.234,56 €`).
    pub currency_before: bool,
}

impl Default for NumberFormat {
    fn default() -> Self {
        Self {
            decimals: 2,
            thousands: true,
            decimal_comma: true,
            currency: None,
            currency_before: false,
        }
    }
}

/// Formato de un campo.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum FieldFormat {
    #[default]
    None,
    Number(NumberFormat),
    Percent {
        decimals: u8,
    },
    /// Fecha, p. ej. `dd/mm/yyyy`.
    Date {
        pattern: String,
    },
    /// Hora, p. ej. `HH:MM`.
    Time {
        pattern: String,
    },
    /// Código postal de 5 cifras.
    PostalCode,
    /// Máscara de tecleo de Acrobat: `9` cifra, `A` letra, `O` letra o cifra, `X` cualquier carácter; el resto es literal.
    Mask {
        mask: String,
    },
}

/// Validación al confirmar un valor.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum Validation {
    #[default]
    None,
    /// Número entre dos límites (cualquiera puede faltar).
    Range { min: Option<f64>, max: Option<f64> },
    /// DNI o NIE español, con su letra de control.
    Dni,
    /// IBAN con su dígito de control (módulo 97).
    Iban,
    /// JavaScript propio (puede poner `event.rc = false` para rechazar).
    Script { js: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CalcOp {
    Sum,
    Product,
    Average,
    Min,
    Max,
}

impl CalcOp {
    fn code(self) -> &'static str {
        match self {
            CalcOp::Sum => "SUM",
            CalcOp::Product => "PRD",
            CalcOp::Average => "AVG",
            CalcOp::Min => "MIN",
            CalcOp::Max => "MAX",
        }
    }
    fn from_code(c: &str) -> Option<Self> {
        Some(match c {
            "SUM" => CalcOp::Sum,
            "PRD" => CalcOp::Product,
            "AVG" => CalcOp::Average,
            "MIN" => CalcOp::Min,
            "MAX" => CalcOp::Max,
            _ => return None,
        })
    }
}

/// Cálculo automático de un campo a partir de otros.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Calculation {
    pub op: CalcOp,
    /// Nombres completos de los campos de entrada.
    pub fields: Vec<String>,
    /// Redondeo del resultado (evita `0.30000000000000004`).
    #[serde(default)]
    pub decimals: Option<u8>,
}

/// Literal de cadena JavaScript.
fn js_str(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

fn js_num(v: f64) -> String {
    if v.is_finite() { format!("{v}") } else { "0".into() }
}

fn sep_style(thousands: bool, comma: bool) -> u8 {
    match (thousands, comma) {
        (true, false) => 0,
        (false, false) => 1,
        (true, true) => 2,
        (false, true) => 3,
    }
}

/// Acciones `(formato, tecleo)` de un formato.
fn format_scripts(f: &FieldFormat) -> Result<(Option<String>, Option<String>)> {
    Ok(match f {
        FieldFormat::None => (None, None),
        FieldFormat::Number(n) => {
            let args = format!(
                "{}, {}, 0, 0, {}, {}",
                n.decimals.min(12),
                sep_style(n.thousands, n.decimal_comma),
                js_str(n.currency.as_deref().unwrap_or("")),
                n.currency_before
            );
            (
                Some(format!("AFNumber_Format({args});")),
                Some(format!("AFNumber_Keystroke({args});")),
            )
        }
        FieldFormat::Percent { decimals } => {
            let args = format!("{}, {}", (*decimals).min(12), sep_style(true, true));
            (
                Some(format!("AFPercent_Format({args});")),
                Some(format!("AFPercent_Keystroke({args});")),
            )
        }
        FieldFormat::Date { pattern } => {
            check_pattern(pattern)?;
            (
                Some(format!("AFDate_FormatEx({});", js_str(pattern))),
                Some(format!("AFDate_KeystrokeEx({});", js_str(pattern))),
            )
        }
        FieldFormat::Time { pattern } => {
            check_pattern(pattern)?;
            (
                Some(format!("AFTime_FormatEx({});", js_str(pattern))),
                Some("AFTime_Keystroke(0);".into()),
            )
        }
        FieldFormat::PostalCode => (
            Some("AFSpecial_Format(0);".into()),
            Some("AFSpecial_Keystroke(0);".into()),
        ),
        FieldFormat::Mask { mask } => {
            check_pattern(mask)?;
            (None, Some(format!("AFSpecial_KeystrokeEx({});", js_str(mask))))
        }
    })
}

fn check_pattern(p: &str) -> Result<()> {
    if p.is_empty() || p.chars().count() > 64 {
        return Err(Error::InvalidInput(
            "el patrón de formato debe tener entre 1 y 64 caracteres".into(),
        ));
    }
    Ok(())
}

const DNI_JS: &str = "(function(){var v=String(event.value).toUpperCase().replace(/[\\s-]/g,'');if(v==='')return;\
var L='TRWAGMYFPDXBNJZSQVHLCKE';var m=/^([XYZ]|\\d)(\\d{7})([A-Z])$/.exec(v);var ok=false;\
if(m){var n=m[1]==='X'?'0':m[1]==='Y'?'1':m[1]==='Z'?'2':m[1];ok=L.charAt(parseInt(n+m[2],10)%23)===m[3];}\
if(!ok){event.rc=false;app.alert('El DNI o NIE no es válido.');}})();";

const IBAN_JS: &str = "(function(){var v=String(event.value).toUpperCase().replace(/\\s/g,'');if(v==='')return;\
var ok=/^[A-Z]{2}\\d{2}[A-Z0-9]{11,30}$/.test(v);if(ok){var r=v.substring(4)+v.substring(0,4);var rem=0;\
for(var i=0;i<r.length;i++){var c=r.charAt(i);var d=(c>='A'&&c<='Z')?String(c.charCodeAt(0)-55):c;\
for(var j=0;j<d.length;j++){rem=(rem*10+parseInt(d.charAt(j),10))%97;}}ok=rem===1;}\
if(!ok){event.rc=false;app.alert('El IBAN no es válido.');}})();";

fn validation_script(v: &Validation) -> Result<Option<String>> {
    Ok(match v {
        Validation::None => None,
        Validation::Range { min, max } => {
            if let (Some(a), Some(b)) = (min, max)
                && a > b
            {
                return Err(Error::InvalidInput("el mínimo del rango es mayor que el máximo".into()));
            }
            Some(format!(
                "AFRange_Validate({}, {}, {}, {});",
                min.is_some(),
                js_num(min.unwrap_or(0.0)),
                max.is_some(),
                js_num(max.unwrap_or(0.0))
            ))
        }
        Validation::Dni => Some(DNI_JS.into()),
        Validation::Iban => Some(IBAN_JS.into()),
        Validation::Script { js } => (!js.trim().is_empty()).then(|| js.clone()),
    })
}

fn calc_script(c: &Calculation) -> String {
    let list: Vec<String> = c.fields.iter().map(|f| js_str(f)).collect();
    let mut s = format!(
        "AFSimple_Calculate({}, new Array({}));",
        js_str(c.op.code()),
        list.join(",")
    );
    if let Some(d) = c.decimals {
        let k = 10f64.powi(d.min(10) as i32);
        s.push_str(&format!(" event.value = Math.round(event.value * {k}) / {k};"));
    }
    s
}

/// Fija (o quita, con `None`) la acción de `/AA` bajo la clave `key` (`F`, `K`, `V` o `C`).
fn set_action(doc: &PdfDocument, field: &mut Object, key: &str, js: Option<&str>) -> Result<()> {
    let existing = field.get_dict("AA")?.filter(|a| a.is_dict().unwrap_or(false));
    match js {
        Some(js) => {
            let mut aa = match existing {
                Some(a) => a,
                None => {
                    put(field, "AA", obj(doc, "<< >>")?)?;
                    field
                        .get_dict("AA")?
                        .ok_or_else(|| Error::Pdf("no se pudo crear /AA".into()))?
                }
            };
            let act = obj(doc, &format!("<< /S /JavaScript /JS {} >>", pdf_str(js)))?;
            put(&mut aa, key, act)
        }
        None => {
            if let Some(mut aa) = existing {
                aa.dict_delete(key)?;
            }
            Ok(())
        }
    }
}

fn field_named(doc: &PdfDocument, name: &str) -> Result<Object> {
    for p in 0..doc.page_count()? {
        for (n, w) in page_widgets(doc, p)? {
            if n == name {
                let f = field_of(&w.annotation().object())?;
                if !f.is_indirect().unwrap_or(false) {
                    return Err(Error::Unsupported(format!(
                        "el campo «{name}» no es un objeto indirecto"
                    )));
                }
                return Ok(f);
            }
        }
    }
    Err(Error::InvalidInput(format!("el campo «{name}» no existe")))
}

/// Fija el formato de un campo (`FieldFormat::None` lo quita).
pub fn set_format(doc: &mut PdfDocument, name: &str, format: &FieldFormat) -> Result<()> {
    let (fmt, key) = format_scripts(format)?;
    let mut field = field_named(doc, name)?;
    set_action(doc, &mut field, "F", fmt.as_deref())?;
    set_action(doc, &mut field, "K", key.as_deref())?;
    refresh_widgets(doc, name)
}

/// Fija la validación de un campo (`Validation::None` la quita).
pub fn set_validation(doc: &mut PdfDocument, name: &str, validation: &Validation) -> Result<()> {
    let js = validation_script(validation)?;
    let mut field = field_named(doc, name)?;
    set_action(doc, &mut field, "V", js.as_deref())
}

/// Regenera la apariencia de los widgets de un campo (el formato cambia cómo se ve el valor).
fn refresh_widgets(doc: &mut PdfDocument, name: &str) -> Result<()> {
    for p in 0..doc.page_count()? {
        for (n, w) in page_widgets(doc, p)? {
            if n == name {
                crate::ap::rebuild_ap(doc, &mut w.annotation().object())?;
            }
        }
    }
    Ok(())
}

/// Lee el cálculo de un campo si lo definió `set_calculation` (u otra herramienta con el mismo formato).
pub fn get_calculation(doc: &PdfDocument, name: &str) -> Result<Option<Calculation>> {
    let field = field_named(doc, name)?;
    Ok(read_calc(&field))
}

pub(crate) fn read_calc(field: &Object) -> Option<Calculation> {
    let js = field
        .get_dict("AA")
        .ok()??
        .get_dict("C")
        .ok()??
        .get_dict("JS")
        .ok()??
        .as_string()
        .ok()?;
    let rest = js.strip_prefix("AFSimple_Calculate(")?;
    // Literales JSON consecutivos: la operación y luego los campos.
    let mut strings = Vec::new();
    let mut i = 0;
    let bytes = rest.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'"' {
            let mut de = serde_json::Deserializer::from_str(&rest[i..]).into_iter::<String>();
            let v = de.next()?.ok()?;
            i += de.byte_offset();
            strings.push(v);
        } else if bytes[i] == b';' {
            break;
        } else {
            i += 1;
        }
    }
    let op = CalcOp::from_code(strings.first()?)?;
    let decimals = js.split("Math.round(event.value * ").nth(1).and_then(|t| {
        let k: f64 = t.split(')').next()?.trim().parse().ok()?;
        Some(k.log10().round() as u8)
    });
    Some(Calculation {
        op,
        fields: strings[1..].to_vec(),
        decimals,
    })
}

/// Fija (o quita, con `None`) el cálculo de un campo y recalcula el orden de cálculo del formulario
/// (`/CO`) para que cada campo se calcule después de sus entradas calculadas.
pub fn set_calculation(doc: &mut PdfDocument, name: &str, calc: Option<&Calculation>) -> Result<()> {
    let mut field = field_named(doc, name)?;
    if let Some(c) = calc {
        if c.fields.is_empty() {
            return Err(Error::InvalidInput(
                "el cálculo necesita al menos un campo de entrada".into(),
            ));
        }
        for f in &c.fields {
            if f == name {
                return Err(Error::InvalidInput(
                    "un campo no puede calcularse a partir de sí mismo".into(),
                ));
            }
            field_named(doc, f)?;
        }
    }
    let previous = field.get_dict("AA")?.and_then(|a| a.get_dict("C").ok().flatten());
    set_action(doc, &mut field, "C", calc.map(calc_script).as_deref())?;
    if let Err(e) = update_calc_order(doc) {
        // Deshace el cambio si deja un ciclo.
        let old = previous
            .and_then(|p| p.get_dict("JS").ok().flatten())
            .and_then(|j| j.as_string().ok());
        set_action(doc, &mut field, "C", old.as_deref())?;
        return Err(e);
    }
    Ok(())
}

/// Reconstruye `/CO` con todos los campos que tienen cálculo, ordenados por dependencias.
fn update_calc_order(doc: &mut PdfDocument) -> Result<()> {
    let mut names: Vec<String> = Vec::new();
    let mut objs: Vec<Object> = Vec::new();
    let mut deps: Vec<Vec<String>> = Vec::new();
    for p in 0..doc.page_count()? {
        for (n, w) in page_widgets(doc, p)? {
            if names.contains(&n) {
                continue;
            }
            let f = field_of(&w.annotation().object())?;
            if let Some(c) = read_calc(&f) {
                names.push(n);
                deps.push(c.fields);
                objs.push(f);
            }
        }
    }
    // Orden topológico estable (algoritmo de Kahn respetando el orden de aparición).
    let mut done: Vec<usize> = Vec::new();
    while done.len() < names.len() {
        let next = (0..names.len()).find(|i| {
            !done.contains(i)
                && deps[*i].iter().all(|d| match names.iter().position(|n| n == d) {
                    Some(j) => done.contains(&j),
                    None => true,
                })
        });
        match next {
            Some(i) => done.push(i),
            None => {
                return Err(Error::InvalidInput(
                    "los cálculos se referencian entre sí en círculo".into(),
                ));
            }
        }
    }
    let mut af = doc
        .catalog()?
        .get_dict("AcroForm")?
        .ok_or_else(|| Error::Pdf("el documento no tiene formulario".into()))?;
    if done.is_empty() {
        af.dict_delete("CO")?;
        return Ok(());
    }
    put(&mut af, "CO", obj(doc, "[]")?)?;
    let mut co = af
        .get_dict("CO")?
        .ok_or_else(|| Error::Pdf("no se pudo crear /CO".into()))?;
    for i in done {
        co.array_push(objs[i].try_clone()?)?;
    }
    Ok(())
}

/// Número de un valor de campo: admite `1234.5`, `1.234,5` y `1234,5`; lo que no sea número cuenta como 0.
fn parse_number(s: &str) -> f64 {
    let t: String = s
        .trim()
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '€' && *c != '%')
        .collect();
    let t = if t.contains(',') {
        t.replace('.', "").replace(',', ".")
    } else {
        t
    };
    t.parse().ok().filter(|v: &f64| v.is_finite()).unwrap_or(0.0)
}

fn compute(op: CalcOp, values: &[f64]) -> f64 {
    match op {
        CalcOp::Sum => values.iter().sum(),
        CalcOp::Product => values.iter().product(),
        CalcOp::Average => values.iter().sum::<f64>() / values.len().max(1) as f64,
        CalcOp::Min => values.iter().copied().fold(f64::INFINITY, f64::min),
        CalcOp::Max => values.iter().copied().fold(f64::NEG_INFINITY, f64::max),
    }
}

/// Valor actual de un campo por su nombre completo (vacío si no tiene).
fn value_of(doc: &PdfDocument, name: &str) -> Result<String> {
    for p in 0..doc.page_count()? {
        for (n, w) in page_widgets(doc, p)? {
            if n == name {
                return Ok(w.value()?.unwrap_or_default());
            }
        }
    }
    Ok(String::new())
}

/// Ejecuta los cálculos del formulario en el orden de `/CO` y guarda los resultados como valores de campo.
/// Devuelve los nombres de los campos calculados.
///
/// Los cálculos creados con `set_calculation` se evalúan aquí de forma nativa (sin motor de scripts); los lectores
/// externos ejecutan el mismo cálculo con su propio `AFSimple_Calculate`.
pub fn recalculate(doc: &mut PdfDocument) -> Result<Vec<String>> {
    let Some(af) = doc.catalog()?.get_dict("AcroForm")? else {
        return Ok(Vec::new());
    };
    let Some(co) = af.get_dict("CO")? else {
        return Ok(Vec::new());
    };
    let want: Vec<i32> = (0..co.len()?)
        .filter_map(|i| co.get_array(i as i32).ok().flatten())
        .map(|o| xref_of(&o))
        .collect();
    // Nombre completo y cálculo de cada campo de /CO.
    let mut order: Vec<(String, Calculation)> = Vec::new();
    for x in want {
        'found: for p in 0..doc.page_count()? {
            for (n, w) in page_widgets(doc, p)? {
                let f = field_of(&w.annotation().object())?;
                if xref_of(&f) == x {
                    if let Some(c) = read_calc(&f) {
                        order.push((n, c));
                    }
                    break 'found;
                }
            }
        }
    }
    let mut done = Vec::new();
    for (name, calc) in order {
        let values: Vec<f64> = calc
            .fields
            .iter()
            .map(|f| value_of(doc, f).map(|v| parse_number(&v)))
            .collect::<Result<_>>()?;
        let mut r = compute(calc.op, &values);
        // Sin decimales pedidos, se limpia el ruido de coma flotante (0.1 + 0.2).
        let k = 10f64.powi(calc.decimals.map_or(10, |d| d.min(10)) as i32);
        r = (r * k).round() / k;
        let text = format!("{r}");
        for p in 0..doc.page_count()? {
            let page = doc.load_pdf_page(p)?;
            let widgets: Vec<_> = page
                .widgets()
                .filter(|w| w.name().ok().flatten().as_deref() == Some(&name))
                .collect();
            for mut w in widgets {
                w.set_value(doc, &text, true)?;
                crate::ap::rebuild_ap(doc, &mut w.annotation().object())?;
            }
        }
        done.push(name);
    }
    Ok(done)
}
