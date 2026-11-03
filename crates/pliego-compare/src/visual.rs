//! Diferencia por píxeles entre páginas emparejadas, agrupada en regiones.

use crate::extract::Raster;
use crate::model::{Rect, VisualRegion};

pub(crate) struct VisualParams {
    /// Diferencia máxima por canal (0–255) a partir de la cual un píxel cuenta como distinto.
    pub threshold: u8,
    /// Distancia en puntos por debajo de la cual dos zonas distintas se unen en una región.
    pub merge_gap_pt: f32,
    /// Regiones con menos píxeles distintos que esto se descartan (ruido de suavizado).
    pub min_pixels: u32,
    pub scale: f32,
}

fn px(r: &Raster, x: usize, y: usize) -> [u8; 3] {
    if x < r.width && y < r.height {
        let p = (y * r.width + x) * 3;
        [r.rgb[p], r.rgb[p + 1], r.rgb[p + 2]]
    } else {
        [255, 255, 255]
    }
}

/// Compara dos rasters sobre un lienzo común (relleno blanco) y devuelve las regiones distintas en puntos.
pub(crate) fn diff_regions(a: &Raster, b: &Raster, p: &VisualParams) -> Vec<VisualRegion> {
    let (w, h) = (a.width.max(b.width), a.height.max(b.height));
    let cell = ((p.scale * 3.0).round() as usize).max(2);
    let (gw, gh) = (w.div_ceil(cell), h.div_ceil(cell));
    let mut count = vec![0u32; gw * gh];
    for y in 0..h {
        for x in 0..w {
            let (pa, pb) = (px(a, x, y), px(b, x, y));
            let d = pa.iter().zip(&pb).map(|(u, v)| u.abs_diff(*v)).max().unwrap_or(0);
            if d > p.threshold {
                count[(y / cell) * gw + x / cell] += 1;
            }
        }
    }
    let gap = ((p.merge_gap_pt * p.scale / cell as f32).ceil() as isize).max(1);
    // Componentes conexas sobre las celdas marcadas, uniendo celdas a distancia ≤ gap.
    let mut seen = vec![false; gw * gh];
    let mut out = Vec::new();
    for start in 0..gw * gh {
        if count[start] == 0 || seen[start] {
            continue;
        }
        seen[start] = true;
        let mut stack = vec![start];
        let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0usize, 0usize);
        let mut pixels = 0u32;
        while let Some(c) = stack.pop() {
            let (cx, cy) = ((c % gw) as isize, (c / gw) as isize);
            pixels += count[c];
            x0 = x0.min(cx as usize);
            y0 = y0.min(cy as usize);
            x1 = x1.max(cx as usize);
            y1 = y1.max(cy as usize);
            for ny in (cy - gap).max(0)..=(cy + gap).min(gh as isize - 1) {
                for nx in (cx - gap).max(0)..=(cx + gap).min(gw as isize - 1) {
                    let n = ny as usize * gw + nx as usize;
                    if count[n] > 0 && !seen[n] {
                        seen[n] = true;
                        stack.push(n);
                    }
                }
            }
        }
        if pixels < p.min_pixels {
            continue;
        }
        let to_pt = |v: usize| v as f32 * cell as f32 / p.scale;
        out.push(VisualRegion {
            rect: Rect {
                x0: to_pt(x0),
                y0: to_pt(y0),
                x1: to_pt(x1 + 1).min(w as f32 / p.scale),
                y1: to_pt(y1 + 1).min(h as f32 / p.scale),
            },
            changed_pixels: pixels,
        });
    }
    out.sort_by(|a, b| (a.rect.y0, a.rect.x0).partial_cmp(&(b.rect.y0, b.rect.x0)).unwrap());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blank(w: usize, h: usize) -> Raster {
        Raster {
            width: w,
            height: h,
            rgb: vec![255; w * h * 3],
        }
    }
    fn paint(r: &mut Raster, x0: usize, y0: usize, x1: usize, y1: usize) {
        for y in y0..y1 {
            for x in x0..x1 {
                let p = (y * r.width + x) * 3;
                r.rgb[p..p + 3].fill(0);
            }
        }
    }
    const P: VisualParams = VisualParams {
        threshold: 24,
        merge_gap_pt: 4.0,
        min_pixels: 4,
        scale: 1.0,
    };

    #[test]
    fn same_image_has_no_regions() {
        let mut a = blank(100, 100);
        paint(&mut a, 10, 10, 30, 30);
        assert!(diff_regions(&a, &a, &P).is_empty());
    }

    #[test]
    fn moved_square_gives_two_regions() {
        let (mut a, mut b) = (blank(200, 200), blank(200, 200));
        paint(&mut a, 10, 10, 40, 40);
        paint(&mut b, 120, 120, 150, 150);
        let r = diff_regions(&a, &b, &P);
        assert_eq!(r.len(), 2);
        assert!(r[0].rect.x0 <= 10.0 && r[0].rect.x1 >= 40.0);
        assert!(r[1].rect.x0 <= 120.0 && r[1].rect.y1 >= 150.0);
    }

    #[test]
    fn different_sizes_use_white_canvas() {
        let a = blank(50, 50);
        let mut b = blank(80, 50);
        paint(&mut b, 60, 10, 70, 20);
        assert_eq!(diff_regions(&a, &b, &P).len(), 1);
    }
}
