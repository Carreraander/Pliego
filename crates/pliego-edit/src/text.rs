//! Reemplazo e inserción de texto con reflujo.

use std::fmt::Write as _;

use pliego_core::{Error, Report, Result};
use pliego_document::Document;
use pliego_pdf::{
    Font, Matrix, PdfDocument, PdfPage, Rect, RedactImageMethod, RedactLineArtMethod, RedactOptions, RedactTextMethod,
};

use crate::blocks::extract;
use crate::embed::embed_subset;
use crate::fonts::{Metrics, base14_name, encode};
use crate::load_page;
use crate::original::{self, Original};
use crate::types::{Align, FontFamily, RectPts, Style};
use crate::unicode::{LoadedFont, Shaper, find_font, is_wide};

const DEFAULT_SIZE: f32 = 12.0;

/// Texto ya resuelto: fuente, tamaño y color finales.
struct Resolved {
    font: &'static str,
    size: f32,
    color: [f32; 3],
    align: Align,
}

fn resolve(style: &Style, base: Option<(&crate::TextBlock, ())>) -> Resolved {
    let (b_family, b_bold, b_italic, b_size, b_color) = match base {
        Some((b, _)) => (b.family, b.bold, b.italic, b.font_size, b.color),
        None => (FontFamily::Sans, false, false, DEFAULT_SIZE, [0.0; 3]),
    };
    Resolved {
        font: base14_name(
            style.family.unwrap_or(b_family),
            style.bold.unwrap_or(b_bold),
            style.italic.unwrap_or(b_italic),
        ),
        size: style.size.filter(|s| *s > 0.0).unwrap_or(b_size),
        color: style.color.unwrap_or(b_color),
        align: style.align,
    }
}

/// Fuente de composición: estándar (WinAnsi) o Unicode formada con rustybuzz.
enum Face<'a> {
    Std {
        name: &'static str,
        metrics: Metrics,
    },
    Uni {
        shaper: Box<Shaper<'a>>,
        bytes: &'a [u8],
    },
    /// La fuente que ya usa el bloque original.
    Orig(Original),
}

impl Face<'_> {
    fn width(&self, text: &str, size: f32) -> f32 {
        match self {
            Face::Std { metrics, .. } => metrics.width(text, size),
            Face::Uni { shaper, .. } => shaper.width(text, size),
            Face::Orig(o) => o.width(text, size).unwrap_or(0.0),
        }
    }

    /// Ascendente en fracción del tamaño.
    fn ascent(&self) -> f32 {
        match self {
            Face::Std { name, .. } => Font::new(name).map(|f| f.ascender().clamp(0.5, 1.2)).unwrap_or(0.8),
            Face::Uni { shaper, .. } => shaper.ascender().clamp(0.5, 1.2),
            Face::Orig(_) => 0.8,
        }
    }

    /// Prepara la fuente en la página para dibujar `lines`: nombre del recurso y, con fuente Unicode, el paso de
    /// glifo original a código del subconjunto incrustado.
    fn prepare(&self, page: &mut PdfPage, doc: &mut PdfDocument, lines: &[String], label: &str) -> Result<Prepared> {
        match self {
            Face::Std { name, .. } => Ok(Prepared {
                resource: page.insert_font(doc, name)?.0,
                codes: None,
            }),
            Face::Orig(o) => Ok(Prepared {
                resource: o.resource.clone(),
                codes: None,
            }),
            Face::Uni { bytes, .. } => {
                let e = embed_subset(doc, page, bytes, label, lines)?;
                Ok(Prepared {
                    resource: e.resource,
                    codes: Some(e.codes),
                })
            }
        }
    }

    /// Cadena hexadecimal del texto con los códigos que usa la fuente en el PDF.
    fn hex(&self, line: &str, prepared: &Prepared) -> Result<String> {
        Ok(match (self, &prepared.codes) {
            (Face::Orig(o), _) => o.hex(line),
            (Face::Uni { shaper, .. }, Some(codes)) => shaper
                .shape(line, 1.0)
                .glyphs
                .iter()
                .map(|(g, _)| format!("{:04X}", codes.get(g).copied().unwrap_or(0)))
                .collect(),
            _ => encode(line)?.iter().map(|b| format!("{b:02X}")).collect(),
        })
    }
}

