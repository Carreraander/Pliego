//! Análisis de maquetación: del texto estructurado de cada página al modelo [`Layout`].
//!
//! Pasos por página: fragmentos (líneas) → detección de dos columnas → filas → tablas → párrafos y listas →
//! imágenes. Entre páginas: encabezados y pies repetidos.

use std::collections::{HashMap, HashSet};

use image::{ExtendedColorType, ImageEncoder, codecs};
use mupdf::{Colorspace, Matrix};
use pliego_convert::{Block, StructuredPage};
use pliego_core::{Error, JobContext, PageRange, Result};

use crate::extract::structured_split;
use crate::lines::{Frag, cluster_rows, union};
use crate::model::*;
use crate::paragraphs::{ListCounter, Region, build_paragraphs};
use crate::tables::{build_table, find_tables};
use crate::util::{open_document, pdf_err};

/// Qué piezas necesita el escritor que consume el análisis.
#[derive(Debug, Clone, Copy)]
pub(crate) struct AnalyzeOptions {
    /// Separar encabezados y pies repetidos del cuerpo.
    pub headers: bool,
    /// Renderizar imágenes (necesario para DOCX y PPTX, no para XLSX).
    pub images: bool,
    /// Resolución con la que se renderizan las imágenes.
    pub image_dpi: f32,
}

const MAX_RENDER_PIXELS: f64 = 60_000_000.0;

fn text_frags(sp: &StructuredPage) -> Vec<Frag> {
    let mut out = Vec::new();
    for b in &sp.blocks {
        if let Block::Text { lines, .. } = b {
            for l in lines {
                if l.vertical || l.text().trim().is_empty() {
                    continue;
                }
                out.push(Frag {
                    bbox: l.bbox,
                    spans: l.spans.clone(),
                });
            }
        }
    }
    out
}

fn image_boxes(sp: &StructuredPage) -> Vec<BBox> {
    sp.blocks
        .iter()
        .filter_map(|b| match b {
            Block::Image { bbox } => Some(*bbox),
            _ => None,
        })
        .collect()
}

fn area(b: &BBox) -> f32 {
    (b.x1 - b.x0).max(0.0) * (b.y1 - b.y0).max(0.0)
}

/// Página escaneada: casi sin texto y con una imagen que cubre la mayor parte de la hoja.
pub(crate) fn is_scanned(sp: &StructuredPage) -> bool {
    let chars: usize = text_frags(sp)
        .iter()
        .map(|f| f.text().chars().filter(|c| !c.is_whitespace()).count())
        .sum();
    let page = sp.width * sp.height;
    chars < 5 && page > 0.0 && image_boxes(sp).iter().any(|b| area(b) >= 0.4 * page)
}

fn normalize_key(t: &str) -> String {
    let mut out = String::new();
    for c in t.split_whitespace().collect::<Vec<_>>().join(" ").chars() {
        out.push(if c.is_ascii_digit() { '#' } else { c });
    }
    out
}

#[derive(PartialEq, Clone, Copy)]
enum Zone {
    Header,
    Footer,
}

fn zone_of(f: &Frag, h: f32) -> Option<Zone> {
    if f.bbox.y1 <= 0.12 * h {
        Some(Zone::Header)
    } else if f.bbox.y0 >= 0.88 * h {
        Some(Zone::Footer)
    } else {
        None
    }
}

/// Claves de texto que se repiten en la zona superior o inferior de la mayoría de las páginas.
fn repeated_keys(pages: &[(&StructuredPage, Vec<Frag>)]) -> HashMap<(bool, String), usize> {
    let mut counts: HashMap<(bool, String), usize> = HashMap::new();
    for (sp, frags) in pages {
        let mut seen = HashSet::new();
        for f in frags {
            if let Some(z) = zone_of(f, sp.height) {
                let key = (z == Zone::Header, normalize_key(&f.text()));
                if seen.insert(key.clone()) {
                    *counts.entry(key).or_default() += 1;
                }
            }
        }
    }
    let n = pages.len();
    counts.retain(|_, c| *c >= 2 && *c * 10 >= n * 6);
    counts
}

const PAGE_WORDS: &[&str] = &["página", "pagina", "pág", "pag", "page", "p"];

