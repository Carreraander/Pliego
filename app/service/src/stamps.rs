//! Sellos como anotación desde la app (flujo W27, modo Sellos): coloca un sello predefinido, de texto o de imagen en
//! el centro o en una esquina de las páginas elegidas y guarda una copia. Colocar con clic y arrastrar llegará cuando
//! la capa interactiva del visor esté en `main`; este módulo ya recibe un recuadro en puntos.

use std::path::Path;

use pliego_annot::{AnnotSpec, Geometry, RectPts, StampSource};
use pliego_appdata::copyjob::unique_sibling;
use pliego_core::{Error, JobContext, PageRange, Report, Result};
use serde::{Deserialize, Serialize};

/// Dónde va el sello en la página.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Place {
    Center,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// Margen a los bordes en puntos.
const MARGIN: f32 = 36.0;

/// Recuadro del sello (en el espacio visual de la página, y hacia abajo) para una página de `w` × `h` puntos.
/// El sello ocupa el 40 % del ancho (entre 120 y 260 pt) con proporción 10:3.
pub fn stamp_rect(w: f32, h: f32, place: Place) -> RectPts {
    let sw = (w * 0.4).clamp(120.0, 260.0).min(w);
    let sh = (sw * 0.3).min(h);
    let (x0, y0) = match place {
        Place::Center => ((w - sw) / 2.0, (h - sh) / 2.0),
        Place::TopLeft => (MARGIN.min(w - sw), MARGIN.min(h - sh)),
        Place::TopRight => ((w - sw - MARGIN).max(0.0), MARGIN.min(h - sh)),
        Place::BottomLeft => (MARGIN.min(w - sw), (h - sh - MARGIN).max(0.0)),
        Place::BottomRight => ((w - sw - MARGIN).max(0.0), (h - sh - MARGIN).max(0.0)),
    };
    [x0, y0, x0 + sw, y0 + sh]
}

/// Lo que pide el panel de Sellos.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StampRequest {
    pub source: StampSource,
    pub place: Place,
}

/// Pone el sello en las páginas `pages` (`"all"`, `"3"`, `"1-3,5"`…) de `src` y guarda `<nombre> (con sello).pdf`
/// junto a él. El original no cambia.
pub fn stamp_annot_copy(
    src: &Path,
    pages: &str,
    password: Option<&str>,
    req: &StampRequest,
    ctx: &JobContext,
) -> Result<Report> {
    let range: PageRange = pages.trim().parse()?;
    if let StampSource::Text { text, .. } = &req.source
        && text.trim().is_empty()
    {
        return Err(Error::InvalidInput("el texto del sello está vacío".into()));
    }
    let mut doc = pliego_organize::open(src, password)?;
    let sel = range.resolve(pliego_organize::page_count(&doc)?)?;
    for (i, &p) in sel.iter().enumerate() {
        ctx.step(i as u64, sel.len() as u64, "sello")?;
        let b = doc
            .load_page(p as i32)
            .and_then(|pg| pg.bounds())
            .map_err(|e| Error::Pdf(e.to_string()))?;
        let rect = stamp_rect(b.width(), b.height(), req.place);
        let spec = AnnotSpec::new(
            p as usize,
            Geometry::Stamp {
                rect,
                source: req.source.clone(),
            },
        );
        pliego_annot::add(&mut doc, &spec)?;
    }
    let out = unique_sibling(src, "con sello", "pdf");
    pliego_organize::save(&doc, &out)?;
    Ok(Report::with_output(out, format!("sello en {} páginas", sel.len())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_stays_inside_the_page_for_every_place() {
        for (w, h) in [(595.0, 842.0), (842.0, 595.0), (200.0, 100.0)] {
            for place in [
                Place::Center,
                Place::TopLeft,
                Place::TopRight,
                Place::BottomLeft,
                Place::BottomRight,
            ] {
                let [x0, y0, x1, y1] = stamp_rect(w, h, place);
                assert!(
                    x0 >= 0.0 && y0 >= 0.0 && x1 <= w + 0.01 && y1 <= h + 0.01,
                    "{place:?} {w}x{h}"
                );
                assert!(x1 > x0 && y1 > y0);
            }
        }
    }

    #[test]
    fn corners_are_where_they_say() {
        let c = stamp_rect(595.0, 842.0, Place::Center);
        assert!(((c[0] + c[2]) / 2.0 - 297.5).abs() < 0.01);
        let tl = stamp_rect(595.0, 842.0, Place::TopLeft);
        let br = stamp_rect(595.0, 842.0, Place::BottomRight);
        assert!(tl[0] < 100.0 && tl[1] < 100.0);
        assert!(br[2] > 495.0 && br[3] > 742.0);
    }
}
