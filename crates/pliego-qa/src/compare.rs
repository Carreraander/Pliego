//! Comparación de dos motores sobre un mismo PDF: páginas, tamaños, píxeles a 72 y 150 ppp y texto extraído.

use std::collections::HashMap;
use std::path::Path;

use pliego_core::{EngineDocument, RgbaImage};
use serde::{Deserialize, Serialize};

use crate::engines::DynEngine;

/// Resoluciones comparadas (ppp).
pub const DPIS: [u32; 2] = [72, 150];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thresholds {
    /// Diferencia máxima por canal para que dos píxeles cuenten como iguales (antialiasing).
    pub pixel_tolerance: u8,
    /// Fracción máxima de píxeles distintos por página.
    pub max_diff_fraction: f64,
    /// Similitud mínima del texto extraído (0 a 1).
    pub min_text_similarity: f64,
    /// Páginas máximas por archivo (las primeras).
    pub max_pages: u32,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            pixel_tolerance: 16,
            max_diff_fraction: 0.005,
            min_text_similarity: 0.98,
            max_pages: 20,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageDiff {
    pub size_mismatch: bool,
    pub diff_fraction: f64,
    pub mean_abs: f64,
    pub max_abs: u8,
}

/// Diferencia de dos imágenes RGBA. Si el tamaño no coincide se marca y se compara la zona común.
pub fn diff_images(a: &RgbaImage, b: &RgbaImage, tolerance: u8) -> ImageDiff {
    let size_mismatch = (a.width, a.height) != (b.width, b.height);
    let (w, h) = (a.width.min(b.width) as usize, a.height.min(b.height) as usize);
    let (mut bad, mut sum, mut max) = (0u64, 0u64, 0u8);
    for y in 0..h {
        let ra = &a.data[y * a.width as usize * 4..][..w * 4];
        let rb = &b.data[y * b.width as usize * 4..][..w * 4];
        for x in 0..w {
            let (pa, pb) = (&ra[x * 4..x * 4 + 3], &rb[x * 4..x * 4 + 3]);
            let d = (0..3).map(|i| pa[i].abs_diff(pb[i])).max().unwrap_or(0);
            sum += u64::from(d);
            max = max.max(d);
            if d > tolerance {
                bad += 1;
            }
        }
    }
    let n = (w * h).max(1) as f64;
    ImageDiff {
        size_mismatch,
        diff_fraction: bad as f64 / n,
        mean_abs: sum as f64 / n,
        max_abs: max,
    }
}

/// Similitud de texto entre 0 y 1: coincidencia de palabras como multiconjunto (ignora el orden de línea y el espaciado).
pub fn text_similarity(a: &str, b: &str) -> f64 {
    fn bag(s: &str) -> HashMap<&str, u32> {
        let mut m = HashMap::new();
        for w in s.split_whitespace() {
            *m.entry(w).or_insert(0) += 1;
        }
        m
    }
    let (ba, bb) = (bag(a), bag(b));
    let (na, nb): (u32, u32) = (ba.values().sum(), bb.values().sum());
    if na + nb == 0 {
        return 1.0;
    }
    let common: u32 = ba.iter().map(|(w, c)| (*c).min(*bb.get(w).unwrap_or(&0))).sum();
    f64::from(2 * common) / f64::from(na + nb)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageReport {
    pub page: u32,
    pub size_a: Option<(f32, f32)>,
    pub size_b: Option<(f32, f32)>,
    /// Una entrada por resolución de [`DPIS`].
    pub diffs: Vec<(u32, Option<ImageDiff>)>,
    pub text_similarity: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    /// Dentro de los umbrales.
    Pass,
    /// Los dos motores rechazan el archivo (aceptable: PDF roto o cifrado).
    BothRejected,
    /// Fuera de umbrales o un motor abre y el otro no.
    Fail,
    /// El proceso de comparación murió o se colgó.
    Crash,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileReport {
    pub file: String,
    pub status: Status,
    pub reasons: Vec<String>,
    pub pages_a: Option<u32>,
    pub pages_b: Option<u32>,
    pub pages: Vec<PageReport>,
}

fn size(doc: &dyn EngineDocument, page: u32) -> Option<(f32, f32)> {
    doc.page_size(page).ok().map(|s| (s.width, s.height))
}

/// Compara `path` con los dos motores. Nunca entra en pánico por un error de motor: lo anota en `reasons`.
pub fn compare_file(a: &dyn DynEngine, b: &dyn DynEngine, path: &Path, th: &Thresholds) -> FileReport {
    let file = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut rep = FileReport {
        file,
        status: Status::Pass,
        reasons: Vec::new(),
        pages_a: None,
        pages_b: None,
        pages: Vec::new(),
    };
    let (da, db) = (a.open(path), b.open(path));
    let (da, db) = match (da, db) {
        (Ok(x), Ok(y)) => (x, y),
        (Err(_), Err(_)) => {
            rep.status = Status::BothRejected;
            return rep;
        }
        (Ok(_), Err(e)) => {
            rep.status = Status::Fail;
            rep.reasons
                .push(format!("{} abre y {} rechaza: {e}", a.name(), b.name()));
            return rep;
        }
        (Err(e), Ok(_)) => {
            rep.status = Status::Fail;
            rep.reasons
                .push(format!("{} rechaza ({e}) y {} abre", a.name(), b.name()));
            return rep;
        }
    };
    rep.pages_a = da.page_count().ok();
    rep.pages_b = db.page_count().ok();
    if rep.pages_a != rep.pages_b {
        rep.reasons.push(format!(
            "número de páginas distinto: {:?} frente a {:?}",
            rep.pages_a, rep.pages_b
        ));
    }
    let n = rep.pages_a.unwrap_or(0).min(rep.pages_b.unwrap_or(0)).min(th.max_pages);
    for page in 0..n {
        let mut pr = PageReport {
            page,
            size_a: size(da.as_ref(), page),
            size_b: size(db.as_ref(), page),
            diffs: Vec::new(),
            text_similarity: 1.0,
        };
        for dpi in DPIS {
            let zoom = dpi as f32 / 72.0;
            let d = match (da.render_page(page, zoom), db.render_page(page, zoom)) {
                (Ok(x), Ok(y)) => Some(diff_images(&x, &y, th.pixel_tolerance)),
                (Err(_), Err(_)) => None,
                (x, y) => {
                    rep.reasons.push(format!(
                        "página {} a {dpi} ppp: render {} / {}",
                        page + 1,
                        if x.is_ok() { "ok" } else { "falla" },
                        if y.is_ok() { "ok" } else { "falla" }
                    ));
                    None
                }
            };
            if let Some(d) = &d {
                if d.size_mismatch {
                    rep.reasons
                        .push(format!("página {} a {dpi} ppp: tamaño de imagen distinto", page + 1));
                } else if d.diff_fraction > th.max_diff_fraction {
                    rep.reasons.push(format!(
                        "página {} a {dpi} ppp: {:.3} % de píxeles distintos (máx. {:.3} %)",
                        page + 1,
                        d.diff_fraction * 100.0,
                        th.max_diff_fraction * 100.0
                    ));
                }
            }
            pr.diffs.push((dpi, d));
        }
        if let (Ok(x), Ok(y)) = (da.page_text(page), db.page_text(page)) {
            pr.text_similarity = text_similarity(&x, &y);
            if pr.text_similarity < th.min_text_similarity {
                rep.reasons.push(format!(
                    "página {}: texto con similitud {:.3} (mín. {:.3})",
                    page + 1,
                    pr.text_similarity,
                    th.min_text_similarity
                ));
            }
        }
        rep.pages.push(pr);
    }
    if !rep.reasons.is_empty() {
        rep.status = Status::Fail;
    }
    rep
}

#[cfg(test)]
mod tests {
    use super::*;

    fn img(w: u32, h: u32, v: u8) -> RgbaImage {
        RgbaImage {
            width: w,
            height: h,
            data: vec![v; (w * h * 4) as usize],
        }
    }

    #[test]
    fn identical_images_have_no_diff() {
        let d = diff_images(&img(4, 4, 200), &img(4, 4, 200), 16);
        assert_eq!(d.diff_fraction, 0.0);
        assert!(!d.size_mismatch);
    }

    #[test]
    fn tolerance_absorbs_antialiasing() {
        assert_eq!(diff_images(&img(4, 4, 200), &img(4, 4, 210), 16).diff_fraction, 0.0);
        assert_eq!(diff_images(&img(4, 4, 200), &img(4, 4, 250), 16).diff_fraction, 1.0);
    }

    #[test]
    fn size_mismatch_is_flagged() {
        assert!(diff_images(&img(4, 4, 0), &img(5, 4, 0), 16).size_mismatch);
    }

    #[test]
    fn text_similarity_ignores_order_and_spacing() {
        assert_eq!(text_similarity("a b  c", "c\nb a"), 1.0);
        assert_eq!(text_similarity("", ""), 1.0);
        assert!(text_similarity("a b c d", "a b x y") < 0.6);
    }
}
