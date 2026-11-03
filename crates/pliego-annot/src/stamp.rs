//! Sellos, enlaces y adjuntos: anotaciones cuya apariencia o estructura se construye a mano.

use mupdf::font::{Font, SimpleFontEncoding};
use mupdf::image::Image;
use mupdf::pdf::{PdfAnnotation, PdfAnnotationType, PdfDocument, PdfObject, PdfPage};
use mupdf::{Buffer, Rect};
use pliego_core::{Error, Result};

use crate::types::*;

fn pe(e: mupdf::Error) -> Error {
    Error::Pdf(e.to_string())
}

fn name(s: &str) -> Result<PdfObject> {
    PdfObject::new_name(s).map_err(pe)
}

fn real(v: f32) -> Result<PdfObject> {
    PdfObject::new_real(v).map_err(pe)
}

/// Cadena PDF literal en Latin-1 (los caracteres fuera de Latin-1 pasan a `?`).
fn latin1_literal(s: &str) -> Vec<u8> {
    let mut out = vec![b'('];
    for c in s.chars() {
        let b = u8::try_from(u32::from(c)).unwrap_or(b'?');
        if matches!(b, b'(' | b')' | b'\\') {
            out.push(b'\\');
        }
        out.push(b);
    }
    out.push(b')');
    out
}

fn bbox(w: f32, h: f32, doc: &PdfDocument) -> Result<PdfObject> {
    let mut a = doc.new_array().map_err(pe)?;
    for v in [0.0, 0.0, w, h] {
        a.array_push(real(v)?).map_err(pe)?;
    }
    Ok(a)
}

fn form_stream(doc: &mut PdfDocument, w: f32, h: f32, resources: PdfObject, content: &[u8]) -> Result<PdfObject> {
    let mut d = doc.new_dict().map_err(pe)?;
    d.dict_put("Type", name("XObject")?).map_err(pe)?;
    d.dict_put("Subtype", name("Form")?).map_err(pe)?;
    d.dict_put("BBox", bbox(w, h, doc)?).map_err(pe)?;
    d.dict_put("Resources", resources).map_err(pe)?;
    let buf = Buffer::from_bytes(content).map_err(pe)?;
    doc.add_stream(&buf, Some(&d), false).map_err(pe)
}

fn set_normal_appearance(doc: &PdfDocument, annot: &PdfAnnotation, stream: PdfObject) -> Result<()> {
    let mut ap = doc.new_dict().map_err(pe)?;
    ap.dict_put("N", stream).map_err(pe)?;
    annot.object().dict_put("AP", ap).map_err(pe)
}

fn text_stamp(doc: &mut PdfDocument, w: f32, h: f32, text: &str, c: Rgb) -> Result<PdfObject> {
    let font = Font::new("Helvetica-Bold").map_err(pe)?;
    let fobj = doc.add_simple_font(&font, SimpleFontEncoding::Latin).map_err(pe)?;
    // Ancho del texto en unidades de 1 pt de cuerpo, para centrar y ajustar el tamaño.
    let adv: f32 = text
        .chars()
        .map(|ch| {
            font.encode_character(ch as i32)
                .and_then(|g| font.advance_glyph(g))
                .unwrap_or(0.6)
        })
        .sum();
    let adv = if adv > 0.0 {
        adv
    } else {
        0.6 * text.chars().count() as f32
    };
    let size = (h * 0.5).min((w - 12.0) / adv).max(4.0);
    let tx = ((w - adv * size) / 2.0).max(2.0);
    let ty = (h - size * 0.72) / 2.0;
    let mut content = format!(
        "q {r} {g} {b} RG {r} {g} {b} rg 2 w 2 2 {rw} {rh} re S BT /F1 {size} Tf {tx} {ty} Td ",
        r = c[0],
        g = c[1],
        b = c[2],
        rw = (w - 4.0).max(1.0),
        rh = (h - 4.0).max(1.0),
    )
    .into_bytes();
    content.extend(latin1_literal(text));
    content.extend(b" Tj ET Q");
    let mut fonts = doc.new_dict().map_err(pe)?;
    fonts.dict_put("F1", fobj).map_err(pe)?;
    let mut res = doc.new_dict().map_err(pe)?;
    res.dict_put("Font", fonts).map_err(pe)?;
    form_stream(doc, w, h, res, &content)
}

