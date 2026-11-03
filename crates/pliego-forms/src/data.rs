use mupdf::pdf::PdfDocument;
use pliego_core::{Error, Result};
use serde::{Deserialize, Serialize};

use crate::csv;
use crate::exchange;
use crate::fields::{FieldInfo, FieldKind, FieldValue, FieldValues, FillResult, fields, fill};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    Json,
    Csv,
    Fdf,
    Xfdf,
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
        Format::Fdf | Format::Xfdf => {
            let infos = fields(doc)?;
            let pairs: exchange::Pairs = values
                .into_iter()
                .map(|(name, v)| {
                    let text = match v {
                        // Las casillas se intercambian con el nombre de su estado activo.
                        FieldValue::Bool(b) => {
                            let on = infos
                                .iter()
                                .find(|i| i.name == name)
                                .and_then(|i| i.options.first().cloned())
                                .unwrap_or_else(|| "Yes".into());
                            if b { on } else { "Off".into() }
                        }
                        FieldValue::Text(t) => t,
                    };
                    (name, text)
                })
                .collect();
            Ok(if format == Format::Fdf {
                exchange::write_fdf(&pairs)
            } else {
                exchange::write_xfdf(&pairs)
            })
        }
    }
}

/// Convierte una fila de texto a valores tipados según el tipo de cada campo del documento.
pub(crate) fn coerce(infos: &[FieldInfo], row: &[(String, String)]) -> FieldValues {
    let mut out = FieldValues::new();
    for (name, text) in row {
        let kind = infos.iter().find(|i| &i.name == name).map(|i| i.kind);
        let v = match kind {
            Some(FieldKind::CheckBox) => {
                let is_state = infos
                    .iter()
                    .find(|i| &i.name == name)
                    .is_some_and(|i| i.options.iter().any(|o| o == text));
                FieldValue::Bool(is_state || is_on(text))
            }
            _ => FieldValue::Text(text.clone()),
        };
        out.insert(name.clone(), v);
    }
    out
}

/// Importa valores desde JSON o CSV (primera fila de datos) y rellena el documento.
pub fn import_data(doc: &mut PdfDocument, text: &str, format: Format) -> Result<FillResult> {
    // FDF es binario: aquí cada carácter se toma como un byte (vista Latin-1). Usa `import_data_bytes` con archivos.
    import_data_bytes(
        doc,
        &text
            .chars()
            .map(|c| u8::try_from(u32::from(c)).unwrap_or(b'?'))
            .collect::<Vec<_>>(),
        format,
    )
}

/// Importa valores desde los bytes de un archivo de datos (JSON, CSV, FDF o XFDF).
pub fn import_data_bytes(doc: &mut PdfDocument, bytes: &[u8], format: Format) -> Result<FillResult> {
    let utf8 =
        || std::str::from_utf8(bytes).map_err(|_| Error::InvalidInput("el archivo de datos no es UTF-8 válido".into()));
    let values = match format {
        Format::Fdf | Format::Xfdf => {
            let pairs = if format == Format::Fdf {
                exchange::parse_fdf(bytes)?
            } else {
                exchange::parse_xfdf(utf8()?)?
            };
            coerce(&fields(doc)?, &pairs)
        }
        Format::Json => serde_json::from_str::<FieldValues>(utf8()?)
            .map_err(|e| Error::InvalidInput(format!("el JSON de datos no es válido: {e}")))?,
        Format::Csv => {
            let rows = csv::parse(utf8()?);
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
