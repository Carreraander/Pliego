use std::collections::BTreeMap;

use pliego_core::{Error, JobContext, Result};
use pliego_pdf::PdfDocument;
use serde::{Deserialize, Serialize};

use crate::scan::{WidthEdit, page_content, scan_page_with};

/// Resultado de [`fix_hairlines`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HairlineFix {
    /// Ajustes de grosor (`w`) modificados.
    pub widths_changed: usize,
    /// Páginas cuyo contenido (o el de sus formularios) cambió.
    pub pages_changed: usize,
}

type Edits = BTreeMap<(usize, usize), f32>;

fn format_width(w: f32) -> String {
    // Se redondea hacia arriba a 3 decimales para no quedar por debajo del mínimo.
    let v = (w * 1000.0).ceil() / 1000.0;
    let s = format!("{v:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Reemplaza los operandos indicados (las posiciones no deben solaparse).
fn apply(data: &[u8], edits: &BTreeMap<(usize, usize), f32>) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut at = 0;
    for (&(start, end), &w) in edits {
        out.extend_from_slice(&data[at..start]);
        out.extend_from_slice(format_width(w).as_bytes());
        at = end;
    }
    out.extend_from_slice(&data[at..]);
    out
}

fn merge(into: &mut BTreeMap<(usize, usize), f32>, e: &WidthEdit) {
    let slot = into.entry(e.span).or_insert(0.0);
    *slot = slot.max(e.new_width);
}

/// Sube al mínimo `min_width` (en puntos) el grosor de todos los trazos más finos, incluidos los de líneas
/// de grosor 0 («la línea más fina del dispositivo»). Si un formulario se usa con escalas distintas, se aplica
/// el grosor necesario para la escala más pequeña.
///
/// Solo se tocan los operadores `w` del contenido de las páginas y de los formularios XObject; los grosores
/// que vienen de un estado gráfico externo (`gs` con `/LW`) se dejan como están.
pub fn fix_hairlines(doc: &mut PdfDocument, min_width: f32, ctx: &JobContext<'_>) -> Result<HairlineFix> {
    if !(min_width.is_finite() && min_width > 0.0) {
        return Err(Error::InvalidInput(
            "el grosor mínimo debe ser un número positivo".into(),
        ));
    }
    let pages = doc.page_count()?.max(0) as usize;
    let mut forms: BTreeMap<i32, BTreeMap<(usize, usize), f32>> = BTreeMap::new();
    let mut page_edits: Vec<(usize, Edits)> = Vec::new();
    let mut form_pages: BTreeMap<i32, Vec<usize>> = BTreeMap::new();
    for p in 0..pages {
        ctx.step(p as u64, pages as u64, "Buscando líneas finas")?;
        let scan = scan_page_with(doc, p, Some(min_width))?;
        let mut own = BTreeMap::new();
        for e in &scan.edits {
            if e.key == 0 {
                merge(&mut own, e);
            } else {
                merge(forms.entry(e.key).or_default(), e);
                let list = form_pages.entry(e.key).or_default();
                if list.last() != Some(&p) {
                    list.push(p);
                }
            }
        }
        if !own.is_empty() {
            page_edits.push((p, own));
        }
    }

    let mut report = HairlineFix::default();
    let mut changed_pages = std::collections::BTreeSet::new();
    for (p, edits) in page_edits {
        let data = apply(&page_content(doc, p)?, &edits);
        let stream = doc.add_stream(&data, None, false)?;
        let mut page = doc.find_page(p as i32)?;
        page.dict_put("Contents", stream)?;
        report.widths_changed += edits.len();
        changed_pages.insert(p);
    }
    for (key, edits) in forms {
        let mut form = doc.new_indirect(key, 0)?;
        let data = apply(&form.read_stream()?, &edits);
        form.write_stream(&data)?;
        report.widths_changed += edits.len();
        changed_pages.extend(form_pages.get(&key).into_iter().flatten().copied());
    }
    report.pages_changed = changed_pages.len();
    ctx.step(pages as u64, pages as u64, "Líneas finas corregidas")?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_up_and_trims() {
        assert_eq!(format_width(0.25), "0.25");
        assert_eq!(format_width(0.2500001), "0.251");
        assert_eq!(format_width(2.0), "2");
    }

    #[test]
    fn applies_edits_in_order() {
        let data = b"q 0.05 w S Q 0 w S";
        let mut e = BTreeMap::new();
        e.insert((2, 6), 0.25);
        e.insert((13, 14), 0.5);
        assert_eq!(apply(data, &e), b"q 0.25 w S Q 0.5 w S");
    }
}
