//! Informe en PDF: página de resumen y, por cada página con diferencias, las dos versiones
//! una junto a otra con los cambios resaltados (añadido verde, borrado rojo, cambiado ámbar).

use std::io::Write;
use std::path::Path;

use flate2::Compression;
use flate2::write::ZlibEncoder;
use pliego_core::{Error, JobContext, Lang, Result};

use crate::extract::{self, Raster};
use crate::model::{ChangeKind, Comparison, PageComparison, PageStatus, Rect};

const GREEN: (f32, f32, f32) = (0.10, 0.65, 0.25);
const RED: (f32, f32, f32) = (0.85, 0.12, 0.12);
const AMBER: (f32, f32, f32) = (0.96, 0.62, 0.04);
const GREY: (f32, f32, f32) = (0.45, 0.45, 0.45);
/// Resolución de las imágenes de las páginas en el informe.
const REPORT_DPI: f32 = 100.0;

/// Escritor mínimo de PDF: objetos numerados, `xref` clásico, sin dependencias de MuPDF.
struct PdfOut {
    buf: Vec<u8>,
    offsets: Vec<usize>,
}

impl PdfOut {
    fn new() -> Self {
        let mut buf = b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n".to_vec();
        buf.reserve(1 << 16);
        Self {
            buf,
            offsets: Vec::new(),
        }
    }
    fn alloc(&mut self) -> u32 {
        self.offsets.push(0);
        self.offsets.len() as u32
    }
    fn begin(&mut self, id: u32) {
        self.offsets[id as usize - 1] = self.buf.len();
        self.buf.extend_from_slice(format!("{id} 0 obj\n").as_bytes());
    }
    fn object(&mut self, id: u32, body: &str) {
        self.begin(id);
        self.buf.extend_from_slice(body.as_bytes());
        self.buf.extend_from_slice(b"\nendobj\n");
    }
    fn stream(&mut self, id: u32, dict: &str, data: &[u8]) {
        self.begin(id);
        self.buf
            .extend_from_slice(format!("<< {dict} /Length {} >>\nstream\n", data.len()).as_bytes());
        self.buf.extend_from_slice(data);
        self.buf.extend_from_slice(b"\nendstream\nendobj\n");
    }
    fn finish(mut self, root: u32, info: u32) -> Vec<u8> {
        let xref = self.buf.len();
        let n = self.offsets.len() + 1;
        self.buf
            .extend_from_slice(format!("xref\n0 {n}\n0000000000 65535 f \n").as_bytes());
        for off in &self.offsets {
            self.buf.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
        }
        self.buf.extend_from_slice(
            format!("trailer\n<< /Size {n} /Root {root} 0 R /Info {info} 0 R >>\nstartxref\n{xref}\n%%EOF\n")
                .as_bytes(),
        );
        self.buf
    }
}

fn deflate(data: &[u8]) -> Result<Vec<u8>> {
    let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
    e.write_all(data)?;
    Ok(e.finish()?)
}

/// Cadena PDF en WinAnsi (Latin-1 para el español); lo que no cabe se sustituye por `?`.
fn pdf_text(s: &str) -> Vec<u8> {
    let mut out = vec![b'('];
    for c in s.chars() {
        let b = if (32..256).contains(&(c as u32)) {
            c as u32 as u8
        } else {
            b'?'
        };
        if matches!(b, b'(' | b')' | b'\\') {
            out.push(b'\\');
        }
        out.push(b);
    }
    out.push(b')');
    out
}

/// Cadena de texto Unicode para el diccionario de información (UTF-16BE con marca).
fn pdf_unicode(s: &str) -> String {
    let mut out = String::from("<FEFF");
    for u in s.encode_utf16() {
        out.push_str(&format!("{u:04X}"));
    }
    out.push('>');
    out
}

/// Contenido de una página: operadores acumulados.
struct Canvas {
    ops: Vec<u8>,
    images: Vec<(String, u32)>,
}

