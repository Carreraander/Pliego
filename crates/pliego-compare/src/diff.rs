//! Diferencia por palabras (subsecuencia común más larga) y agrupación en cambios.

use std::collections::HashMap;

use crate::extract::Word;
use crate::model::{ChangeKind, Rect, TextChange};

/// Máximo de celdas de la tabla de programación dinámica (≈ 36 MB con `u16`).
const MAX_CELLS: usize = 18_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Equal(usize, usize),
    Del(usize),
    Ins(usize),
}

fn ops(a: &[u32], b: &[u32]) -> Vec<Op> {
    // Recorta prefijo y sufijo comunes.
    let pre = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let suf = a[pre..]
        .iter()
        .rev()
        .zip(b[pre..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let (am, bm) = (&a[pre..a.len() - suf], &b[pre..b.len() - suf]);
    let mut out: Vec<Op> = (0..pre).map(|i| Op::Equal(i, i)).collect();
    let (n, m) = (am.len(), bm.len());
    if n * m > MAX_CELLS || n.max(m) >= u16::MAX as usize {
        // Demasiado grande: se trata el bloque central como reemplazo completo.
        out.extend((0..n).map(|i| Op::Del(pre + i)));
        out.extend((0..m).map(|j| Op::Ins(pre + j)));
    } else {
        // lcs[i][j] = longitud de la subsecuencia común de am[i..] y bm[j..].
        let w = m + 1;
        let mut t = vec![0u16; (n + 1) * w];
        for i in (0..n).rev() {
            for j in (0..m).rev() {
                t[i * w + j] = if am[i] == bm[j] {
                    t[(i + 1) * w + j + 1] + 1
                } else {
                    t[(i + 1) * w + j].max(t[i * w + j + 1])
                };
            }
        }
        let (mut i, mut j) = (0, 0);
        while i < n || j < m {
            if i < n && j < m && am[i] == bm[j] {
                out.push(Op::Equal(pre + i, pre + j));
                i += 1;
                j += 1;
            } else if i < n && (j == m || t[(i + 1) * w + j] >= t[i * w + j + 1]) {
                out.push(Op::Del(pre + i));
                i += 1;
            } else {
                out.push(Op::Ins(pre + j));
                j += 1;
            }
        }
    }
    out.extend((0..suf).map(|k| Op::Equal(a.len() - suf + k, b.len() - suf + k)));
    out
}

/// Une rectángulos de palabras contiguas de la misma línea para dar una caja por línea.
pub(crate) fn merge_rects(words: &[&Word]) -> Vec<Rect> {
    let mut out: Vec<Rect> = Vec::new();
    for w in words {
        let r = w.rect;
        if let Some(last) = out.last_mut() {
            let overlap = (last.y1.min(r.y1) - last.y0.max(r.y0)).max(0.0);
            let h = last.height().min(r.height()).max(0.1);
            let gap = r.x0 - last.x1;
            if overlap / h > 0.5 && gap < h * 2.0 && gap > -h * 4.0 {
                *last = last.union(&r);
                continue;
            }
        }
        out.push(r);
    }
    out
}

fn join(words: &[&Word]) -> String {
    words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" ")
}

fn format_note(a: &Word, b: &Word) -> Option<String> {
    let mut parts = Vec::new();
    if a.font != b.font {
        parts.push(format!("fuente: {} → {}", or_unknown(&a.font), or_unknown(&b.font)));
    }
    if (a.size - b.size).abs() > 0.5 {
        parts.push(format!("tamaño: {:.1} → {:.1} pt", a.size, b.size));
    }
    (!parts.is_empty()).then(|| parts.join("; "))
}

fn or_unknown(s: &str) -> &str {
    if s.is_empty() { "desconocida" } else { s }
}

fn intern<'a>(words: &'a [Word], ids: &mut HashMap<&'a str, u32>) -> Vec<u32> {
    words
        .iter()
        .map(|w| {
            let next = ids.len() as u32;
            *ids.entry(w.text.as_str()).or_insert(next)
        })
        .collect()
}