/// Sustituye los números de página de un encabezado o pie por campos dinámicos.
fn apply_fields(p: &mut Paragraph, page_no: u32, total: u32) {
    let full = p.text();
    let mut tokens: Vec<(usize, usize)> = Vec::new();
    let mut start = None;
    for (i, c) in full.char_indices() {
        if c.is_whitespace() {
            if let Some(s) = start.take() {
                tokens.push((s, i));
            }
        } else if start.is_none() {
            start = Some(i);
        }
    }
    if let Some(s) = start {
        tokens.push((s, full.len()));
    }
    let mut targets: Vec<(usize, usize, Field)> = Vec::new();
    for (k, &(s, e)) in tokens.iter().enumerate() {
        let tok = &full[s..e];
        if !tok.chars().all(|c| c.is_ascii_digit()) || tok.starts_with('0') {
            continue;
        }
        let Ok(n) = tok.parse::<u32>() else { continue };
        let prev = k.checked_sub(1).map(|j| {
            full[tokens[j].0..tokens[j].1]
                .to_lowercase()
                .trim_end_matches(['.', ':'])
                .to_string()
        });
        let field = match prev.as_deref() {
            Some("de" | "of" | "/") => (n == total).then_some(Field::NumPages),
            Some(w) if PAGE_WORDS.contains(&w) => (n == page_no).then_some(Field::Page),
            None if tokens.len() == 1 => (n == page_no).then_some(Field::Page),
            _ => None,
        };
        if let Some(f) = field {
            targets.push((s, e, f));
        }
    }
    if targets.is_empty() {
        return;
    }
    let mut out: Vec<Run> = Vec::new();
    let mut offset = 0;
    for r in &p.runs {
        let (rs, re) = (offset, offset + r.text.len());
        offset = re;
        let mut cursor = rs;
        for &(s, e, f) in targets.iter().filter(|(s, e, _)| *s >= rs && *e <= re) {
            if s > cursor {
                out.push(Run {
                    text: full[cursor..s].to_string(),
                    ..r.clone()
                });
            }
            out.push(Run {
                text: full[s..e].to_string(),
                field: Some(f),
                ..r.clone()
            });
            cursor = e;
        }
        if cursor < re {
            out.push(Run {
                text: full[cursor..re].to_string(),
                ..r.clone()
            });
        }
    }
    p.runs = out;
}

/// Busca el hueco central entre dos columnas de texto. Devuelve sus límites en x.
fn detect_gutter(frags: &[Frag]) -> Option<(f32, f32)> {
    if frags.len() < 8 {
        return None;
    }
    let x0 = frags.iter().map(|f| f.bbox.x0).fold(f32::MAX, f32::min);
    let x1 = frags.iter().map(|f| f.bbox.x1).fold(f32::MIN, f32::max);
    let cw = x1 - x0;
    let narrow: Vec<&Frag> = frags.iter().filter(|f| f.width() < 0.55 * cw).collect();
    if narrow.len() < 8 {
        return None;
    }
    let mut ivs: Vec<(f32, f32)> = narrow.iter().map(|f| (f.bbox.x0, f.bbox.x1)).collect();
    ivs.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut merged: Vec<(f32, f32)> = Vec::new();
    for iv in ivs {
        match merged.last_mut() {
            Some(l) if iv.0 <= l.1 => l.1 = l.1.max(iv.1),
            _ => merged.push(iv),
        }
    }
    let (lo, hi) = (x0 + 0.25 * cw, x1 - 0.25 * cw);
    let (gl, gr) = merged
        .windows(2)
        .map(|w| (w[0].1, w[1].0))
        .filter(|(a, b)| *a >= lo && *b <= hi)
        .max_by(|a, b| (a.1 - a.0).total_cmp(&(b.1 - b.0)))?;
    if gr - gl < 8.0 {
        return None;
    }
    let left: Vec<&&Frag> = narrow.iter().filter(|f| f.bbox.x1 <= gl + 0.5).collect();
    let right: Vec<&&Frag> = narrow.iter().filter(|f| f.bbox.x0 >= gr - 0.5).collect();
    let n = narrow.len();
    if left.len() < 4 || right.len() < 4 || left.len() * 5 < n || right.len() * 5 < n {
        return None;
    }
    // Columnas de texto de verdad: las líneas llegan hasta el hueco, son largas (la mediana ocupa casi todo el
    // ancho de la columna) y las de la derecha empiezan todas en el mismo x. Una tabla de texto con huecos
    // no cumple esto.
    let lx0 = left.iter().map(|f| f.bbox.x0).fold(f32::MAX, f32::min);
    // Borde «típico» de la columna izquierda: el percentil 90 evita que una línea desbordada decida.
    let mut x1s: Vec<f32> = left.iter().map(|f| f.bbox.x1).collect();
    x1s.sort_by(|a, b| a.total_cmp(b));
    let edge = x1s[(x1s.len() * 9 / 10).min(x1s.len() - 1)];
    let reach = left.iter().filter(|f| f.bbox.x1 >= edge - 20.0).count();
    let mut widths: Vec<f32> = left.iter().map(|f| f.width()).collect();
    widths.sort_by(|a, b| a.total_cmp(b));
    let median = widths[widths.len() / 2];
    let rx0 = right.iter().map(|f| f.bbox.x0).fold(f32::MAX, f32::min);
    let aligned = right.iter().filter(|f| (f.bbox.x0 - rx0).abs() <= 2.0).count();
    (reach * 5 >= left.len() && median >= 0.7 * (edge - lx0) && aligned * 2 >= right.len()).then_some((gl, gr))
}