impl Canvas {
    fn new() -> Self {
        Self {
            ops: Vec::new(),
            images: Vec::new(),
        }
    }
    fn raw(&mut self, s: &str) {
        self.ops.extend_from_slice(s.as_bytes());
    }
    fn text(&mut self, x: f32, y: f32, size: f32, bold: bool, color: (f32, f32, f32), s: &str) {
        let font = if bold { "F2" } else { "F1" };
        self.raw(&format!(
            "BT /{font} {size} Tf {:.3} {:.3} {:.3} rg {x:.2} {y:.2} Td ",
            color.0, color.1, color.2
        ));
        self.ops.extend(pdf_text(s));
        self.raw(" Tj ET\n");
    }
    fn fill_rect(&mut self, r: (f32, f32, f32, f32), c: (f32, f32, f32), alpha_gs: bool) {
        self.raw(if alpha_gs { "q /GSa gs " } else { "q " });
        self.raw(&format!(
            "{:.3} {:.3} {:.3} rg {:.2} {:.2} {:.2} {:.2} re f Q\n",
            c.0, c.1, c.2, r.0, r.1, r.2, r.3
        ));
    }
    fn stroke_rect(&mut self, r: (f32, f32, f32, f32), c: (f32, f32, f32), width: f32) {
        self.raw(&format!(
            "q {:.3} {:.3} {:.3} RG {width} w {:.2} {:.2} {:.2} {:.2} re S Q\n",
            c.0, c.1, c.2, r.0, r.1, r.2, r.3
        ));
    }
    fn image(&mut self, id: u32, x: f32, y: f32, w: f32, h: f32) {
        let name = format!("Im{}", self.images.len());
        self.raw(&format!("q {w:.2} 0 0 {h:.2} {x:.2} {y:.2} cm /{name} Do Q\n"));
        self.images.push((name, id));
    }
}

struct Doc {
    out: PdfOut,
    pages_id: u32,
    font: u32,
    font_bold: u32,
    kids: Vec<u32>,
}

impl Doc {
    fn new() -> Self {
        let mut out = PdfOut::new();
        let root = out.alloc();
        let pages_id = out.alloc();
        let font = out.alloc();
        let font_bold = out.alloc();
        debug_assert_eq!(root, 1);
        out.object(
            font,
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>",
        );
        out.object(
            font_bold,
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>",
        );
        Self {
            out,
            pages_id,
            font,
            font_bold,
            kids: Vec::new(),
        }
    }

    fn add_image(&mut self, r: &Raster) -> Result<u32> {
        let id = self.out.alloc();
        let data = deflate(&r.rgb)?;
        let dict = format!(
            "/Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode",
            r.width, r.height
        );
        self.out.stream(id, &dict, &data);
        Ok(id)
    }

    fn add_page(&mut self, w: f32, h: f32, c: Canvas) -> Result<()> {
        let content = self.out.alloc();
        let data = deflate(&c.ops)?;
        self.out.stream(content, "/Filter /FlateDecode", &data);
        let page = self.out.alloc();
        let xobjs: String = c.images.iter().map(|(n, id)| format!("/{n} {id} 0 R ")).collect();
        let body = format!(
            "<< /Type /Page /Parent {} 0 R /MediaBox [0 0 {w} {h}] /Contents {content} 0 R /Resources << /Font << /F1 {} 0 R /F2 {} 0 R >> \
             /ExtGState << /GSa << /ca 0.30 >> >> /XObject << {xobjs}>> >> >>",
            self.pages_id, self.font, self.font_bold
        );
        self.out.object(page, &body);
        self.kids.push(page);
        Ok(())
    }

    fn finish(mut self, title: &str) -> Vec<u8> {
        let kids: String = self.kids.iter().map(|k| format!("{k} 0 R ")).collect();
        self.out.object(
            self.pages_id,
            &format!("<< /Type /Pages /Kids [{kids}] /Count {} >>", self.kids.len()),
        );
        self.out
            .object(1, &format!("<< /Type /Catalog /Pages {} 0 R >>", self.pages_id));
        let info = self.out.alloc();
        self.out.object(
            info,
            &format!(
                "<< /Title {} /Producer (Pliego) /Creator (Pliego) >>",
                pdf_unicode(title)
            ),
        );
        self.out.finish(1, info)
    }
}

fn truncate(s: &str, max: usize) -> String {
    let s: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if s.chars().count() > max {
        let cut: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{cut}…")
    } else {
        s
    }
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.display().to_string())
}

/// Entrecomilla un fragmento: «así» en español y "así" en inglés (el PDF usa WinAnsi sin comillas tipográficas inglesas).
fn quoted(lang: Lang, s: &str) -> String {
    match lang {
        Lang::Es => format!("«{s}»"),
        Lang::En => format!("\"{s}\""),
    }
}

