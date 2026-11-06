//! Recolocación del texto al editar (W43): cuando un párrafo gana o pierde líneas, lo que hay debajo sube o baja
//! con su aspecto intacto y, si ya no cabe en la página, pasa al principio de la siguiente (que se crea si no existe).
//!
//! Heurística (sin entender maquetas): la **columna** es la del párrafo editado; la **cadena** es lo que hay debajo
//! en esa columna (bloques de texto e imágenes) mientras la separación entre elementos no pase de unas ocho líneas
//! (o el 10 % del alto de la página). En una página de una sola columna la columna es todo el ancho del cuerpo, así
//! que un logo o una línea centrados también se mueven; si hay bloques lado a lado (varias columnas, tablas) se usa
//! solo la columna del párrafo. Los
//! encabezados (franja superior) y pies (franja inferior) no se mueven, y el límite de la página es el pie, o el
//! margen inferior de 36 pt. Lo movido no se recompone: se desplaza (véase `pliego_pdf::PdfDocument::move_content`).

use pliego_core::{Error, Report, Result};
use pliego_pdf::{ContentMove, PdfDocument, Rect, Size};

use crate::blocks::extract;
use crate::images::image_boxes;
use crate::load_page;
use crate::types::RectPts;

/// Cómo recolocar lo que rodea al texto editado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlowOptions {
    /// Subir o bajar lo que hay debajo del párrafo según gane o pierda líneas.
    pub push_below: bool,
    /// Lo que ya no cabe en la página pasa a la siguiente (creándola si hace falta).
    pub overflow_to_next_page: bool,
}

impl Default for FlowOptions {
    fn default() -> Self {
        Self {
            push_below: true,
            overflow_to_next_page: true,
        }
    }
}

impl FlowOptions {
    /// Sin recolocación: el texto nuevo se compone y nada más se mueve.
    pub const NONE: FlowOptions = FlowOptions {
        push_below: false,
        overflow_to_next_page: false,
    };
}

/// Franja superior (fracción del alto) que se considera encabezado.
const HEADER_BAND: f32 = 0.075;
/// A partir de esta fracción del alto empieza el pie de página.
const FOOTER_BAND: f32 = 0.92;
/// Margen inferior que se respeta si no hay pie.
const BOTTOM_MARGIN: f32 = 36.0;
/// Máximo de páginas por las que puede propagarse el desbordamiento.
const MAX_PAGES: usize = 50;

#[derive(Debug, Clone)]
struct Elem {
    rect: RectPts,
    /// Cajas de las líneas (vacío en imágenes).
    lines: Vec<RectPts>,
}

fn overlap_ratio(a: (f32, f32), b: (f32, f32)) -> f32 {
    let o = a.1.min(b.1) - a.0.max(b.0);
    let w = (a.1 - a.0).min(b.1 - b.0);
    if w <= 0.0 { 0.0 } else { (o / w).max(0.0) }
}

fn page_height(pdf: &PdfDocument, page: usize) -> Result<f32> {
    Ok(load_page(pdf, page)?.bounds()?.height())
}

/// Elementos de la columna `col` de la página, de arriba abajo.
fn column_elems(pdf: &PdfDocument, page: usize, col: (f32, f32)) -> Result<Vec<Elem>> {
    let p = load_page(pdf, page)?;
    let mut out: Vec<Elem> = extract(&p, page)?
        .into_iter()
        .map(|r| Elem {
            rect: r.block.bbox,
            lines: r.line_rects,
        })
        .collect();
    for rect in image_boxes(pdf, page)? {
        out.push(Elem {
            rect,
            lines: Vec::new(),
        });
    }
    out.retain(|e| overlap_ratio((e.rect[0], e.rect[2]), col) >= 0.6);
    out.sort_by(|a, b| a.rect[1].total_cmp(&b.rect[1]));
    Ok(out)
}