/// Fuente lista en la página.
struct Prepared {
    resource: String,
    codes: Option<std::collections::HashMap<u16, u16>>,
}

/// Trozos de una línea: cada uno puede ir tras un espacio. Los ideogramas, kana y hangul se pueden partir
/// en cualquier punto aunque no haya espacios.
fn tokens(para: &str) -> Vec<(String, bool)> {
    let mut out: Vec<(String, bool)> = Vec::new();
    let (mut cur, mut space) = (String::new(), false);
    let flush = |cur: &mut String, space: &mut bool, out: &mut Vec<(String, bool)>| {
        if !cur.is_empty() {
            out.push((std::mem::take(cur), *space));
            *space = false;
        }
    };
    for c in para.chars() {
        if c.is_whitespace() {
            flush(&mut cur, &mut space, &mut out);
            space = true;
        } else if is_wide(c) {
            flush(&mut cur, &mut space, &mut out);
            out.push((c.to_string(), space));
            space = false;
        } else {
            cur.push(c);
        }
    }
    flush(&mut cur, &mut space, &mut out);
    out
}

/// Reparte el texto en líneas que caben en `max_width` (sin límite si es `None`).
/// Tolerancia (pt) al comparar anchuras: evita una línea extra cuando la más ancha mide justo el ancho del bloque.
const WRAP_TOLERANCE: f32 = 0.5;

fn wrap(text: &str, face: &Face, size: f32, max_width: Option<f32>) -> Vec<String> {
    let mut lines = Vec::new();
    for para in text.split('\n') {
        let Some(max) = max_width else {
            lines.push(para.to_owned());
            continue;
        };
        let mut cur = String::new();
        for (tok, space) in tokens(para) {
            let candidate = if cur.is_empty() {
                tok.clone()
            } else if space {
                format!("{cur} {tok}")
            } else {
                format!("{cur}{tok}")
            };
            if cur.is_empty() || face.width(&candidate, size) <= max + WRAP_TOLERANCE {
                cur = candidate;
            } else {
                lines.push(std::mem::take(&mut cur));
                cur = tok;
            }
        }
        lines.push(cur);
    }
    lines
}

