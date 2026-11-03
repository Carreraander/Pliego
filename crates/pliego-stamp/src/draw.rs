use mupdf::pdf::{InsertImageOptions, PageImageSource, PdfDocument, PdfPage};
use mupdf::{Image, Rect};
use pliego_core::{Error, JobContext, Result};

use crate::options::{
    Background, BackgroundFill, BatesOptions, Corner, HeaderFooter, StampContext, TextStyle, Watermark,
    WatermarkContent,
};
use crate::pdf_err;
use crate::text::{Measurer, num, pdf_string};

/// Tamaño visual de la página y matriz `cm` que pasa de coordenadas visuales (origen abajo a la izquierda,
/// como se ve en pantalla) a las del espacio de usuario del PDF (tiene en cuenta giro y CropBox).
struct Frame {
    w: f32,
    h: f32,
    cm: String,
}

/// CropBox (o MediaBox) en el espacio de usuario, sin girar. `PdfPage::crop_box` no sirve aquí porque
/// mezcla las dimensiones visuales con las del espacio de usuario en páginas giradas.
fn user_box(page: &PdfPage) -> Result<Rect> {
    let obj = page.object();
    for key in ["CropBox", "MediaBox"] {
        let Some(arr) = obj.get_dict_inheritable(key).map_err(pdf_err)? else {
            continue;
        };
        let mut v = [0.0f32; 4];
        let mut ok = arr.len().map_err(pdf_err)? >= 4;
        for (i, slot) in v.iter_mut().enumerate() {
            match arr.get_array(i as i32).map_err(pdf_err)?.map(|n| n.as_float()) {
                Some(Ok(f)) => *slot = f,
                _ => ok = false,
            }
        }
        if ok {
            return Ok(Rect::new(
                v[0].min(v[2]),
                v[1].min(v[3]),
                v[0].max(v[2]),
                v[1].max(v[3]),
            ));
        }
    }
    Ok(Rect::new(0.0, 0.0, 595.0, 842.0))
}

fn frame(page: &PdfPage) -> Result<Frame> {
    let b = user_box(page)?;
    let (w, h) = (b.x1 - b.x0, b.y1 - b.y0);
    let rot = page.rotation().map_err(pdf_err)?.rem_euclid(360);
    let (vw, vh, m) = match rot {
        90 => (h, w, [0.0, 1.0, -1.0, 0.0, w, 0.0]),
        180 => (w, h, [-1.0, 0.0, 0.0, -1.0, w, h]),
        270 => (h, w, [0.0, -1.0, 1.0, 0.0, 0.0, h]),
        _ => (w, h, [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]),
    };
    let cm = format!(
        "{} {} {} {} {} {} cm\n",
        num(m[0]),
        num(m[1]),
        num(m[2]),
        num(m[3]),
        num(m[4] + b.x0),
        num(m[5] + b.y0)
    );
    Ok(Frame { w: vw, h: vh, cm })
}

fn check(doc: &PdfDocument, pages: &[u32]) -> Result<()> {
    let n = doc.page_count().map_err(pdf_err)?.max(0) as u32;
    match pages.iter().find(|&&p| p >= n) {
        Some(p) => Err(Error::InvalidInput(format!(
            "la página {} no existe (el documento tiene {n})",
            p + 1
        ))),
        None => Ok(()),
    }
}

/// Dibuja en cada página con `build`, que devuelve el flujo de contenido en coordenadas visuales.
fn stamp_pages(
    doc: &mut PdfDocument,
    pages: &[u32],
    overlay: bool,
    ctx: &JobContext,
    what: &str,
    mut build: impl FnMut(&mut PdfDocument, &mut PdfPage, &Frame, usize, u32) -> Result<Vec<u8>>,
) -> Result<()> {
    check(doc, pages)?;
    let total = pages.len() as u64;
    for (i, &p) in pages.iter().enumerate() {
        ctx.step(i as u64, total, what)?;
        let mut page = doc.load_pdf_page(p as i32).map_err(pdf_err)?;
        if overlay {
            // Equilibra el estado gráfico del contenido original para que el sello no lo herede.
            page.wrap_contents(doc).map_err(pdf_err)?;
        }
        let fr = frame(&page)?;
        let body = build(doc, &mut page, &fr, i, p)?;
        if body.is_empty() {
            continue;
        }
        let mut stream = b"q\n".to_vec();
        stream.extend(fr.cm.bytes());
        stream.extend(body);
        stream.extend(b"Q\n");
        page.insert_contents(doc, &stream, overlay).map_err(pdf_err)?;
    }
    ctx.step(total, total, "Listo")
}

fn color(c: [f32; 3]) -> String {
    format!("{} {} {} rg\n", num(c[0]), num(c[1]), num(c[2]))
}