/// Parte alta del cuerpo de la página (primer elemento de la columna fuera de la franja de encabezado).
fn body_top(pdf: &PdfDocument, page: usize, col: (f32, f32)) -> Result<Option<f32>> {
    let h = page_height(pdf, page)?;
    Ok(column_elems(pdf, page, col)?
        .into_iter()
        .find(|e| e.rect[3] > HEADER_BAND * h && e.rect[1] < FOOTER_BAND * h)
        .map(|e| e.rect[1]))
}

/// Recoloca lo que hay bajo `y_start` en la columna `col` de la página, `dy` puntos (positivo = hacia abajo).
/// `reserve` es el hueco que hay que dejar al principio de la página siguiente (por delante de lo que desborde) para
/// el texto del propio párrafo que también pasa a ella; `inserted` son las páginas que ya se insertaron tras `page`
/// para tramos enteros de ese texto (la página que recibe lo que desborde es `page + 1 + inserted`). Devuelve dónde
/// quedó el texto del párrafo si algo desbordó, y `None` si no hubo desbordamiento.
#[allow(clippy::too_many_arguments)]
pub(crate) fn push_column(
    pdf: &mut PdfDocument,
    page: usize,
    col: (f32, f32),
    y_start: f32,
    dy: f32,
    leading: f32,
    default_top: f32,
    opts: &FlowOptions,
    depth: usize,
    reserve: f32,
    inserted: usize,
    report: &mut Report,
) -> Result<Option<Dest>> {
    if dy == 0.0 || depth > MAX_PAGES {
        return Ok(None);
    }
    let h = page_height(pdf, page)?;
    let gap_limit = (8.0 * leading).max(0.1 * h);
    let all = column_elems(pdf, page, col)?;
    // Cadena: lo que sigue a `y_start` sin huecos grandes, sin tocar el pie.
    let mut chain: Vec<Elem> = Vec::new();
    let mut prev = y_start;
    for e in all {
        if e.rect[3] <= y_start || e.rect[1] < y_start - 3.0 {
            continue;
        }
        if e.rect[1] >= FOOTER_BAND * h || e.rect[1] - prev > gap_limit {
            break;
        }
        prev = prev.max(e.rect[3]);
        chain.push(e);
    }
    if chain.is_empty() {
        return Ok(None);
    }
    let x0 = chain.iter().map(|e| e.rect[0]).fold(f32::MAX, f32::min) - 2.0;
    let x1 = chain.iter().map(|e| e.rect[2]).fold(f32::MIN, f32::max) + 2.0;
    let chain_end = chain.iter().map(|e| e.rect[3]).fold(f32::MIN, f32::max);

    // Límite inferior: el pie, o el margen; nunca por encima de donde ya llegaba el contenido.
    let limit = bottom_limit(pdf, page, col, leading)?.max(chain_end);

    // ¿Dónde se corta si no cabe? A mitad de la separación entre la última línea que cabe y la primera que no.
    let mut cut: Option<(f32, f32)> = None; // (y de corte, parte alta de lo que pasa)
    if dy > 0.0 && opts.overflow_to_next_page && chain_end + dy > limit {
        let mut before = y_start;
        'outer: for e in &chain {
            let parts: Vec<RectPts> = if e.lines.len() > 1 {
                e.lines.clone()
            } else {
                vec![e.rect]
            };
            for l in &parts {
                if l[3] + dy > limit {
                    cut = Some(((before + l[1]) / 2.0, l[1]));
                    break 'outer;
                }
                before = before.max(l[3]);
            }
        }
    }

    let Some((cut_y, over_top)) = cut else {
        let area = Rect::new(x0, y_start, x1, chain_end + 1.0);
        pdf.move_content(page as i32, &[ContentMove { area, dx: 0.0, dy }])?;
        return Ok(None);
    };

    // Desbordamiento: prepara la página que recibe lo que sobra, corta aquí, pega allí y baja lo que se queda.
    let next = page + 1 + inserted;
    let c2 = (x0 + 2.0, x1 - 2.0);
    let created = ensure_next_page(pdf, next - 1, report)?;
    let existing_top = body_top(pdf, next, c2)?.unwrap_or(default_top);
    let group_h = chain_end - over_top + leading;
    let over_h = group_h + reserve;
    let fresh = h - BOTTOM_MARGIN - default_top;
    if group_h > fresh {
        let msg = "lo que seguía al párrafo no cabe entero en una página y puede salirse de ella".to_string();
        if !report.warnings.contains(&msg) {
            report.warnings.push(msg);
        }
    }
    // Página del resto del párrafo y página de lo que lo seguía: la misma si caben juntos (empujando lo que ya había
    // arriba), o páginas nuevas intercaladas si no, para no solapar nada.
    let room = if created {
        fresh
    } else {
        bottom_limit(pdf, next, c2, leading)? - existing_top
    };
    let (tail, group, push_existing) = if over_h <= room {
        let d = Dest {
            page: next,
            top: existing_top,
        };
        (d, d, !created)
    } else {
        let blank_tail = Dest {
            page: next,
            top: default_top,
        };
        if over_h <= fresh {
            if !created {
                insert_blank_page(
                    pdf,
                    next - 1,
                    next,
                    report,
                    "lo que seguía no cabía en la página siguiente",
                )?;
            }
            (blank_tail, blank_tail, false)
        } else if group_h <= fresh {
            // El resto del párrafo en una página y lo que lo seguía en otra, intercaladas antes de lo que había.
            if !created {
                insert_blank_page(
                    pdf,
                    next - 1,
                    next,
                    report,
                    "el resto del párrafo ocupa casi una página",
                )?;
            }
            insert_blank_page(
                pdf,
                next,
                next + 1,
                report,
                "lo que seguía no cabía tras el resto del párrafo",
            )?;
            let g = Dest {
                page: next + 1,
                top: default_top,
            };
            (blank_tail, g, false)
        } else {
            (blank_tail, blank_tail, false)
        }
    };
    let same_page = tail.page == group.page;
    if push_existing {
        push_column(
            pdf,
            next,
            c2,
            existing_top,
            over_h,
            leading,
            default_top,
            opts,
            depth + 1,
            0.0,
            0,
            report,
        )?;
    }
    let cut_area = Rect::new(x0, cut_y, x1, chain_end + 1.0);
    if let Some(c) = pdf.cut_content(page as i32, cut_area)? {
        let lead = if same_page { reserve } else { 0.0 };
        let dy_paste = group.top + lead - c.bounds.y0;
        pdf.paste_content(group.page as i32, &c, 0.0, dy_paste)?;
    }
    if cut_y > y_start {
        let area = Rect::new(x0, y_start, x1, cut_y);
        pdf.move_content(page as i32, &[ContentMove { area, dx: 0.0, dy }])?;
    }
    Ok(Some(tail))
}

