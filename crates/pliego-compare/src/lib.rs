//! Comparación de dos versiones de un PDF (flujo W08).
//!
//! - Alinea las páginas (detecta las insertadas y las eliminadas).
//! - Compara el texto por palabras, con su posición en la página y los cambios de fuente o tamaño.
//! - Compara visualmente las páginas emparejadas (píxeles) y agrupa las diferencias en regiones.
//! - Genera un informe en PDF ([`write_report`]) o en JSON (el propio [`Comparison`] es serializable).
//!
//! Páginas desde 0 en la API.

mod align;
mod diff;
mod extract;
mod model;
mod report;
mod visual;

use std::path::Path;

use pliego_core::{JobContext, Result};
use serde::{Deserialize, Serialize};

pub use model::{ChangeKind, Comparison, PageComparison, PageStatus, Rect, Summary, TextChange, VisualRegion};
pub use report::{write_report, write_report_with};

use align::Slot;

/// Opciones de la comparación. Valores por defecto razonables para documentos de oficina.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CompareOptions {
    /// Comparar el texto palabra por palabra.
    pub text: bool,
    /// Comparar las páginas por píxeles.
    pub visual: bool,
    /// Resolución de la comparación visual (ppp).
    pub visual_dpi: f32,
    /// Diferencia por canal (0–255) a partir de la cual un píxel cuenta como distinto.
    pub visual_threshold: u8,
    /// Distancia en puntos para unir zonas distintas cercanas en una sola región.
    pub region_merge_gap: f32,
    /// Mínimo de píxeles distintos para que una región se informe.
    pub min_region_pixels: u32,
    /// Parecido mínimo (0–1) para emparejar dos páginas.
    pub page_match_threshold: f32,
    pub password_a: Option<String>,
    pub password_b: Option<String>,
}

impl Default for CompareOptions {
    fn default() -> Self {
        Self {
            text: true,
            visual: true,
            visual_dpi: 96.0,
            visual_threshold: 40,
            region_merge_gap: 6.0,
            min_region_pixels: 12,
            page_match_threshold: 0.5,
            password_a: None,
            password_b: None,
        }
    }
}

/// Compara `a` (original) con `b` (nueva).
pub fn compare(a: &Path, b: &Path, opts: &CompareOptions, ctx: &JobContext) -> Result<Comparison> {
    let da = extract::open(a, opts.password_a.as_deref())?;
    let db = extract::open(b, opts.password_b.as_deref())?;
    let na = da.page_count().map_err(extract::pdf_err)? as u32;
    let nb = db.page_count().map_err(extract::pdf_err)? as u32;
    let total = (na + nb) as u64 * 2;

    let mut done = 0u64;
    let mut load = |doc: &mupdf::Document, n: u32, label: &str| -> Result<Vec<extract::PageData>> {
        let mut v = Vec::with_capacity(n as usize);
        for i in 0..n {
            ctx.step(done, total, &format!("Leyendo {label}, página {}", i + 1))?;
            v.push(extract::page_data(doc, i)?);
            done += 1;
        }
        Ok(v)
    };
    let pa = load(&da, na, "el original")?;
    let pb = load(&db, nb, "la versión nueva")?;

    let slots = align::align(&pa, &pb, opts.page_match_threshold);
    let params = visual::VisualParams {
        threshold: opts.visual_threshold,
        merge_gap_pt: opts.region_merge_gap,
        min_pixels: opts.min_region_pixels,
        scale: opts.visual_dpi.clamp(24.0, 300.0) / 72.0,
    };

    let mut pages = Vec::with_capacity(slots.len());
    let mut summary = Summary {
        pages_a: na,
        pages_b: nb,
        ..Summary::default()
    };
    for slot in slots {
        ctx.step(done, total, "Comparando páginas")?;
        done += 2;
        match slot {
            Slot::OnlyA(i) => {
                summary.deleted_pages += 1;
                pages.push(single(PageStatus::Deleted, Some(i), None));
            }
            Slot::OnlyB(j) => {
                summary.inserted_pages += 1;
                pages.push(single(PageStatus::Inserted, None, Some(j)));
            }
            Slot::Pair(i, j) => {
                let (ia, ib) = (&pa[i as usize], &pb[j as usize]);
                let text_changes = if opts.text {
                    diff::diff_words(&ia.words, &ib.words, i, j)
                } else {
                    Vec::new()
                };
                let visual_regions = if opts.visual {
                    let ra = extract::render(&da, i, params.scale)?;
                    let rb = extract::render(&db, j, params.scale)?;
                    visual::diff_regions(&ra, &rb, &params)
                } else {
                    Vec::new()
                };
                let bag_a: Vec<_> = ia.words.iter().map(|w| w.text.as_str()).collect();
                let similarity = word_similarity(&bag_a, &ib.words);
                for t in &text_changes {
                    let (wa, wb) = (count_words(&t.text_a), count_words(&t.text_b));
                    if t.format_only {
                        summary.format_changes += 1;
                    } else {
                        match t.kind {
                            ChangeKind::Added => summary.added_words += wb,
                            ChangeKind::Removed => summary.removed_words += wa,
                            ChangeKind::Changed => summary.changed_words += wb,
                        }
                    }
                }
                summary.visual_regions += visual_regions.len() as u32;
                let pc = PageComparison {
                    status: PageStatus::Matched,
                    page_a: Some(i),
                    page_b: Some(j),
                    similarity,
                    text_changes,
                    visual_regions,
                };
                if pc.has_differences() {
                    summary.changed_pages += 1;
                }
                pages.push(pc);
            }
        }
    }
    ctx.step(total, total, "Comparación terminada")?;
    Ok(Comparison {
        path_a: a.to_path_buf(),
        path_b: b.to_path_buf(),
        summary,
        pages,
        password_a: opts.password_a.clone(),
        password_b: opts.password_b.clone(),
    })
}

fn single(status: PageStatus, a: Option<u32>, b: Option<u32>) -> PageComparison {
    PageComparison {
        status,
        page_a: a,
        page_b: b,
        similarity: 0.0,
        text_changes: Vec::new(),
        visual_regions: Vec::new(),
    }
}

fn count_words(s: &str) -> u32 {
    s.split_whitespace().count() as u32
}

fn word_similarity(a: &[&str], b: &[extract::Word]) -> f32 {
    use std::collections::HashMap;
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    let mut bag: HashMap<&str, u32> = HashMap::new();
    for w in a {
        *bag.entry(w).or_insert(0) += 1;
    }
    let mut common = 0u32;
    for w in b {
        if let Some(n) = bag.get_mut(w.text.as_str())
            && *n > 0
        {
            *n -= 1;
            common += 1;
        }
    }
    2.0 * common as f32 / (a.len() + b.len()) as f32
}