fn region_of(frags: &[Frag], edge: f32) -> Region {
    Region {
        left: frags.iter().map(|f| f.bbox.x0).fold(f32::MAX, f32::min),
        right: frags.iter().map(|f| f.bbox.x1).fold(f32::MIN, f32::max),
        edge,
    }
}

/// Filas → tablas y párrafos, en orden de lectura.
fn elements_from(frags: Vec<Frag>, edge: f32, counter: &mut ListCounter) -> Vec<Element> {
    if frags.is_empty() {
        return Vec::new();
    }
    let region = region_of(&frags, edge);
    let rows = cluster_rows(frags);
    let tables = find_tables(&rows);
    let mut out = Vec::new();
    let mut at = 0;
    for t in tables {
        out.extend(
            build_paragraphs(&rows[at..t.start], region, counter)
                .into_iter()
                .map(Element::Paragraph),
        );
        counter.reset_run();
        out.push(Element::Table(build_table(&rows[t.clone()])));
        at = t.end;
    }
    out.extend(
        build_paragraphs(&rows[at..], region, counter)
            .into_iter()
            .map(Element::Paragraph),
    );
    out
}

enum Segment {
    Spanning(Vec<Frag>),
    Columns(Vec<Frag>, Vec<Frag>),
    Single(Vec<Frag>),
}

fn segments(frags: Vec<Frag>, gutter: Option<(f32, f32)>) -> Vec<Segment> {
    let Some((gl, gr)) = gutter else {
        return vec![Segment::Single(frags)];
    };
    let (spanning, rest): (Vec<Frag>, Vec<Frag>) = frags.into_iter().partition(|f| f.bbox.x0 < gl && f.bbox.x1 > gr);
    let mut spanning = spanning;
    spanning.sort_by(|a, b| a.bbox.y0.total_cmp(&b.bbox.y0));
    let n = spanning.len();
    let mut bands: Vec<(Vec<Frag>, Vec<Frag>)> = (0..=n).map(|_| (Vec::new(), Vec::new())).collect();
    for f in rest {
        let cy = (f.bbox.y0 + f.bbox.y1) / 2.0;
        let b = spanning.iter().filter(|s| s.bbox.y1 <= cy).count();
        if (f.bbox.x0 + f.bbox.x1) / 2.0 < (gl + gr) / 2.0 {
            bands[b].0.push(f);
        } else {
            bands[b].1.push(f);
        }
    }
    let mut out: Vec<Segment> = Vec::new();
    for (b, (l, r)) in bands.into_iter().enumerate() {
        match (l.is_empty(), r.is_empty()) {
            (true, true) => {}
            (false, false) => out.push(Segment::Columns(l, r)),
            _ => out.push(Segment::Single(if l.is_empty() { r } else { l })),
        }
        if b < n {
            match out.last_mut() {
                Some(Segment::Spanning(v)) => v.push(spanning[b].clone()),
                _ => out.push(Segment::Spanning(vec![spanning[b].clone()])),
            }
        }
    }
    out
}

fn bounds(els: &[Element]) -> Option<BBox> {
    els.iter().map(|e| e.bbox()).reduce(union)
}