/// Registra la fuente estándar con codificación WinAnsi (así los acentos y la eñe salen bien) en la página
/// y devuelve su nombre de recurso. `cache` guarda el objeto de fuente para reutilizarlo en todas las páginas.
fn font_resource(
    doc: &mut PdfDocument,
    page: &mut PdfPage,
    style: &TextStyle,
    cache: &mut Option<i32>,
) -> Result<String> {
    if !(style.size.is_finite() && style.size > 0.0) {
        return Err(Error::InvalidInput("el tamaño de letra debe ser mayor que 0".into()));
    }
    let key: String = style.font.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    let name = format!("PlgF{key}");
    let font = match *cache {
        Some(xref) => doc.new_indirect(xref, 0).map_err(pdf_err)?,
        None => {
            let src = format!(
                "<< /Type /Font /Subtype /Type1 /BaseFont /{} /Encoding /WinAnsiEncoding >>",
                style.font.replace(' ', "")
            );
            let obj = doc.new_object_from_str(&src).map_err(pdf_err)?;
            let obj = doc.add_object(&obj).map_err(pdf_err)?;
            *cache = Some(obj.as_indirect().map_err(pdf_err)?);
            obj
        }
    };
    let mut resources = page.resources().map_err(pdf_err)?;
    let mut fonts = match resources.get_dict("Font").map_err(pdf_err)? {
        Some(f) if f.is_dict().map_err(pdf_err)? => f.copy_dict().map_err(pdf_err)?,
        _ => doc.new_dict().map_err(pdf_err)?,
    };
    fonts.dict_put(name.as_str(), font).map_err(pdf_err)?;
    resources.dict_put("Font", fonts).map_err(pdf_err)?;
    Ok(format!("/{name}"))
}

/// `BT … ET` con el texto con su línea base en (x, y) visuales.
fn text_at(font: &str, style: &TextStyle, x: f32, y: f32, text: &str) -> Vec<u8> {
    let mut out = format!(
        "{}BT\n{font} {} Tf\n{} {} Td\n",
        color(style.color),
        num(style.size),
        num(x),
        num(y)
    )
    .into_bytes();
    out.extend(pdf_string(text));
    out.extend(b" Tj\nET\n");
    out
}

fn expand(tpl: &str, page: u32, pages: u32, ctx: &StampContext) -> String {
    tpl.replace("{page}", &(page + 1).to_string())
        .replace("{pages}", &pages.to_string())
        .replace("{date}", &ctx.date)
        .replace("{file}", &ctx.file)
}

/// Encabezado y pie con variables `{page}`, `{pages}`, `{date}` y `{file}`.
pub fn header_footer(
    doc: &mut PdfDocument,
    pages: &[u32],
    opts: &HeaderFooter,
    sctx: &StampContext,
    ctx: &JobContext,
) -> Result<()> {
    let total = doc.page_count().map_err(pdf_err)?.max(0) as u32;
    let meter = Measurer::new(&opts.style.font)?;
    let size = opts.style.size;
    let mut cache = None;
    stamp_pages(doc, pages, true, ctx, "Encabezados y pies", |doc, page, fr, _, p| {
        let font = font_resource(doc, page, &opts.style, &mut cache)?;
        let mut body = Vec::new();
        let rows = [(&opts.header, fr.h - opts.margin - size), (&opts.footer, opts.margin)];
        for (slots, y) in rows {
            for (which, tpl) in [(0, &slots.left), (1, &slots.center), (2, &slots.right)] {
                if tpl.is_empty() {
                    continue;
                }
                let text = expand(tpl, p, total, sctx);
                let w = meter.width(&text, size)?;
                let x = match which {
                    0 => opts.margin,
                    1 => (fr.w - w) / 2.0,
                    _ => fr.w - opts.margin - w,
                };
                body.extend(text_at(&font, &opts.style, x, y, &text));
            }
        }
        Ok(body)
    })
}

/// Numeración Bates consecutiva sobre las páginas indicadas, en el orden dado.
pub fn bates(doc: &mut PdfDocument, pages: &[u32], opts: &BatesOptions, ctx: &JobContext) -> Result<()> {
    let meter = Measurer::new(&opts.style.font)?;
    let size = opts.style.size;
    let mut cache = None;
    stamp_pages(doc, pages, true, ctx, "Numeración Bates", |doc, page, fr, i, _| {
        let font = font_resource(doc, page, &opts.style, &mut cache)?;
        let n = opts.start + i as u64;
        let text = format!(
            "{}{:0width$}{}",
            opts.prefix,
            n,
            opts.suffix,
            width = opts.digits as usize
        );
        let w = meter.width(&text, size)?;
        let (x, y) = match opts.corner {
            Corner::TopLeft => (opts.margin, fr.h - opts.margin - size),
            Corner::TopRight => (fr.w - opts.margin - w, fr.h - opts.margin - size),
            Corner::BottomLeft => (opts.margin, opts.margin),
            Corner::BottomRight => (fr.w - opts.margin - w, opts.margin),
        };
        Ok(text_at(&font, &opts.style, x, y, &text))
    })
}