fn summary_lines(c: &Comparison, lang: Lang) -> Vec<(bool, (f32, f32, f32), String)> {
    let mut lines = Vec::new();
    for p in &c.pages {
        let pg = lang.pick("Pág.", "Page");
        let label = match (p.page_a, p.page_b) {
            (Some(a), Some(b)) => format!("{pg} {} → {}", a + 1, b + 1),
            (Some(a), None) => format!("{pg} {} ({})", a + 1, lang.pick("original", "original")),
            (None, Some(b)) => format!("{pg} {} ({})", b + 1, lang.pick("nueva", "new")),
            (None, None) => String::new(),
        };
        match p.status {
            PageStatus::Inserted => lines.push((
                false,
                GREEN,
                format!("{label}: {}", lang.pick("página insertada", "page inserted")),
            )),
            PageStatus::Deleted => lines.push((
                false,
                RED,
                format!("{label}: {}", lang.pick("página eliminada", "page deleted")),
            )),
            PageStatus::Matched => {
                for t in &p.text_changes {
                    let (color, desc) = match (t.kind, t.format_only) {
                        (_, true) => (
                            AMBER,
                            format!(
                                "{} ({}): {}",
                                lang.pick("formato", "format"),
                                t.note.clone().unwrap_or_default(),
                                quoted(lang, &truncate(&t.text_b, 50))
                            ),
                        ),
                        (ChangeKind::Added, _) => (
                            GREEN,
                            format!(
                                "{}: {}",
                                lang.pick("añadido", "added"),
                                quoted(lang, &truncate(&t.text_b, 90))
                            ),
                        ),
                        (ChangeKind::Removed, _) => (
                            RED,
                            format!(
                                "{}: {}",
                                lang.pick("borrado", "deleted"),
                                quoted(lang, &truncate(&t.text_a, 90))
                            ),
                        ),
                        (ChangeKind::Changed, _) => (
                            AMBER,
                            format!(
                                "{}: {} → {}",
                                lang.pick("cambiado", "changed"),
                                quoted(lang, &truncate(&t.text_a, 42)),
                                quoted(lang, &truncate(&t.text_b, 42))
                            ),
                        ),
                    };
                    lines.push((false, color, format!("{label}: {desc}")));
                }
                if !p.visual_regions.is_empty() {
                    lines.push((
                        false,
                        GREY,
                        match lang {
                            Lang::Es => format!("{label}: {} zona(s) con diferencias visuales", p.visual_regions.len()),
                            Lang::En => format!("{label}: {} area(s) with visual differences", p.visual_regions.len()),
                        },
                    ));
                }
            }
        }
    }
    lines
}

fn summary_pages(doc: &mut Doc, c: &Comparison, lang: Lang) -> Result<()> {
    const W: f32 = 595.0;
    const H: f32 = 842.0;
    let lines = summary_lines(c, lang);
    let s = &c.summary;
    let mut idx = 0;
    let mut first = true;
    loop {
        let mut cv = Canvas::new();
        let mut y = H - 60.0;
        if first {
            cv.text(
                50.0,
                y,
                20.0,
                true,
                (0.1, 0.1, 0.1),
                lang.pick("Informe de comparación", "Comparison report"),
            );
            y -= 26.0;
            cv.text(
                50.0,
                y,
                10.0,
                false,
                GREY,
                &format!("{}: {}", lang.pick("Original", "Original"), file_name(&c.path_a)),
            );
            y -= 14.0;
            cv.text(
                50.0,
                y,
                10.0,
                false,
                GREY,
                &format!("{}{}", lang.pick("Nueva:    ", "New:      "), file_name(&c.path_b)),
            );
            y -= 28.0;
            let rows = [
                (
                    GREY,
                    match lang {
                        Lang::Es => format!("Páginas: {} (original) · {} (nueva)", s.pages_a, s.pages_b),
                        Lang::En => format!("Pages: {} (original) · {} (new)", s.pages_a, s.pages_b),
                    },
                ),
                (
                    GREEN,
                    format!(
                        "{}: {}",
                        lang.pick("Páginas insertadas", "Pages inserted"),
                        s.inserted_pages
                    ),
                ),
                (
                    RED,
                    format!(
                        "{}: {}",
                        lang.pick("Páginas eliminadas", "Pages deleted"),
                        s.deleted_pages
                    ),
                ),
                (
                    AMBER,
                    format!(
                        "{}: {}",
                        lang.pick("Páginas emparejadas con cambios", "Matched pages with changes"),
                        s.changed_pages
                    ),
                ),
                (
                    GREEN,
                    format!("{}: {}", lang.pick("Palabras añadidas", "Words added"), s.added_words),
                ),
                (
                    RED,
                    format!(
                        "{}: {}",
                        lang.pick("Palabras borradas", "Words deleted"),
                        s.removed_words
                    ),
                ),
                (
                    AMBER,
                    match lang {
                        Lang::Es => format!(
                            "Palabras cambiadas: {} · cambios de formato: {}",
                            s.changed_words, s.format_changes
                        ),
                        Lang::En => format!(
                            "Words changed: {} · format changes: {}",
                            s.changed_words, s.format_changes
                        ),
                    },
                ),
                (
                    GREY,
                    format!(
                        "{}: {}",
                        lang.pick("Zonas con diferencias visuales", "Areas with visual differences"),
                        s.visual_regions
                    ),
                ),
            ];
            for (col, text) in rows {
                cv.fill_rect((50.0, y - 1.0, 8.0, 8.0), col, false);
                cv.text(66.0, y, 11.0, false, (0.1, 0.1, 0.1), &text);
                y -= 16.0;
            }
            y -= 12.0;
            if c.is_identical() {
                cv.text(
                    50.0,
                    y,
                    12.0,
                    true,
                    GREEN,
                    lang.pick("No se encontraron diferencias.", "No differences were found."),
                );
                y -= 20.0;
            } else {
                cv.text(50.0, y, 12.0, true, (0.1, 0.1, 0.1), lang.pick("Cambios", "Changes"));
                y -= 18.0;
            }
            first = false;
        } else {
            cv.text(
                50.0,
                y,
                12.0,
                true,
                (0.1, 0.1, 0.1),
                lang.pick("Cambios (continuación)", "Changes (continued)"),
            );
            y -= 20.0;
        }
        while idx < lines.len() && y > 50.0 {
            let (_, col, text) = &lines[idx];
            cv.fill_rect((50.0, y - 1.0, 6.0, 6.0), *col, false);
            cv.text(62.0, y, 9.0, false, (0.1, 0.1, 0.1), text);
            y -= 13.0;
            idx += 1;
        }
        doc.add_page(W, H, cv)?;
        if idx >= lines.len() {
            return Ok(());
        }
    }
}

