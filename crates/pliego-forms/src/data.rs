use mupdf::pdf::PdfDocument;
use pliego_core::{Error, Result};
use serde::{Deserialize, Serialize};

use crate::csv;
use crate::fields::{FieldInfo, FieldKind, FieldValue, FieldValues, FillResult, fields, fill};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    Json,
    Csv,
}

fn is_on(v: &str) -> bool {
    matches!(
        v.trim().to_lowercase().as_str(),
        "true" | "1" | "x" | "yes" | "on" | "si" | "sí" | "verdadero"
    )
}

fn exported(f: &FieldInfo) -> Option<FieldValue> {
    match f.kind {
        FieldKind::PushButton | FieldKind::Signature | FieldKind::Unknown => None,
        FieldKind::CheckBox => Some(FieldValue::Bool(
            f.value.as_deref().is_some_and(|v| !v.is_empty() && v != "Off"),
        )),
        _ => Some(FieldValue::Text(
            f.value.clone().filter(|v| v != "Off").unwrap_or_default(),
        )),
    }
}

/// Valores actuales de los campos rellenables.
pub fn current_values(doc: &mut PdfDocument) -> Result<FieldValues> {
    Ok(fields(doc)?
        .iter()
        .filter_map(|f| exported(f).map(|v| (f.name.clone(), v)))
        .collect())
}

/// Exporta los valores de los campos: JSON (objeto nombre→valor) o CSV (cabecera + una fila).
pub fn export_data(doc: &mut PdfDocument, format: Format) -> Result<String> {
    let values = current_values(doc)?;
    match format {
        Format::Json => serde_json::to_string_pretty(&values).map_err(|e| Error::Other(e.to_string())),
        Format::Csv => {
            let header: Vec<String> = values.keys().cloned().collect();
            let row = values
                .values()
                .map(|v| match v {
                    FieldValue::Bool(b) => b.to_string(),
                    FieldValue::Text(t) => t.clone(),
                })
                .collect();
            Ok(csv::write(&[header, row]))
        }
    }
}

/// Convierte una fila de texto a valores tipados según el tipo de cada campo del documento.
pub(crate) fn coerce(infos: &[FieldInfo], row: &[(String, String)]) -> FieldValues {
    let mut out = FieldValues::new();
    for (name, text) in row {
        let kind = infos.iter().find(|i| &i.name == name).map(|i| i.kind);
        let v = match kind {
            Some(FieldKind::CheckBox) => FieldValue::Bool(is_on(text)),
            _ => FieldValue::Text(text.clone()),
        };
        out.insert(name.clone(), v);
    }
    out
}

/// Importa valores desde JSON o CSV (primera fila de datos) y rellena el documento.
pub fn import_data(doc: &mut PdfDocument, text: &str, format: Format) -> Result<FillResult> {
    let values = match format {
        Format::Json => serde_json::from_str::<FieldValues>(text)
            .map_err(|e| Error::InvalidInput(format!("el JSON de datos no es válido: {e}")))?,
        Format::Csv => {
            let rows = csv::parse(text);
            let [header, first, ..] = rows.as_slice() else {
                return Err(Error::InvalidInput(
                    "el CSV necesita una fila de cabecera y una de datos".into(),
                ));
            };
            let pairs: Vec<(String, String)> = header.iter().cloned().zip(first.iter().cloned()).collect();
            coerce(&fields(doc)?, &pairs)
        }
    };
    fill(doc, &values)
}