fn opacity_gs(doc: &mut PdfDocument, page: &mut PdfPage, opacity: f32) -> Result<String> {
    if !(0.0..=1.0).contains(&opacity) {
        return Err(Error::InvalidInput("la opacidad debe estar entre 0 y 1".into()));
    }
    let gs = page
        .register_ext_gstate(doc, Some(opacity), Some(opacity))
        .map_err(pdf_err)?;
    Ok(gs.map(|g| format!("{g} gs\n")).unwrap_or_default())
}

fn read_image(path: &std::path::Path) -> Result<(Vec<u8>, f32)> {
    let data = std::fs::read(path).map_err(|e| Error::io_at(path, e))?;
    let img = Image::from_bytes(&data)
        .map_err(|_| Error::InvalidInput(format!("{} no es una imagen válida", path.display())))?;
    let ratio = img.height() as f32 / img.width().max(1) as f32;
    Ok((data, ratio))
}

/// Marca de agua de texto o imagen centrada en la página.
pub fn watermark(doc: &mut PdfDocument, pages: &[u32], wm: &Watermark, ctx: &JobContext) -> Result<()> {
    match &wm.content {
        WatermarkContent::Text { text, style } => {
            let meter = Measurer::new(&style.font)?;
            let w = meter.width(text, style.size)?;
            let (sin, cos) = wm.rotation.to_radians().sin_cos();
            let mut cache = None;
            stamp_pages(doc, pages, !wm.behind, ctx, "Marca de agua", |doc, page, fr, _, _| {
                let font = font_resource(doc, page, style, &mut cache)?;
                let gs = opacity_gs(doc, page, wm.opacity)?;
                let mut body = format!(
                    "{gs}{} {} {} {} {} {} cm\n",
                    num(cos),
                    num(sin),
                    num(-sin),
                    num(cos),
                    num(fr.w / 2.0),
                    num(fr.h / 2.0)
                )
                .into_bytes();
                body.extend(text_at(&font, style, -w / 2.0, -style.size * 0.35, text));
                Ok(body)
            })
        }
        WatermarkContent::Image { path, scale } => {
            if !(*scale > 0.0 && *scale <= 1.0) {
                return Err(Error::InvalidInput(
                    "la escala de la imagen debe estar entre 0 y 1".into(),
                ));
            }
            let (data, ratio) = read_image(path)?;
            check(doc, pages)?;
            let total = pages.len() as u64;
            for (i, &p) in pages.iter().enumerate() {
                ctx.step(i as u64, total, "Marca de agua")?;
                let mut page = doc.load_pdf_page(p as i32).map_err(pdf_err)?;
                let b = user_box(&page)?;
                let (w, h) = (b.x1 - b.x0, b.y1 - b.y0);
                let iw = w.min(h) * scale.max(0.01) * if w < h { 1.0 } else { h / w };
                let (rw, rh) = (iw, iw * ratio);
                let (cx, cy) = ((b.x0 + b.x1) / 2.0, (b.y0 + b.y1) / 2.0);
                let rect = Rect::new(cx - rw / 2.0, cy - rh / 2.0, cx + rw / 2.0, cy + rh / 2.0);
                let opts = InsertImageOptions {
                    overlay: !wm.behind,
                    opacity: Some(wm.opacity),
                    optional_content: None,
                };
                page.insert_image(
                    doc,
                    rect,
                    PageImageSource::Bytes {
                        data: &data,
                        format_hint: None,
                    },
                    opts,
                )
                .map_err(pdf_err)?;
            }
            ctx.step(total, total, "Listo")
        }
    }
}

/// Fondo de color o imagen detrás del contenido.
pub fn background(doc: &mut PdfDocument, pages: &[u32], bg: &Background, ctx: &JobContext) -> Result<()> {
    match &bg.fill {
        BackgroundFill::Color { rgb } => stamp_pages(doc, pages, false, ctx, "Fondo", |doc, page, fr, _, _| {
            let gs = opacity_gs(doc, page, bg.opacity)?;
            Ok(format!("{gs}{}0 0 {} {} re f\n", color(*rgb), num(fr.w), num(fr.h)).into_bytes())
        }),
        BackgroundFill::Image { path } => {
            let (data, _) = read_image(path)?;
            check(doc, pages)?;
            let total = pages.len() as u64;
            for (i, &p) in pages.iter().enumerate() {
                ctx.step(i as u64, total, "Fondo")?;
                let mut page = doc.load_pdf_page(p as i32).map_err(pdf_err)?;
                let rect = user_box(&page)?;
                let opts = InsertImageOptions {
                    overlay: false,
                    opacity: Some(bg.opacity),
                    optional_content: None,
                };
                page.insert_image(
                    doc,
                    rect,
                    PageImageSource::Bytes {
                        data: &data,
                        format_hint: None,
                    },
                    opts,
                )
                .map_err(pdf_err)?;
            }
            ctx.step(total, total, "Listo")
        }
    }
}
