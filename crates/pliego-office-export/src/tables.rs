//! Detección de tablas por alineación: filas consecutivas con varios fragmentos que comparten columnas.

use std::ops::Range;

use crate::lines::{Row, is_marker_only, union};
use crate::model::{Align, BBox, Cell, Run, Table};

const BAND_TOL: f32 = 0.5;

/// Intervalos de columnas (bandas) a partir de la proyección horizontal de los fragmentos.
fn bands(rows: &[Row], wide_limit: f32) -> Vec<(f32, f32)> {
    let mut ivs: Vec<(f32, f32)> = rows
        .iter()
        .flat_map(|r| r.frags.iter())
        .filter(|f| f.width() <= wide_limit)
        .map(|f| (f.bbox.x0, f.bbox.x1))
        .collect();
    ivs.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out: Vec<(f32, f32)> = Vec::new();
    for iv in ivs {
        match out.last_mut() {
            Some(last) if iv.0 <= last.1 + BAND_TOL => last.1 = last.1.max(iv.1),
            _ => out.push(iv),
        }
    }
    // Un encabezado alineado a la izquierda sobre cifras alineadas a la derecha deja dos bandas que nunca
    // coinciden en una misma fila: son la misma columna.
    loop {
        let occupied = |band: (f32, f32), row: &Row| {
            row.frags
                .iter()
                .any(|f| f.width() <= wide_limit && f.bbox.x0 < band.1 + BAND_TOL && f.bbox.x1 > band.0 - BAND_TOL)
        };
        let mut merged = false;
        for i in 0..out.len().saturating_sub(1) {
            let (a, b) = (out[i], out[i + 1]);
            let gap = b.0 - a.1;
            let a_rows: Vec<bool> = rows.iter().map(|r| occupied(a, r)).collect();
            let b_rows: Vec<bool> = rows.iter().map(|r| occupied(b, r)).collect();
            let disjoint = a_rows.iter().zip(&b_rows).all(|(x, y)| !(*x && *y));
            let a_n = a_rows.iter().filter(|x| **x).count();
            let b_n = b_rows.iter().filter(|x| **x).count();
            // Solo se unen cuando una de las dos aparece en una sola fila (el encabezado).
            if gap < 40.0 && disjoint && (a_n == 1 || b_n == 1) && a_n + b_n >= 3 {
                out[i].1 = b.1;
                out.remove(i + 1);
                merged = true;
                break;
            }
        }
        if !merged {
            break;
        }
    }
    out
}

fn marker_rows(rows: &[Row]) -> bool {
    let multi: Vec<&Row> = rows.iter().filter(|r| r.frags.len() >= 2).collect();
    let markers = multi.iter().filter(|r| is_marker_only(&r.frags[0].text())).count();
    !multi.is_empty() && markers * 10 >= multi.len() * 7
}

/// Rangos de filas que forman una tabla.
pub(crate) fn find_tables(rows: &[Row]) -> Vec<Range<usize>> {
    let n = rows.len();
    let multi: Vec<bool> = rows.iter().map(|r| r.frags.len() >= 2).collect();
    let mut found = Vec::new();
    let mut i = 0;
    while i < n {
        if !multi[i] {
            i += 1;
            continue;
        }
        let mut j = i + 1; // fin exclusivo
        while j < n {
            let gap = rows[j].bbox.y0 - rows[j - 1].bbox.y1;
            let h = (rows[j].bbox.y1 - rows[j].bbox.y0).max(rows[j - 1].bbox.y1 - rows[j - 1].bbox.y0);
            if gap > 1.2 * h + 4.0 {
                break;
            }
            let bridge =
                !multi[j] && j + 1 < n && multi[j + 1] && rows[j + 1].bbox.y0 - rows[j].bbox.y1 <= 1.2 * h + 4.0;
            if multi[j] || bridge {
                j += 1;
            } else {
                break;
            }
        }
        let run = &rows[i..j];
        let m = run.iter().filter(|r| r.frags.len() >= 2).count();
        let width = run.iter().map(|r| r.bbox.x1).fold(f32::MIN, f32::max)
            - run.iter().map(|r| r.bbox.x0).fold(f32::MAX, f32::min);
        let ncols = bands(run, 0.6 * width).len();
        let ok = ncols >= 2 && !marker_rows(run) && ((m >= 3) || (m >= 2 && ncols >= 3));
        if ok {
            found.push(i..j);
            i = j;
        } else {
            i += 1;
        }
    }
    found
}

fn is_number_like(s: &str) -> bool {
    let t = s.trim();
    !t.is_empty()
        && t.chars().any(|c| c.is_ascii_digit())
        && t.chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '.' | ',' | '-' | '+' | '%' | '€' | '$' | '£' | ' ' | '\u{a0}'))
}