fn image_stamp(doc: &mut PdfDocument, w: f32, h: f32, path: &str) -> Result<PdfObject> {
    let img =
        Image::from_file(path).map_err(|e| Error::InvalidInput(format!("no se pudo leer la imagen {path}: {e}")))?;
    let (iw, ih) = (img.width() as f32, img.height() as f32);
    if iw <= 0.0 || ih <= 0.0 {
        return Err(Error::InvalidInput("la imagen está vacía".into()));
    }
    let obj = doc.add_image(&img).map_err(pe)?;
    let scale = (w / iw).min(h / ih);
    let (dw, dh) = (iw * scale, ih * scale);
    let content = format!("q {dw} 0 0 {dh} {} {} cm /Im1 Do Q", (w - dw) / 2.0, (h - dh) / 2.0);
    let mut xo = doc.new_dict().map_err(pe)?;
    xo.dict_put("Im1", obj).map_err(pe)?;
    let mut res = doc.new_dict().map_err(pe)?;
    res.dict_put("XObject", xo).map_err(pe)?;
    form_stream(doc, w, h, res, content.as_bytes())
}

/// Crea sellos, enlaces y adjuntos. Devuelve la anotación y si ya tiene su apariencia propia
/// (en ese caso no hay que regenerarla).
pub(crate) fn create(doc: &mut PdfDocument, page: &mut PdfPage, geometry: &Geometry) -> Result<(PdfAnnotation, bool)> {
    match geometry {
        Geometry::Stamp { rect, source } => {
            let r = Rect::new(rect[0], rect[1], rect[2], rect[3]);
            let (w, h) = (r.x1 - r.x0, r.y1 - r.y0);
            if w < 4.0 || h < 4.0 {
                return Err(Error::InvalidInput("el recuadro del sello es demasiado pequeño".into()));
            }
            let stream = match source {
                StampSource::Preset { preset } => text_stamp(doc, w, h, preset.text(), preset.color())?,
                StampSource::Text { text, color } => text_stamp(doc, w, h, text, *color)?,
                StampSource::Image { path } => image_stamp(doc, w, h, path)?,
            };
            let a = page.add_stamp_annotation(r, "Custom").map_err(pe)?;
            set_normal_appearance(doc, &a, stream)?;
            Ok((a, true))
        }
        Geometry::Link { rect, target } => {
            let a = page.create_annotation(PdfAnnotationType::Link).map_err(pe)?;
            let mut obj = a.object();
            // MuPDF no escribe /Rect en los enlaces: se pasa del espacio de página al espacio PDF a mano.
            let inv = page
                .ctm()
                .map_err(pe)?
                .invert()
                .ok_or_else(|| Error::Pdf("página no invertible".into()))?;
            let pdf_rect = Rect::new(rect[0], rect[1], rect[2], rect[3]).transform(&inv);
            let mut ra = doc.new_array().map_err(pe)?;
            for v in [pdf_rect.x0, pdf_rect.y0, pdf_rect.x1, pdf_rect.y1] {
                ra.array_push(real(v)?).map_err(pe)?;
            }
            obj.dict_put("Rect", ra).map_err(pe)?;
            let mut action = doc.new_dict().map_err(pe)?;
            match target {
                LinkTarget::Uri { uri } => {
                    action.dict_put("S", name("URI")?).map_err(pe)?;
                    action
                        .dict_put("URI", PdfObject::new_string(uri).map_err(pe)?)
                        .map_err(pe)?;
                }
                LinkTarget::Page { page: p } => {
                    let n = doc.page_count().map_err(pe)? as usize;
                    if *p >= n {
                        return Err(Error::InvalidInput(format!("la página {} no existe", p + 1)));
                    }
                    let mut dest = doc.new_array().map_err(pe)?;
                    dest.array_push(doc.find_page(*p as i32).map_err(pe)?).map_err(pe)?;
                    dest.array_push(name("Fit")?).map_err(pe)?;
                    action.dict_put("S", name("GoTo")?).map_err(pe)?;
                    action.dict_put("D", dest).map_err(pe)?;
                }
            }
            obj.dict_put("A", action).map_err(pe)?;
            let mut border = doc.new_array().map_err(pe)?;
            for _ in 0..3 {
                border.array_push(real(0.0)?).map_err(pe)?;
            }
            obj.dict_put("Border", border).map_err(pe)?;
            Ok((a, true))
        }
        Geometry::Attachment { at, path } => {
            let data = std::fs::read(path).map_err(|e| Error::io_at(path, e))?;
            let file_name = std::path::Path::new(path)
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or("adjunto")
                .to_owned();
            let mut sd = doc.new_dict().map_err(pe)?;
            sd.dict_put("Type", name("EmbeddedFile")?).map_err(pe)?;
            let mut params = doc.new_dict().map_err(pe)?;
            params
                .dict_put(
                    "Size",
                    PdfObject::new_int(i32::try_from(data.len()).unwrap_or(i32::MAX)).map_err(pe)?,
                )
                .map_err(pe)?;
            sd.dict_put("Params", params).map_err(pe)?;
            let stream = doc
                .add_stream(&Buffer::from_bytes(&data).map_err(pe)?, Some(&sd), true)
                .map_err(pe)?;
            let mut ef = doc.new_dict().map_err(pe)?;
            ef.dict_put("F", stream).map_err(pe)?;
            let mut fs = doc.new_dict().map_err(pe)?;
            fs.dict_put("Type", name("Filespec")?).map_err(pe)?;
            fs.dict_put("F", PdfObject::new_string(&file_name).map_err(pe)?)
                .map_err(pe)?;
            fs.dict_put("UF", PdfObject::new_string(&file_name).map_err(pe)?)
                .map_err(pe)?;
            fs.dict_put("EF", ef).map_err(pe)?;
            let fs = doc.add_object(&fs).map_err(pe)?;
            let mut a = page.create_annotation(PdfAnnotationType::FileAttachment).map_err(pe)?;
            a.set_rect(Rect::new(at[0], at[1], at[0] + 18.0, at[1] + 18.0))
                .map_err(pe)?;
            a.set_icon_name("Paperclip").map_err(pe)?;
            a.object().dict_put("FS", fs).map_err(pe)?;
            Ok((a, false))
        }
        _ => Err(Error::Other("geometría no soportada en este módulo".into())),
    }
}