fn encode_region(img: &image::RgbImage, b: &BBox, scale: f32) -> Option<ImageData> {
    let (iw, ih) = img.dimensions();
    let x0 = ((b.x0 * scale).floor().max(0.0) as u32).min(iw.saturating_sub(1));
    let y0 = ((b.y0 * scale).floor().max(0.0) as u32).min(ih.saturating_sub(1));
    let x1 = ((b.x1 * scale).ceil() as u32).clamp(x0 + 1, iw);
    let y1 = ((b.y1 * scale).ceil() as u32).clamp(y0 + 1, ih);
    let (w, h) = (x1 - x0, y1 - y0);
    let crop = image::imageops::crop_imm(img, x0, y0, w, h).to_image();
    let mut out = Vec::new();
    let big = (w as u64) * (h as u64) > 400_000;
    let r = if big {
        codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 85).write_image(
            crop.as_raw(),
            w,
            h,
            ExtendedColorType::Rgb8,
        )
    } else {
        codecs::png::PngEncoder::new(&mut out).write_image(crop.as_raw(), w, h, ExtendedColorType::Rgb8)
    };
    r.ok()?;
    Some(ImageData {
        bbox: *b,
        data: out,
        ext: if big { "jpeg" } else { "png" },
        px: (w, h),
    })
}

fn render_page(doc: &mupdf::Document, page: u32, w: f32, h: f32, dpi: f32) -> Result<(image::RgbImage, f32)> {
    let mut scale = dpi / 72.0;
    let px = (w as f64 * scale as f64) * (h as f64 * scale as f64);
    if px > MAX_RENDER_PIXELS {
        scale *= (MAX_RENDER_PIXELS / px).sqrt() as f32;
    }
    let p = doc.load_page(page as i32).map_err(pdf_err)?;
    let pix = p
        .to_pixmap(&Matrix::new_scale(scale, scale), &Colorspace::device_rgb(), false, true)
        .map_err(pdf_err)?;
    let (pw, ph, n, stride) = (
        pix.width() as usize,
        pix.height() as usize,
        pix.n() as usize,
        pix.stride() as usize,
    );
    if n != 3 {
        return Err(Error::Pdf(format!("mapa de bits inesperado de {n} canales")));
    }
    let s = pix.samples();
    let mut data = Vec::with_capacity(pw * ph * 3);
    for y in 0..ph {
        data.extend_from_slice(&s[y * stride..y * stride + pw * 3]);
    }
    let img = image::RgbImage::from_raw(pw as u32, ph as u32, data)
        .ok_or_else(|| Error::Pdf("mapa de bits con tamaño inconsistente".into()))?;
    Ok((img, scale))
}

fn insert_image(sections: &mut [Section], img: ImageData) {
    let cy = (img.bbox.y0 + img.bbox.y1) / 2.0;
    let cx = (img.bbox.x0 + img.bbox.x1) / 2.0;
    let target = sections
        .iter()
        .position(|s| {
            s.content
                .iter()
                .filter_map(|c| bounds(c))
                .map(|b| b.y1)
                .fold(f32::MIN, f32::max)
                >= cy
        })
        .unwrap_or(sections.len() - 1);
    let s = &mut sections[target];
    let col = if s.columns.len() == 2 && cx > (s.columns[0].x1 + s.columns[1].x0) / 2.0 {
        1
    } else {
        0
    };
    let list = &mut s.content[col];
    let at = list
        .iter()
        .position(|e| e.bbox().y0 > img.bbox.y0)
        .unwrap_or(list.len());
    list.insert(at, Element::Image(img));
}

struct PageInput<'a> {
    sp: &'a StructuredPage,
    frags: Vec<Frag>,
}