/// Rectángulo de la página (esquina superior izquierda, puntos) → caja PDF `(x, y, w, h)` dentro del panel.
struct Panel {
    ox: f32,
    top: f32,
    scale: f32,
}

impl Panel {
    fn rect(&self, r: &Rect) -> (f32, f32, f32, f32) {
        (
            self.ox + r.x0 * self.scale,
            self.top - r.y1 * self.scale,
            r.width() * self.scale,
            r.height() * self.scale,
        )
    }
}

fn detail_page(
    doc: &mut Doc,
    c: &Comparison,
    p: &PageComparison,
    da: &pliego_pdf::Document,
    db: &pliego_pdf::Document,
    lang: Lang,
) -> Result<()> {
    const W: f32 = 842.0;
    const H: f32 = 595.0;
    const MARGIN: f32 = 30.0;
    let panel_w = (W - 3.0 * MARGIN) / 2.0;
    let panel_h = H - 2.0 * MARGIN - 38.0;
    let mut cv = Canvas::new();
    let head = match p.status {
        PageStatus::Matched => match lang {
            Lang::Es => format!(
                "Original pág. {} · Nueva pág. {}",
                p.page_a.unwrap() + 1,
                p.page_b.unwrap() + 1
            ),
            Lang::En => format!(
                "Original page {} · New page {}",
                p.page_a.unwrap() + 1,
                p.page_b.unwrap() + 1
            ),
        },
        PageStatus::Inserted => match lang {
            Lang::Es => format!("Página insertada: nueva pág. {}", p.page_b.unwrap() + 1),
            Lang::En => format!("Page inserted: new page {}", p.page_b.unwrap() + 1),
        },
        PageStatus::Deleted => match lang {
            Lang::Es => format!("Página eliminada: original pág. {}", p.page_a.unwrap() + 1),
            Lang::En => format!("Page deleted: original page {}", p.page_a.unwrap() + 1),
        },
    };
    cv.text(MARGIN, H - MARGIN + 2.0, 12.0, true, (0.1, 0.1, 0.1), &head);
    cv.text(
        W - 300.0,
        H - MARGIN - 8.0,
        8.0,
        false,
        GREY,
        lang.pick(
            "Verde: añadido · Rojo: borrado · Ámbar: cambiado",
            "Green: added · Red: deleted · Amber: changed",
        ),
    );

    let mut panels: [Option<Panel>; 2] = [None, None];
    for (k, (idx, d, label)) in [
        (p.page_a, da, lang.pick("Original", "Original")),
        (p.page_b, db, lang.pick("Nueva", "New")),
    ]
    .into_iter()
    .enumerate()
    {
        let x = MARGIN + k as f32 * (panel_w + MARGIN);
        let top = H - MARGIN - 26.0;
        cv.text(x, top + 4.0, 9.0, true, GREY, label);
        let Some(i) = idx else {
            cv.stroke_rect((x, top - panel_h, panel_w, panel_h), GREY, 0.5);
            cv.text(
                x + 10.0,
                top - 20.0,
                10.0,
                false,
                GREY,
                lang.pick("(sin página)", "(no page)"),
            );
            continue;
        };
        let (pw, ph) = extract::page_size(d, i)?;
        let sc = (panel_w / pw).min(panel_h / ph);
        let raster = extract::render(d, i, REPORT_DPI / 72.0)?;
        let img = doc.add_image(&raster)?;
        let panel = Panel { ox: x, top, scale: sc };
        cv.image(img, x, top - ph * sc, pw * sc, ph * sc);
        cv.stroke_rect((x, top - ph * sc, pw * sc, ph * sc), (0.7, 0.7, 0.7), 0.5);
        panels[k] = Some(panel);
    }
    let paint = |cv: &mut Canvas, panel: &Option<Panel>, rects: &[Rect], col| {
        if let Some(panel) = panel {
            for r in rects {
                cv.fill_rect(panel.rect(r), col, true);
            }
        }
    };
    match p.status {
        PageStatus::Inserted => {
            if let Some(pn) = &panels[1] {
                let (pw, ph) = extract::page_size(db, p.page_b.unwrap())?;
                cv.stroke_rect(
                    (pn.ox, pn.top - ph * pn.scale, pw * pn.scale, ph * pn.scale),
                    GREEN,
                    3.0,
                );
            }
        }
        PageStatus::Deleted => {
            if let Some(pn) = &panels[0] {
                let (pw, ph) = extract::page_size(da, p.page_a.unwrap())?;
                cv.stroke_rect((pn.ox, pn.top - ph * pn.scale, pw * pn.scale, ph * pn.scale), RED, 3.0);
            }
        }
        PageStatus::Matched => {
            for t in &p.text_changes {
                let col = match t.kind {
                    ChangeKind::Added => GREEN,
                    ChangeKind::Removed => RED,
                    ChangeKind::Changed => AMBER,
                };
                paint(&mut cv, &panels[0], &t.rects_a, col);
                paint(&mut cv, &panels[1], &t.rects_b, col);
            }
            for v in &p.visual_regions {
                for pn in panels.iter().flatten() {
                    cv.stroke_rect(pn.rect(&v.rect), AMBER, 1.2);
                }
            }
        }
    }
    let _ = c;
    doc.add_page(W, H, cv)
}

