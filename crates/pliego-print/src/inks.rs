use std::collections::BTreeMap;

use mupdf::pdf::PdfDocument;
use pliego_core::Result;
use serde::{Deserialize, Serialize};

use crate::pdf_err;
use crate::scan::scan_page;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InkKind {
    /// Cian, magenta, amarillo o negro.
    Process,
    /// Color directo (Pantone u otro), una plancha propia.
    Spot,
}

/// Una tinta (plancha) que usa el documento.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ink {
    pub name: String,
    pub kind: InkKind,
    /// Páginas (desde 0) donde se usa.
    pub pages: Vec<usize>,
}

const PROCESS: [&str; 4] = ["Cyan", "Magenta", "Yellow", "Black"];

fn process_ink(name: &str) -> Option<&'static str> {
    PROCESS.iter().copied().find(|p| p.eq_ignore_ascii_case(name))
}

/// Tintas usadas en todo el documento, procesos primero y luego directos por nombre.
///
/// Las tintas de proceso se detectan por el uso de CMYK (`k`, `DeviceCMYK`, ICC de 4 canales...). `None` y `All`
/// (registro) no cuentan.
pub fn inks(doc: &PdfDocument) -> Result<Vec<Ink>> {
    let pages = doc.page_count().map_err(pdf_err)?.max(0) as usize;
    let mut found: BTreeMap<(u8, String), Vec<usize>> = BTreeMap::new();
    for p in 0..pages {
        let scan = scan_page(doc, p)?;
        let mut add = |kind: u8, name: &str| {
            let list = found.entry((kind, name.to_string())).or_default();
            if list.last() != Some(&p) {
                list.push(p);
            }
        };
        if scan.cmyk > 0 {
            for n in PROCESS {
                add(0, n);
            }
        }
        for s in &scan.spots {
            match process_ink(s) {
                Some(n) => add(0, n),
                None if s == "None" || s == "All" => {}
                None => add(1, s),
            }
        }
    }
    let order = |n: &str| PROCESS.iter().position(|p| *p == n).unwrap_or(PROCESS.len());
    let mut list: Vec<Ink> = found
        .into_iter()
        .map(|((kind, name), pages)| Ink {
            kind: if kind == 0 { InkKind::Process } else { InkKind::Spot },
            name,
            pages,
        })
        .collect();
    list.sort_by(|a, b| (a.kind as u8, order(&a.name), &a.name).cmp(&(b.kind as u8, order(&b.name), &b.name)));
    Ok(list)
}