/// Dónde empieza en la página `page` el texto del párrafo que pasó a ella.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Dest {
    pub page: usize,
    pub top: f32,
}

/// Tramo de las líneas sobrantes del párrafo y dónde se dibuja.
#[derive(Debug, Clone)]
pub(crate) struct Placement {
    pub page: usize,
    pub top: f32,
    pub lines: std::ops::Range<usize>,
}

/// Inserta una página en blanco en `at` (mismo tamaño y giro que `like`) y lo avisa.
fn insert_blank_page(pdf: &mut PdfDocument, like: usize, at: usize, report: &mut Report, why: &str) -> Result<()> {
    let src = load_page(pdf, like)?;
    let m = src.media_box()?;
    let rotation = src.rotation()?;
    let mut np = pdf.new_page_at(at as i32, Size::new(m.x1 - m.x0, m.y1 - m.y0))?;
    if rotation != 0 {
        np.set_rotation(rotation)?;
    }
    report
        .warnings
        .push(format!("se insertó la página {} porque {why}", at + 1));
    Ok(())
}

/// Prepara la página siguiente a `page` para recibir texto: la crea (mismo tamaño y giro) si no existe y lo avisa.
/// Devuelve `true` si la creó.
fn ensure_next_page(pdf: &mut PdfDocument, page: usize, report: &mut Report) -> Result<bool> {
    let next = page + 1;
    let count = usize::try_from(pdf.page_count()?).unwrap_or(0);
    if next < count {
        let msg = format!(
            "parte del texto pasó al principio de la página {} (empujando lo que había)",
            next + 1
        );
        if !report.warnings.contains(&msg) {
            report.warnings.push(msg);
        }
        return Ok(false);
    }
    let src = load_page(pdf, page)?;
    let m = src.media_box()?;
    let rotation = src.rotation()?;
    let mut np = pdf.new_page_at(next as i32, Size::new(m.x1 - m.x0, m.y1 - m.y0))?;
    if rotation != 0 {
        np.set_rotation(rotation)?;
    }
    report.warnings.push(format!(
        "el texto desbordó la última página y se creó la página {}",
        next + 1
    ));
    Ok(true)
}