/// Escribe el informe en `out` (a un temporal que luego se renombra).
pub fn write_report(comparison: &Comparison, out: &Path) -> Result<()> {
    write_report_with(comparison, out, &JobContext::silent())
}

/// Igual que [`write_report`], con progreso y cancelación.
pub fn write_report_with(comparison: &Comparison, out: &Path, ctx: &JobContext) -> Result<()> {
    write_report_in(comparison, out, ctx, Lang::Es)
}

/// Igual que [`write_report_with`], con el texto del informe en `lang` (el idioma de la interfaz).
pub fn write_report_in(comparison: &Comparison, out: &Path, ctx: &JobContext, lang: Lang) -> Result<()> {
    let da = extract::open(&comparison.path_a, comparison.password_a.as_deref())?;
    let db = extract::open(&comparison.path_b, comparison.password_b.as_deref())?;
    let mut doc = Doc::new();
    summary_pages(&mut doc, comparison, lang)?;
    let detail: Vec<&PageComparison> = comparison.pages.iter().filter(|p| p.has_differences()).collect();
    for (i, p) in detail.iter().enumerate() {
        ctx.step(i as u64, detail.len() as u64, "Dibujando el informe")?;
        detail_page(&mut doc, comparison, p, &da, &db, lang)?;
    }
    ctx.step(detail.len() as u64, detail.len() as u64, "Informe terminado")?;
    let bytes = doc.finish(lang.pick("Informe de comparación", "Comparison report"));
    let tmp = out.with_extension("pdf.tmp");
    std::fs::write(&tmp, &bytes).map_err(|e| Error::io_at(&tmp, e))?;
    std::fs::rename(&tmp, out).map_err(|e| Error::io_at(out, e))?;
    Ok(())
}