#[allow(clippy::too_many_arguments)]
fn layout_page(
    doc: &mupdf::Document,
    input: PageInput,
    scanned: bool,
    opts: &AnalyzeOptions,
    counter: &mut ListCounter,
) -> Result<PageLayout> {
    let sp = input.sp;
    let (w, h) = (sp.width, sp.height);
    let blank = |scanned| PageLayout {
        index: sp.page,
        width: w,
        height: h,
        margins: Margins {
            left: 0.0,
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
        },
        sections: Vec::new(),
        scanned,
    };

    if scanned {
        let mut page = blank(true);
        if opts.images {
            let (img, scale) = render_page(doc, sp.page, w, h, opts.image_dpi)?;
            let full = BBox {
                x0: 0.0,
                y0: 0.0,
                x1: w,
                y1: h,
            };
            if let Some(data) = encode_region(&img, &full, scale) {
                page.sections.push(Section {
                    columns: vec![ColumnSpec { x0: 0.0, x1: w }],
                    content: vec![vec![Element::Image(data)]],
                    top: 0.0,
                });
            }
        }
        return Ok(page);
    }

    let frags = input.frags;
    let edge = frags.iter().map(|f| f.bbox.x1).fold(0.0, f32::max);
    // Encabezados y pies (números de página, etc.) no cuentan para buscar el hueco entre columnas.
    let body: Vec<Frag> = frags.iter().filter(|f| zone_of(f, h).is_none()).cloned().collect();
    let gutter = detect_gutter(&body);
    let mut sections: Vec<Section> = Vec::new();
    for seg in segments(frags, gutter) {
        match seg {
            Segment::Spanning(f) | Segment::Single(f) => {
                let els = elements_from(f, edge, counter);
                if let Some(b) = bounds(&els) {
                    sections.push(Section {
                        columns: vec![ColumnSpec { x0: b.x0, x1: b.x1 }],
                        content: vec![els],
                        top: 0.0,
                    });
                }
            }
            Segment::Columns(l, r) => {
                let (le, re) = (elements_from(l, edge, counter), elements_from(r, edge, counter));
                if let (Some(lb), Some(rb)) = (bounds(&le), bounds(&re)) {
                    sections.push(Section {
                        columns: vec![ColumnSpec { x0: lb.x0, x1: lb.x1 }, ColumnSpec { x0: rb.x0, x1: rb.x1 }],
                        content: vec![le, re],
                        top: 0.0,
                    });
                }
            }
        }
    }

    if opts.images {
        let page_area = w * h;
        let boxes: Vec<BBox> = image_boxes(sp)
            .into_iter()
            // Una imagen que cubre casi toda la hoja es un fondo, no un elemento del contenido.
            .filter(|b| area(b) < 0.7 * page_area && area(b) > 4.0)
            .collect();
        if !boxes.is_empty() {
            let (img, scale) = render_page(doc, sp.page, w, h, opts.image_dpi)?;
            if sections.is_empty() {
                sections.push(Section {
                    columns: vec![ColumnSpec { x0: 0.0, x1: w }],
                    content: vec![Vec::new()],
                    top: 0.0,
                });
            }
            for b in &boxes {
                if let Some(data) = encode_region(&img, b, scale) {
                    insert_image(&mut sections, data);
                }
            }
        }
    }

    // Márgenes: del contenido real.
    let all: Vec<BBox> = sections
        .iter()
        .flat_map(|s| s.content.iter().flatten())
        .map(|e| e.bbox())
        .collect();
    let content = all.iter().copied().reduce(union);
    let margins = match content {
        Some(c) => Margins {
            left: c.x0.max(0.0),
            top: c.y0.max(0.0),
            // 6 pt de holgura a la derecha: sin ella, una línea que llega justo al margen se parte distinto en Word.
            right: (w - c.x1 - 6.0).max(0.0),
            bottom: (h - c.y1 - 6.0).clamp(6.0, 36.0),
        },
        None => Margins {
            left: 36.0,
            top: 36.0,
            right: 36.0,
            bottom: 36.0,
        },
    };
    let mut cursor = margins.top;
    for s in &mut sections {
        s.top = cursor;
        let bottom = s
            .content
            .iter()
            .filter_map(|c| bounds(c))
            .map(|b| b.y1)
            .fold(cursor, f32::max);
        cursor = bottom;
    }
    let mut page = blank(false);
    page.margins = margins;
    page.sections = sections;
    Ok(page)
}

fn band_from(
    frags: Vec<Frag>,
    zone: Zone,
    page: (&StructuredPage, u32),
    edge: f32,
    counter: &mut ListCounter,
) -> Option<Band> {
    if frags.is_empty() {
        return None;
    }
    let (sp, total) = page;
    let region = region_of(&frags, edge);
    let rows = cluster_rows(frags);
    let mut paragraphs = build_paragraphs(&rows, region, counter);
    for p in &mut paragraphs {
        apply_fields(p, sp.page + 1, total);
    }
    let b = paragraphs.iter().map(|p| p.bbox).reduce(union)?;
    let distance = match zone {
        Zone::Header => b.y0,
        Zone::Footer => sp.height - b.y1,
    };
    Some(Band {
        paragraphs,
        distance: distance.max(0.0),
    })
}