/// Compara las palabras de dos páginas emparejadas y devuelve los cambios agrupados.
pub(crate) fn diff_words(wa: &[Word], wb: &[Word], page_a: u32, page_b: u32) -> Vec<TextChange> {
    let mut ids: HashMap<&str, u32> = HashMap::new();
    let ia = intern(wa, &mut ids);
    let ib = intern(wb, &mut ids);
    let all = ops(&ia, &ib);

    let mut changes = Vec::new();
    let (mut da, mut db): (Vec<&Word>, Vec<&Word>) = (Vec::new(), Vec::new());
    let mut fmt: Vec<(&Word, &Word)> = Vec::new();

    let flush_text = |da: &mut Vec<&Word>, db: &mut Vec<&Word>, changes: &mut Vec<TextChange>| {
        if da.is_empty() && db.is_empty() {
            return;
        }
        let kind = match (da.is_empty(), db.is_empty()) {
            (true, _) => ChangeKind::Added,
            (_, true) => ChangeKind::Removed,
            _ => ChangeKind::Changed,
        };
        changes.push(TextChange {
            kind,
            page_a,
            page_b,
            text_a: join(da),
            text_b: join(db),
            rects_a: merge_rects(da),
            rects_b: merge_rects(db),
            format_only: false,
            note: None,
        });
        da.clear();
        db.clear();
    };
    let flush_fmt = |fmt: &mut Vec<(&Word, &Word)>, changes: &mut Vec<TextChange>| {
        if fmt.is_empty() {
            return;
        }
        let a: Vec<&Word> = fmt.iter().map(|p| p.0).collect();
        let b: Vec<&Word> = fmt.iter().map(|p| p.1).collect();
        changes.push(TextChange {
            kind: ChangeKind::Changed,
            page_a,
            page_b,
            text_a: join(&a),
            text_b: join(&b),
            rects_a: merge_rects(&a),
            rects_b: merge_rects(&b),
            format_only: true,
            note: format_note(a[0], b[0]),
        });
        fmt.clear();
    };

    for op in all {
        match op {
            Op::Equal(i, j) => {
                flush_text(&mut da, &mut db, &mut changes);
                if format_note(&wa[i], &wb[j]).is_some() {
                    // Una palabra con otro formato: se agrupa solo con vecinas que cambian igual.
                    if let Some(&(pa, pb)) = fmt.last()
                        && format_note(pa, pb) != format_note(&wa[i], &wb[j])
                    {
                        flush_fmt(&mut fmt, &mut changes);
                    }
                    fmt.push((&wa[i], &wb[j]));
                } else {
                    flush_fmt(&mut fmt, &mut changes);
                }
            }
            Op::Del(i) => {
                flush_fmt(&mut fmt, &mut changes);
                da.push(&wa[i]);
            }
            Op::Ins(j) => {
                flush_fmt(&mut fmt, &mut changes);
                db.push(&wb[j]);
            }
        }
    }
    flush_text(&mut da, &mut db, &mut changes);
    flush_fmt(&mut fmt, &mut changes);
    changes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(t: &str, x: f32) -> Word {
        Word {
            text: t.into(),
            rect: Rect {
                x0: x,
                y0: 0.0,
                x1: x + 10.0,
                y1: 10.0,
            },
            font: "Helvetica".into(),
            size: 12.0,
        }
    }
    fn words(s: &str) -> Vec<Word> {
        s.split(' ').enumerate().map(|(i, t)| w(t, i as f32 * 12.0)).collect()
    }

    #[test]
    fn identical_has_no_changes() {
        assert!(diff_words(&words("a b c"), &words("a b c"), 0, 0).is_empty());
    }

    #[test]
    fn replaced_word_is_changed() {
        let c = diff_words(&words("pago en treinta dias"), &words("pago en sesenta dias"), 0, 0);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].kind, ChangeKind::Changed);
        assert_eq!((c[0].text_a.as_str(), c[0].text_b.as_str()), ("treinta", "sesenta"));
    }

    #[test]
    fn inserted_and_removed() {
        let c = diff_words(&words("a b c"), &words("a x y b c"), 0, 0);
        assert_eq!(c.len(), 1);
        assert_eq!((c[0].kind, c[0].text_b.as_str()), (ChangeKind::Added, "x y"));
        let c = diff_words(&words("a x y b c"), &words("a b c"), 0, 0);
        assert_eq!((c[0].kind, c[0].text_a.as_str()), (ChangeKind::Removed, "x y"));
    }

    #[test]
    fn format_only_change() {
        let a = words("hola mundo");
        let mut b = words("hola mundo");
        b[0].size = 24.0;
        b[1].size = 24.0;
        let c = diff_words(&a, &b, 0, 0);
        assert_eq!(c.len(), 1);
        assert!(c[0].format_only);
        assert_eq!(c[0].text_b, "hola mundo");
    }
}