fn pdf_num(v: f32) -> String {
    let s = format!("{v:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// Escribe las líneas como un flujo de contenido nuevo sobre la página. `origin` es la línea base de la
/// primera línea (espacio MuPDF) y `x0`/`x1` los límites horizontales para la alineación.
#[allow(clippy::too_many_arguments)]
fn draw(
    page: &mut PdfPage,
    doc: &mut PdfDocument,
    r: &Resolved,
    lines: &[String],
    x0: f32,
    x1: f32,
    baseline: f32,
    leading: f32,
    face: &Face,
    label: &str,
) -> Result<()> {
    let prepared = face.prepare(page, doc, lines, label)?;
    let font_res = &prepared.resource;
    let inv = page
        .object()
        .page_ctm()?
        .invert()
        .ok_or_else(|| Error::Pdf("la matriz de la página no es invertible".into()))?;
    let mut s = String::new();
    let [cr, cg, cb] = r.color;
    let _ = writeln!(s, "q\n{} {} {} rg\nBT", pdf_num(cr), pdf_num(cg), pdf_num(cb));
    let _ = writeln!(s, "{font_res} {} Tf", pdf_num(r.size));
    for (i, line) in lines.iter().enumerate() {
        if line.is_empty() {
            continue;
        }
        let w = face.width(line, r.size);
        let x = match r.align {
            Align::Left => x0,
            Align::Center => x0 + ((x1 - x0) - w) / 2.0,
            Align::Right => x1 - w,
        };
        // El texto se compone derecho en el espacio de la página visible (y hacia abajo); la matriz
        // de texto incluye la inversa de la página para que también salga derecho si está girada.
        let mut tm = Matrix::new(1.0, 0.0, 0.0, -1.0, x, baseline + leading * i as f32);
        tm.concat(inv);
        let hex = face.hex(line, &prepared)?;
        let _ = writeln!(
            s,
            "{} {} {} {} {} {} Tm <{hex}> Tj",
            pdf_num(tm.a),
            pdf_num(tm.b),
            pdf_num(tm.c),
            pdf_num(tm.d),
            pdf_num(tm.e),
            pdf_num(tm.f)
        );
    }
    s.push_str("ET\nQ\n");
    page.insert_contents(doc, s.as_bytes(), true)?;
    Ok(())
}

/// Carga la fuente Unicode si el texto no cabe en WinAnsi o si se indicó una fuente concreta.
fn load_unicode(text: &str, style: &Style) -> Result<Option<LoadedFont>> {
    if style.font_file.is_none() && encode(&text.replace('\n', "")).is_ok() {
        return Ok(None);
    }
    find_font(text, style.font_file.as_deref()).map(Some)
}

fn label(loaded: &Option<LoadedFont>) -> &str {
    loaded.as_ref().map_or("", |l| l.label.as_str())
}

fn make_face<'a>(res: &Resolved, loaded: &'a Option<LoadedFont>) -> Result<Face<'a>> {
    Ok(match loaded {
        Some(l) => Face::Uni {
            shaper: Box::new(Shaper::new(&l.data)?),
            bytes: &l.data,
        },
        None => Face::Std {
            name: res.font,
            metrics: Metrics::new(res.font)?,
        },
    })
}

fn font_warning(res: &Resolved, loaded: &Option<LoadedFont>, original: Option<&str>, text: &str) -> String {
    match loaded {
        Some(l) if encode(&text.replace('\n', "")).is_ok() => format!(
            "el texto se compuso con la fuente indicada «{}» (negrita y cursiva no se aplican)",
            l.label
        ),
        Some(l) => format!(
            "el texto usa caracteres que las fuentes estándar no tienen: se compuso con la fuente «{}» (negrita y cursiva no se aplican)",
            l.label
        ),
        None if original.is_some_and(|o| o.eq_ignore_ascii_case(res.font)) => {
            format!(
                "se compuso con la fuente estándar {}, la misma que el original",
                res.font
            )
        }
        None => format!(
            "se usó la fuente estándar {} en lugar de «{}»: el aspecto del texto puede variar ligeramente",
            res.font,
            original.unwrap_or("desconocida")
        ),
    }
}

fn rect_of(r: RectPts) -> Rect {
    Rect::new(r[0], r[1], r[2], r[3])
}

