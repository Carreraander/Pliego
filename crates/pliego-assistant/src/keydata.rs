//! Extracción determinista de datos clave (fechas e importes). No necesita modelo y siempre indica la página.

use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::chunk::DocumentText;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeyKind {
    Date,
    Amount,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyDatum {
    pub kind: KeyKind,
    /// Texto tal como aparece en el documento.
    pub value: String,
    /// Página de origen, desde 0.
    pub page: u32,
}

const MONTHS: &str =
    "enero|febrero|marzo|abril|mayo|junio|julio|agosto|septiembre|setiembre|octubre|noviembre|diciembre";

static DATES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?i)\b(?:[0-3]?\d\s+de\s+(?:{MONTHS})(?:\s+de(?:l)?\s+\d{{4}})?|[0-3]?\d[/.\-][01]?\d[/.\-](?:\d{{4}}|\d{{2}})|\d{{4}}-[01]\d-[0-3]\d)\b"
    ))
    .unwrap()
});

static AMOUNTS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:[€$£]\s?\d[\d.,]*\d|\b\d[\d.,]*\d?\s?(?:€|euros?|eur\b|usd\b|\$|dólares|dolares))").unwrap()
});

/// Fechas e importes del documento, en orden de aparición y sin repetidos exactos en la misma página.
pub fn extract_key_data(doc: &DocumentText) -> Vec<KeyDatum> {
    let mut out: Vec<KeyDatum> = Vec::new();
    for (i, page) in doc.pages.iter().enumerate() {
        for (kind, re) in [(KeyKind::Date, &*DATES), (KeyKind::Amount, &*AMOUNTS)] {
            for m in re.find_iter(page) {
                let d = KeyDatum {
                    kind,
                    value: m.as_str().trim().to_string(),
                    page: i as u32,
                };
                if !out.contains(&d) {
                    out.push(d);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values(text: &str, kind: KeyKind) -> Vec<String> {
        extract_key_data(&DocumentText::from_pages(vec![text.into()]))
            .into_iter()
            .filter(|d| d.kind == kind)
            .map(|d| d.value)
            .collect()
    }

    #[test]
    fn finds_dates() {
        let v = values(
            "Firmado el 12 de marzo de 2024, vence el 01/09/2025 (ISO 2025-09-01).",
            KeyKind::Date,
        );
        assert_eq!(v, ["12 de marzo de 2024", "01/09/2025", "2025-09-01"]);
    }

    #[test]
    fn finds_amounts() {
        let v = values(
            "Precio: 1.234,56 € más 21 % de IVA; fianza $ 500.00 y 300 euros.",
            KeyKind::Amount,
        );
        assert_eq!(v, ["1.234,56 €", "$ 500.00", "300 euros"]);
    }

    #[test]
    fn records_page() {
        let d = extract_key_data(&DocumentText::from_pages(vec!["nada".into(), "el 3/4/2020".into()]));
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].page, 1);
    }
}
