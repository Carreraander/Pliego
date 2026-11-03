//! Emparejamiento de páginas entre dos versiones (páginas insertadas y eliminadas).

use std::collections::HashMap;

use crate::extract::PageData;

/// Entrada de la alineación: índices desde 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Slot {
    Pair(u32, u32),
    OnlyA(u32),
    OnlyB(u32),
}

fn bag(p: &PageData) -> HashMap<&str, u32> {
    let mut m = HashMap::new();
    for w in &p.words {
        *m.entry(w.text.as_str()).or_insert(0) += 1;
    }
    m
}

/// Parecido entre 0 y 1. Páginas con texto: coeficiente de Dice sobre bolsas de palabras.
/// Páginas casi sin texto (escaneos): solapamiento de las miniaturas.
pub(crate) fn similarity(a: &PageData, ba: &HashMap<&str, u32>, b: &PageData, bb: &HashMap<&str, u32>) -> f32 {
    match (&a.thumb, &b.thumb) {
        (Some(ta), Some(tb)) => {
            let (mut common, mut total) = (0u32, 0u32);
            for (x, y) in ta.iter().zip(tb) {
                common += (*x).min(*y) as u32;
                total += *x as u32 + *y as u32;
            }
            if total == 0 {
                // Dos páginas en blanco, con a lo sumo un par de palabras sueltas.
                return if a.words.iter().map(|w| &w.text).eq(b.words.iter().map(|w| &w.text)) {
                    1.0
                } else {
                    0.0
                };
            }
            2.0 * common as f32 / total as f32
        }
        (None, None) => {
            let common: u32 = ba.iter().map(|(k, v)| (*v).min(bb.get(k).copied().unwrap_or(0))).sum();
            let total = (a.words.len() + b.words.len()) as u32;
            2.0 * common as f32 / total.max(1) as f32
        }
        _ => 0.0,
    }
}

/// Alineación óptima (programación dinámica) que maximiza el parecido total de las páginas emparejadas.
/// Solo se emparejan páginas con parecido ≥ `threshold`.
pub(crate) fn align(a: &[PageData], b: &[PageData], threshold: f32) -> Vec<Slot> {
    let (n, m) = (a.len(), b.len());
    let bags_a: Vec<_> = a.iter().map(bag).collect();
    let bags_b: Vec<_> = b.iter().map(bag).collect();
    // Banda alrededor de la diagonal cuando hay muchas páginas, para acotar el coste.
    let band = if n * m > 250_000 {
        n.abs_diff(m) + 40
    } else {
        usize::MAX
    };
    let w = m + 1;
    let mut score = vec![0f32; (n + 1) * w];
    let mut sims = vec![0f32; (n + 1) * w];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            let mut best = score[(i + 1) * w + j].max(score[i * w + j + 1]);
            if i.abs_diff(j) <= band {
                let s = similarity(&a[i], &bags_a[i], &b[j], &bags_b[j]);
                sims[i * w + j] = s;
                if s >= threshold {
                    best = best.max(score[(i + 1) * w + j + 1] + s);
                }
            }
            score[i * w + j] = best;
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut out = Vec::new();
    while i < n || j < m {
        let s = if i < n && j < m { sims[i * w + j] } else { 0.0 };
        if i < n && j < m && s >= threshold && (score[i * w + j] - (score[(i + 1) * w + j + 1] + s)).abs() < 1e-4 {
            out.push(Slot::Pair(i as u32, j as u32));
            i += 1;
            j += 1;
        } else if i < n && (j == m || score[(i + 1) * w + j] >= score[i * w + j + 1]) {
            out.push(Slot::OnlyA(i as u32));
            i += 1;
        } else {
            out.push(Slot::OnlyB(j as u32));
            j += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::Word;
    use crate::model::Rect;

    fn page(text: &str) -> PageData {
        PageData {
            words: text
                .split(' ')
                .map(|t| Word {
                    text: t.into(),
                    rect: Rect {
                        x0: 0.0,
                        y0: 0.0,
                        x1: 1.0,
                        y1: 1.0,
                    },
                    font: String::new(),
                    size: 12.0,
                })
                .collect(),
            thumb: None,
        }
    }

    #[test]
    fn detects_inserted_and_deleted_pages() {
        let a = [
            page("uno dos tres cuatro"),
            page("cinco seis siete ocho"),
            page("nueve diez once doce"),
        ];
        let b = [
            page("uno dos tres cuatro"),
            page("zeta eta theta iota kappa"),
            page("cinco seis siete ocho"),
        ];
        let s = align(&a, &b, 0.5);
        assert_eq!(
            s,
            vec![Slot::Pair(0, 0), Slot::OnlyB(1), Slot::Pair(1, 2), Slot::OnlyA(2)]
        );
    }

    #[test]
    fn edited_page_still_pairs() {
        let a = [page("uno dos tres cuatro cinco seis")];
        let b = [page("uno dos tres cuatro cinco siete")];
        assert_eq!(align(&a, &b, 0.5), vec![Slot::Pair(0, 0)]);
    }
}