/// Hasta dónde (y, espacio de página) puede llegar el texto de la columna `col`: sobre el pie si lo hay, o el
/// margen inferior.
pub(crate) fn bottom_limit(pdf: &PdfDocument, page: usize, col: (f32, f32), leading: f32) -> Result<f32> {
    let h = page_height(pdf, page)?;
    let footer_top = column_elems(pdf, page, col)?
        .into_iter()
        .filter(|e| e.rect[1] >= FOOTER_BAND * h)
        .map(|e| e.rect[1])
        .fold(f32::MAX, f32::min);
    Ok(if footer_top < f32::MAX {
        footer_top - leading
    } else {
        h - BOTTOM_MARGIN
    })
}

/// Borde derecho de la columna de texto a la que pertenece `bbox`: lo más a la derecha que llegan los demás
/// bloques del cuerpo que empiezan a su altura de margen izquierdo. Si no hay ninguno, el propio borde.
pub(crate) fn column_right(pdf: &PdfDocument, page: usize, block_id: usize, bbox: RectPts) -> Result<f32> {
    let p = load_page(pdf, page)?;
    let h = p.bounds()?.height();
    let right = extract(&p, page)?
        .into_iter()
        .filter(|r| r.block.id != block_id)
        .map(|r| r.block.bbox)
        .filter(|b| b[3] > HEADER_BAND * h && b[1] < FOOTER_BAND * h)
        .filter(|b| (b[0] - bbox[0]).abs() <= 20.0 && overlap_ratio((b[0], b[2]), (bbox[0], bbox[2])) >= 0.6)
        .map(|b| b[2])
        .fold(bbox[2], f32::max);
    Ok(right)
}

/// Columna (x0, x1) que se recoloca al editar el bloque `bbox` cuyo texto nuevo llega hasta `right`. En una página de
/// una sola columna es todo el ancho del cuerpo (para que se muevan también las imágenes y líneas centradas); si hay
/// elementos lado a lado (varias columnas, tablas) es solo la del párrafo.
pub(crate) fn flow_column(pdf: &PdfDocument, page: usize, bbox: RectPts, right: f32) -> Result<(f32, f32)> {
    let strict = (bbox[0], right.max(bbox[2]));
    let p = load_page(pdf, page)?;
    let h = p.bounds()?.height();
    let mut boxes: Vec<RectPts> = extract(&p, page)?.into_iter().map(|r| r.block.bbox).collect();
    boxes.extend(image_boxes(pdf, page)?);
    boxes.retain(|b| b[3] > HEADER_BAND * h && b[1] < FOOTER_BAND * h);
    let side_by_side = boxes.iter().enumerate().any(|(i, a)| {
        boxes[i + 1..]
            .iter()
            .any(|b| overlap_ratio((a[1], a[3]), (b[1], b[3])) >= 0.3 && (a[2] + 12.0 <= b[0] || b[2] + 12.0 <= a[0]))
    });
    if side_by_side || boxes.is_empty() {
        return Ok(strict);
    }
    let x0 = boxes.iter().map(|b| b[0]).fold(strict.0, f32::min);
    let x1 = boxes.iter().map(|b| b[2]).fold(strict.1, f32::max);
    Ok((x0, x1))
}