/// Construye la tabla a partir de las filas de un rango devuelto por [`find_tables`].
pub(crate) fn build_table(rows: &[Row]) -> Table {
    let x0 = rows.iter().map(|r| r.bbox.x0).fold(f32::MAX, f32::min);
    let x1 = rows.iter().map(|r| r.bbox.x1).fold(f32::MIN, f32::max);
    let width = x1 - x0;
    let wide = 0.6 * width;
    let bands = bands(rows, wide);

    // Límites que teselan la tabla: puntos medios entre bandas.
    let mut cols = Vec::with_capacity(bands.len());
    for (k, b) in bands.iter().enumerate() {
        let l = if k == 0 { b.0 } else { (bands[k - 1].1 + b.0) / 2.0 };
        let r = if k + 1 == bands.len() {
            b.1
        } else {
            (b.1 + bands[k + 1].0) / 2.0
        };
        cols.push((l, r));
    }

    let mut out_rows = Vec::with_capacity(rows.len());
    let mut geo: Vec<Vec<Option<BBox>>> = Vec::new();
    for row in rows {
        let mut cells: Vec<Option<Cell>> = vec![None; bands.len()];
        let mut geo_row: Vec<Option<BBox>> = vec![None; bands.len()];
        let mut k = 0;
        while k < bands.len() {
            if cells[k].is_some() {
                k += 1;
                continue;
            }
            let here: Vec<_> = row
                .frags
                .iter()
                .filter(|f| {
                    let first = bands
                        .iter()
                        .position(|b| f.bbox.x0 < b.1 + BAND_TOL && f.bbox.x1 > b.0 - BAND_TOL)
                        .unwrap_or(0);
                    first == k
                })
                .collect();
            if here.is_empty() {
                k += 1;
                continue;
            }
            let mut runs: Vec<Run> = Vec::new();
            let mut bb: Option<BBox> = None;
            let mut last = k;
            for f in &here {
                if !runs.is_empty() {
                    let mut sp = runs.last().cloned().expect("hay texto");
                    sp.text = " ".into();
                    runs.push(sp);
                }
                runs.extend(f.runs());
                bb = Some(bb.map_or(f.bbox, |b| union(b, f.bbox)));
                let l = bands
                    .iter()
                    .rposition(|b| f.bbox.x0 < b.1 + BAND_TOL && f.bbox.x1 > b.0 - BAND_TOL)
                    .unwrap_or(k);
                last = last.max(l);
            }
            let bb = bb.expect("al menos un fragmento");
            let span = last - k + 1;
            geo_row[k] = (span == 1).then_some(bb);
            let align = Align::Left;
            cells[k] = Some(Cell { span, runs, align });
            for c in cells.iter_mut().take(last + 1).skip(k + 1) {
                *c = Some(Cell {
                    span: 0,
                    runs: Vec::new(),
                    align: Align::Left,
                });
            }
            k = last + 1;
        }
        let row_cells: Vec<Cell> = cells
            .into_iter()
            .map(|c| {
                c.unwrap_or(Cell {
                    span: 1,
                    runs: Vec::new(),
                    align: Align::Left,
                })
            })
            .filter(|c| c.span > 0)
            .collect();
        out_rows.push(row_cells);
        geo.push(geo_row);
    }
    align_columns(&mut out_rows, &geo, &bands);

    let mut row_heights = Vec::with_capacity(rows.len());
    for (i, r) in rows.iter().enumerate() {
        let h = match rows.get(i + 1) {
            Some(next) => next.bbox.y0 - r.bbox.y0,
            None => r.bbox.y1 - r.bbox.y0 + 2.0,
        };
        row_heights.push(h.max(4.0));
    }
    let bbox = rows.iter().map(|r| r.bbox).reduce(union).expect("tabla con filas");
    Table {
        bbox,
        cols,
        rows: out_rows,
        row_heights,
    }
}

/// Alineación por columna: gana el borde (izquierdo o derecho) con el que mejor coinciden los textos.
fn align_columns(rows: &mut [Vec<Cell>], geo: &[Vec<Option<BBox>>], bands: &[(f32, f32)]) {
    for (k, band) in bands.iter().enumerate() {
        let mut left_err = 0.0;
        let mut right_err = 0.0;
        let (mut numeric, mut total) = (0, 0);
        for (r, row) in rows.iter().enumerate() {
            let Some(bb) = geo[r][k] else { continue };
            left_err += (bb.x0 - band.0).abs();
            right_err += (bb.x1 - band.1).abs();
            if let Some(cell) = cell_at(row, k) {
                total += 1;
                numeric += usize::from(is_number_like(&cell.text()));
            }
        }
        let mostly_numeric = total > 0 && numeric * 10 >= total * 7;
        // Las cifras van a la derecha salvo que a la izquierda encajen claramente mejor.
        let right_col = if mostly_numeric {
            left_err + 2.0 * total as f32 >= right_err
        } else {
            right_err + 1.0 < left_err
        };
        if !right_col {
            continue;
        }
        for (r, row) in rows.iter_mut().enumerate() {
            let Some(bb) = geo[r][k] else { continue };
            let numeric = cell_at(row, k).is_some_and(|c| is_number_like(&c.text()));
            // Un título que está pegado al borde izquierdo de la columna se queda a la izquierda.
            if (numeric || (bb.x1 - band.1).abs() <= (bb.x0 - band.0).abs() + 3.0)
                && let Some(cell) = cell_at_mut(row, k)
            {
                cell.align = Align::Right;
            }
        }
    }
}

fn cell_index(row: &[Cell], col: usize) -> Option<usize> {
    let mut at = 0;
    for (i, c) in row.iter().enumerate() {
        if at == col {
            return Some(i);
        }
        at += c.span;
    }
    None
}

fn cell_at(row: &[Cell], col: usize) -> Option<&Cell> {
    cell_index(row, col).map(|i| &row[i])
}

fn cell_at_mut(row: &mut [Cell], col: usize) -> Option<&mut Cell> {
    cell_index(row, col).map(|i| &mut row[i])
}
