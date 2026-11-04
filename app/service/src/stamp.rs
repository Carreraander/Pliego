//! Sellos y marcas desde la app (flujo W27): Marca de agua, Encabezado y pie, Fondo y Bates guardan una copia junto
//! al original. La lógica vive aquí, sin Tauri, para probarla en Linux; `app/src-tauri/src/commands/stamp.rs` solo
//! traduce.

use std::path::Path;

use pliego_appdata::copyjob::unique_sibling;
use pliego_core::{JobContext, PageRange, Report, Result};
use pliego_stamp::{Background, BatesOptions, HeaderFooter, StampContext, Watermark};
use serde::{Deserialize, Serialize};

/// Una de las cuatro operaciones de sellado, tal como llega de la interfaz (`{"op": "watermark", …}`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum StampOp {
    Watermark(Watermark),
    HeaderFooter(HeaderFooter),
    Background(Background),
    Bates(BatesOptions),
}

impl StampOp {
    /// Sufijo del nombre de la copia: `<nombre> (<sufijo>).pdf`.
    pub fn suffix(&self) -> &'static str {
        match self {
            StampOp::Watermark(_) => "marca de agua",
            StampOp::HeaderFooter(_) => "encabezado y pie",
            StampOp::Background(_) => "fondo",
            StampOp::Bates(_) => "Bates",
        }
    }

    fn summary(&self, n: usize) -> String {
        let what = match self {
            StampOp::Watermark(_) => "marca de agua",
            StampOp::HeaderFooter(_) => "encabezado y pie",
            StampOp::Background(_) => "fondo",
            StampOp::Bates(_) => "numeración Bates",
        };
        format!("{what} en {n} páginas")
    }
}

/// Aplica `op` a las páginas `pages` (texto de rango: `"all"`, `"1-3,5"`, `"odd"`…) de `src` y guarda
/// `<nombre> (<sufijo>).pdf` junto a él. El original no se toca; el informe lleva la copia como resultado.
pub fn stamp_copy(src: &Path, pages: &str, password: Option<&str>, op: &StampOp, ctx: &JobContext) -> Result<Report> {
    let out = unique_sibling(src, op.suffix(), "pdf");
    stamp_file(src, &out, password, pages, op, ctx)
}

/// Como [`stamp_copy`] pero con la ruta de salida explícita.
pub fn stamp_file(
    src: &Path,
    out: &Path,
    password: Option<&str>,
    pages: &str,
    op: &StampOp,
    ctx: &JobContext,
) -> Result<Report> {
    let range: PageRange = pages.trim().parse()?;
    let mut doc = pliego_organize::open(src, password)?;
    let sel = range.resolve(pliego_organize::page_count(&doc)?)?;
    match op {
        StampOp::Watermark(w) => pliego_stamp::watermark(&mut doc, &sel, w, ctx)?,
        StampOp::HeaderFooter(h) => {
            let file = src
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            pliego_stamp::header_footer(&mut doc, &sel, h, &StampContext::today(file), ctx)?
        }
        StampOp::Background(b) => pliego_stamp::background(&mut doc, &sel, b, ctx)?,
        StampOp::Bates(b) => pliego_stamp::bates(&mut doc, &sel, b, ctx)?,
    }
    pliego_organize::save(&doc, out)?;
    Ok(Report::with_output(out.to_path_buf(), op.summary(sel.len())))
}