/// Separación entre el bloque que acaba en `bottom` y lo siguiente de su columna (la que había entre párrafos), para
/// dejarla también entre el resto del párrafo que pasa a la página siguiente y lo que lo sigue.
pub(crate) fn gap_below(pdf: &PdfDocument, page: usize, col: (f32, f32), bottom: f32, leading: f32) -> Result<f32> {
    let next = column_elems(pdf, page, col)?
        .into_iter()
        .filter(|e| e.rect[1] >= bottom - 3.0)
        .map(|e| e.rect[1] - bottom)
        .fold(f32::MAX, f32::min);
    let fallback = 0.5 * leading;
    Ok(if next < f32::MAX {
        next.clamp(0.25 * leading, 3.0 * leading)
    } else {
        fallback
    })
}

/// Prepara el paso de `rest_lines` líneas del párrafo a las páginas siguientes y devuelve dónde va cada tramo. Lo
/// que cabe en páginas enteras va a páginas nuevas insertadas tras `page`; el último tramo va al principio de la página
/// que sigue (la crea si falta), con `gap_after` de separación antes de lo que había debajo del párrafo (`dy` desde
/// `y_start`) y empujando lo que hubiera ya arriba de esa página.
#[allow(clippy::too_many_arguments)]
pub(crate) fn make_room_on_next_pages(
    pdf: &mut PdfDocument,
    page: usize,
    col: (f32, f32),
    y_start: f32,
    dy: f32,
    leading: f32,
    rest_lines: usize,
    gap_after: f32,
    opts: &FlowOptions,
    report: &mut Report,
) -> Result<Vec<Placement>> {
    let h = page_height(pdf, page)?;
    let default_top = (h * 0.1).max(36.0);
    let cap = (((h - BOTTOM_MARGIN - default_top) / leading).floor().max(1.0)) as usize;
    let (mut left, mut full) = (rest_lines, 0usize);
    while left > cap {
        left -= cap;
        full += 1;
    }
    let mut out = Vec::new();
    let mut from = 0;
    for i in 0..full {
        insert_blank_page(
            pdf,
            page + i,
            page + 1 + i,
            report,
            "el texto sobrante ocupa páginas enteras",
        )?;
        out.push(Placement {
            page: page + 1 + i,
            top: default_top,
            lines: from..from + cap,
        });
        from += cap;
    }
    let rest_h = leading * left as f32 + gap_after;
    let used = push_column(
        pdf,
        page,
        col,
        y_start,
        dy,
        leading,
        default_top,
        opts,
        0,
        rest_h,
        full,
        report,
    )?;
    let dest = match used {
        Some(d) => d,
        None => {
            let src = page + full;
            let created = ensure_next_page(pdf, src, report)?;
            let top = body_top(pdf, src + 1, col)?.unwrap_or(default_top);
            if !created {
                push_column(
                    pdf,
                    src + 1,
                    col,
                    top,
                    rest_h,
                    leading,
                    default_top,
                    opts,
                    1,
                    0.0,
                    0,
                    report,
                )?;
            }
            Dest { page: src + 1, top }
        }
    };
    out.push(Placement {
        page: dest.page,
        top: dest.top,
        lines: from..rest_lines,
    });
    Ok(out)
}

/// Atajo para el párrafo editado: sube o baja lo de debajo `dy` puntos.
#[allow(clippy::too_many_arguments)]
pub(crate) fn push_below(
    pdf: &mut PdfDocument,
    page: usize,
    col: (f32, f32),
    y_start: f32,
    leading: f32,
    dy: f32,
    opts: &FlowOptions,
    report: &mut Report,
) -> Result<()> {
    if !opts.push_below {
        return Ok(());
    }
    let h = page_height(pdf, page)?;
    let default_top = (h * 0.1).max(36.0);
    if !(dy.is_finite() && leading.is_finite() && leading > 0.0) {
        return Err(Error::InvalidInput(
            "no se pudo calcular la recolocación del texto".into(),
        ));
    }
    push_column(
        pdf,
        page,
        col,
        y_start,
        dy,
        leading,
        default_top,
        opts,
        0,
        0.0,
        0,
        report,
    )
    .map(|_| ())
}