/// Destino de un enlace: dirección web o `#página` (desde 1).
fn link_target(action: &PdfObject, doc: &PdfDocument) -> Option<String> {
    if let Some(u) = action.get_dict("URI").ok()?
        && let Ok(s) = u.as_string()
    {
        return Some(s);
    }
    let d = action.get_dict("D").ok()??;
    let pg = d.get_array(0).ok()??;
    let n = doc.lookup_page_number(&pg).ok()?;
    Some(format!("#{}", n + 1))
}

/// Añade a `out` los enlaces de la página. MuPDF los trata aparte de las anotaciones, así que se leen
/// directamente del arreglo `/Annots`. No se pueden editar ni borrar con `update`/`delete`.
pub(crate) fn list_links(doc: &PdfDocument, page: &PdfPage, page_no: usize, out: &mut Vec<AnnotInfo>) -> Result<()> {
    let po = doc.find_page(page_no as i32).map_err(pe)?;
    let Some(annots) = po.get_dict("Annots").map_err(pe)? else {
        return Ok(());
    };
    if !annots.is_array().map_err(pe)? {
        return Ok(());
    }
    let ctm = page.ctm().map_err(pe)?;
    for i in 0..annots.len().map_err(pe)? {
        let Some(o) = annots.get_array(i as i32).map_err(pe)? else {
            continue;
        };
        let is_link = o
            .get_dict("Subtype")
            .map_err(pe)?
            .and_then(|s| s.as_name().ok())
            .is_some_and(|n| n == b"Link");
        if !is_link {
            continue;
        }
        let xref = o.as_indirect().unwrap_or(0);
        let mut v = [0f32; 4];
        if let Some(r) = o.get_dict("Rect").map_err(pe)? {
            for (k, slot) in v.iter_mut().enumerate() {
                if let Some(x) = r.get_array(k as i32).map_err(pe)? {
                    *slot = x.as_float().unwrap_or(0.0);
                }
            }
        }
        let r = Rect::new(v[0], v[1], v[2], v[3]).transform(&ctm);
        let link = o.get_dict("A").map_err(pe)?.and_then(|a| link_target(&a, doc));
        out.push(AnnotInfo {
            id: AnnotId { page: page_no, xref },
            kind: AnnotKind::Link,
            nm: None,
            rect: [r.x0, r.y0, r.x1, r.y1],
            style: Style::default(),
            author: None,
            subject: None,
            contents: None,
            created: None,
            modified: None,
            quads: Vec::new(),
            points: Vec::new(),
            strokes: Vec::new(),
            line: None,
            reply_to: None,
            state: None,
            link,
        });
    }
    Ok(())
}