/// Reemplaza el texto del bloque `block` (id devuelto por [`crate::text_blocks`]) de la página `page`.
///
/// Se retira el texto original dentro del bloque (sin tocar imágenes ni gráficos) y se compone `new_text`
/// con reflujo dentro del ancho del bloque, empezando donde empezaba el original. Lo que queda fuera del
/// bloque no cambia. La fuente se sustituye por una estándar parecida y se avisa en el informe.
pub fn replace_text(doc: &mut Document, page: usize, block: usize, new_text: &str, style: &Style) -> Result<Report> {
    let mut report = Report::default();
    doc.with_pdf_mut(|pdf| {
        let p = load_page(pdf, page)?;
        let raw = extract(&p, page)?
            .into_iter()
            .find(|r| r.block.id == block)
            .ok_or_else(|| {
                Error::InvalidInput(format!("la página {} no tiene el bloque de texto {block}", page + 1))
            })?;
        let b = &raw.block;
        let res = resolve(style, Some((b, ())));
        // Si el estilo no pide otra fuente y la original tiene todos los caracteres, se compone con ella.
        let keep_font =
            style.font_file.is_none() && style.family.is_none() && style.bold.is_none() && style.italic.is_none();
        let original = match (&b.font_name, keep_font) {
            (Some(n), true) => original::find(&p, n)?.filter(|o| o.covers(new_text)),
            _ => None,
        };
        let reused = original.is_some();
        let loaded = if reused { None } else { load_unicode(new_text, style)? };
        let face = match original {
            Some(o) => Face::Orig(o),
            None => make_face(&res, &loaded)?,
        };

        // Retirar el texto original. El recuadro se encoge un poco para no arrastrar líneas vecinas.
        let inset_y = (b.font_size * 0.15).min((b.bbox[3] - b.bbox[1]) / 4.0);
        let area = rect_of([
            b.bbox[0] + 0.5,
            b.bbox[1] + inset_y,
            b.bbox[2] - 0.5,
            b.bbox[3] - inset_y,
        ]);
        let opts = RedactOptions {
            black_boxes: false,
            image_method: RedactImageMethod::None,
            line_art: RedactLineArtMethod::None,
            text: RedactTextMethod::Remove,
        };
        pdf.redact_areas(&[(page as i32, area)], opts)?;
        let mut p = load_page(pdf, page)?;

        let max_width = (b.bbox[2] - raw.first_origin.0).max(res.size);
        let lines = wrap(new_text, &face, res.size, Some(max_width));
        let leading = raw.leading.unwrap_or(res.size * 1.2);
        draw(
            &mut p,
            pdf,
            &res,
            &lines,
            raw.first_origin.0,
            b.bbox[2],
            raw.first_origin.1,
            leading,
            &face,
            label(&loaded),
        )?;

        let new_bottom = raw.first_origin.1 + leading * (lines.len().saturating_sub(1)) as f32;
        if new_bottom > b.bbox[3] + 0.5 {
            report.warnings.push(
                "el texto nuevo ocupa más líneas que el original y puede invadir el contenido que hay debajo".into(),
            );
        }
        if reused {
            report.warnings.push(format!(
                "se reutilizó la fuente original «{}»: el aspecto del texto se conserva",
                b.font_name.as_deref().unwrap_or("desconocida")
            ));
        } else {
            report
                .warnings
                .push(font_warning(&res, &loaded, b.font_name.as_deref(), new_text));
        }
        report.summary = format!("Texto del bloque {block} reemplazado en la página {}", page + 1);
        Ok(())
    })?;
    Ok(report)
}

/// Añade texto nuevo dentro de `rect` (espacio MuPDF) de la página `page`, con reflujo al ancho del recuadro.
pub fn insert_text(doc: &mut Document, page: usize, rect: RectPts, text: &str, style: &Style) -> Result<Report> {
    let loaded = load_unicode(text, style)?;
    if rect[2] <= rect[0] || rect[3] <= rect[1] {
        return Err(Error::InvalidInput("el recuadro del texto está vacío".into()));
    }
    let mut report = Report::default();
    doc.with_pdf_mut(|pdf| {
        let mut p = load_page(pdf, page)?;
        let res = resolve(style, None);
        let face = make_face(&res, &loaded)?;
        let lines = wrap(text, &face, res.size, Some(rect[2] - rect[0]));
        let ascent = face.ascent() * res.size;
        let leading = res.size * 1.2;
        draw(
            &mut p,
            pdf,
            &res,
            &lines,
            rect[0],
            rect[2],
            rect[1] + ascent,
            leading,
            &face,
            label(&loaded),
        )?;
        let bottom = rect[1] + ascent + leading * (lines.len().saturating_sub(1)) as f32;
        if bottom > rect[3] + 0.5 {
            report
                .warnings
                .push("el texto no cabe en el recuadro y se sale por abajo".into());
        }
        if loaded.is_some() {
            report.warnings.push(font_warning(&res, &loaded, None, text));
        }
        report.summary = format!("Texto añadido en la página {}", page + 1);
        Ok(())
    })?;
    Ok(report)
}
