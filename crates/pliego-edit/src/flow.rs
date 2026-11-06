//! Recolocación del texto al editar (W43): cuando un párrafo gana o pierde líneas, lo que hay debajo sube o baja
//! con su aspecto intacto y, si ya no cabe en la página, pasa al principio de la siguiente (que se crea si no existe).
//!
//! Heurística (sin entender maquetas): la **columna** es la del párrafo editado; la **cadena** es lo que hay debajo
//! en esa columna (bloques de texto e imágenes) mientras la separación entre elementos no pase de tres líneas. Los
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
/// Devuelve cuántas páginas más se tocaron.
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
    report: &mut Report,
) -> Result<()> {
    if dy == 0.0 || depth > MAX_PAGES {
        return Ok(());
    }
    let h = page_height(pdf, page)?;
    let gap_limit = (3.0 * leading).max(40.0);
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
        return Ok(());
    }
    let x0 = chain.iter().map(|e| e.rect[0]).fold(f32::MAX, f32::min) - 2.0;
    let x1 = chain.iter().map(|e| e.rect[2]).fold(f32::MIN, f32::max) + 2.0;
    let chain_end = chain.iter().map(|e| e.rect[3]).fold(f32::MIN, f32::max);

    // Límite inferior: el pie, o el margen; nunca por encima de donde ya llegaba el contenido.
    let footer_top = column_elems(pdf, page, col)?
        .into_iter()
        .filter(|e| e.rect[1] >= FOOTER_BAND * h)
        .map(|e| e.rect[1])
        .fold(f32::MAX, f32::min);
    let limit = if footer_top < f32::MAX {
        footer_top - leading
    } else {
        h - BOTTOM_MARGIN
    }
    .max(chain_end);

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
        return Ok(());
    };

    // Desbordamiento: prepara la página siguiente, corta aquí, pega allí y baja lo que se queda.
    let next = page + 1;
    let count = usize::try_from(pdf.page_count()?).unwrap_or(0);
    if next >= count {
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
    } else {
        report.warnings.push(format!(
            "parte del texto pasó al principio de la página {} (empujando lo que había)",
            next + 1
        ));
    }
    let top = body_top(pdf, next, (x0 + 2.0, x1 - 2.0))?.unwrap_or(default_top);
    let over_h = (chain_end - over_top) + leading;
    push_column(
        pdf,
        next,
        (x0 + 2.0, x1 - 2.0),
        top,
        over_h,
        leading,
        default_top,
        opts,
        depth + 1,
        report,
    )?;
    let cut_area = Rect::new(x0, cut_y, x1, chain_end + 1.0);
    if let Some(c) = pdf.cut_content(page as i32, cut_area)? {
        let dy_paste = top - c.bounds.y0;
        pdf.paste_content(next as i32, &c, 0.0, dy_paste)?;
    }
    if cut_y > y_start {
        let area = Rect::new(x0, y_start, x1, cut_y);
        pdf.move_content(page as i32, &[ContentMove { area, dx: 0.0, dy }])?;
    }
    Ok(())
}

/// Atajo para el párrafo editado: sube o baja lo de debajo `dy` puntos.
pub(crate) fn push_below(
    pdf: &mut PdfDocument,
    page: usize,
    block_bbox: RectPts,
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
        (block_bbox[0], block_bbox[2]),
        block_bbox[3],
        dy,
        leading,
        default_top,
        opts,
        0,
        report,
    )
}