/// Analiza las páginas indicadas de `input`.
pub(crate) fn analyze(
    input: &std::path::Path,
    password: Option<&str>,
    pages: &PageRange,
    opts: &AnalyzeOptions,
    ctx: &JobContext,
) -> Result<Layout> {
    let doc = open_document(input, password)?;
    let count = doc.page_count().map_err(pdf_err)? as u32;
    let selected = pages.resolve(count)?;
    let mut sps = Vec::with_capacity(selected.len());
    for (n, &p) in selected.iter().enumerate() {
        ctx.step(
            n as u64,
            selected.len() as u64 * 2,
            &format!("leyendo la página {}", p + 1),
        )?;
        sps.push(structured_split(&doc, p)?);
    }
    let scanned: Vec<bool> = sps.iter().map(is_scanned).collect();
    let mut layout = Layout::default();
    let mut counter = ListCounter::default();
    let mut all_frags: Vec<Vec<Frag>> = sps.iter().map(text_frags).collect();

    if opts.headers {
        let body: Vec<(&StructuredPage, Vec<Frag>)> = sps
            .iter()
            .zip(&all_frags)
            .zip(&scanned)
            .filter(|(_, s)| !**s)
            .map(|((sp, f), _)| (sp, f.clone()))
            .collect();
        let keys = repeated_keys(&body);
        if !keys.is_empty() {
            let edge = all_frags.iter().flatten().map(|f| f.bbox.x1).fold(0.0, f32::max);
            let (mut header_done, mut footer_done) = (false, false);
            for (i, frags) in all_frags.iter_mut().enumerate() {
                let sp = &sps[i];
                let (mut hs, mut fs) = (Vec::new(), Vec::new());
                frags.retain(|f| match zone_of(f, sp.height) {
                    Some(z) if keys.contains_key(&(z == Zone::Header, normalize_key(&f.text()))) => {
                        if z == Zone::Header {
                            hs.push(f.clone());
                        } else {
                            fs.push(f.clone());
                        }
                        false
                    }
                    _ => true,
                });
                if !header_done && !hs.is_empty() {
                    header_done = true;
                    layout.header = band_from(hs, Zone::Header, (sp, count), edge, &mut counter);
                }
                if !footer_done && !fs.is_empty() {
                    footer_done = true;
                    layout.footer = band_from(fs, Zone::Footer, (sp, count), edge, &mut counter);
                }
            }
        }
    }

    let total = sps.len() as u64;
    let mut scanned_pages = Vec::new();
    for (i, sp) in sps.iter().enumerate() {
        ctx.step(
            total + i as u64,
            total * 2,
            &format!("analizando la página {}", sp.page + 1),
        )?;
        if scanned[i] {
            scanned_pages.push(sp.page + 1);
        }
        let frags = std::mem::take(&mut all_frags[i]);
        layout.pages.push(layout_page(
            &doc,
            PageInput { sp, frags },
            scanned[i],
            opts,
            &mut counter,
        )?);
    }
    if !scanned_pages.is_empty() {
        let list = scanned_pages.iter().map(u32::to_string).collect::<Vec<_>>().join(", ");
        layout.warnings.push(format!(
            "Páginas escaneadas sin texto reconocible ({list}): se exportan como imagen. \
             Aplica OCR al PDF antes de exportar para obtener texto editable."
        ));
    }
    ctx.step(total * 2, total * 2, "listo")?;
    Ok(layout)
}

/// Páginas (desde 1) que son solo una imagen, sin texto.
pub(crate) fn scanned_pages(input: &std::path::Path, password: Option<&str>) -> Result<Vec<u32>> {
    let doc = open_document(input, password)?;
    let count = doc.page_count().map_err(pdf_err)? as u32;
    let mut out = Vec::new();
    for p in 0..count {
        if is_scanned(&structured_split(&doc, p)?) {
            out.push(p + 1);
        }
    }
    Ok(out)
}
