//! Submuestreo y recompresión JPEG de imágenes.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use jpeg_encoder::{ColorType, Encoder};
use mupdf::pdf::{PdfDocument, PdfObject};
use mupdf::{Buffer, Device, Image, Matrix};
use pliego_core::{Error, JobContext, Result};

use crate::{ImageStats, pdf_err};

pub(crate) struct ImageParams {
    pub target_dpi: f32,
    pub quality: u8,
    pub min_bytes: u64,
}

type Placements = Rc<RefCell<Vec<(Image, Matrix)>>>;

/// Dispositivo que solo anota qué imágenes se dibujan y con qué matriz.
struct Collector(Placements);

impl mupdf::NativeDevice for Collector {
    fn fill_image(&mut self, img: &Image, cmt: Matrix, _alpha: f32, _cp: mupdf::ColorParams) {
        self.0.borrow_mut().push((img.clone(), cmt));
    }
}

struct Seen {
    image: Image,
    /// Mayor resolución efectiva (ppp) con la que se dibuja la imagen en alguna página.
    dpi: f32,
}

fn effective_dpi(w: u32, h: u32, m: &Matrix) -> f32 {
    let wpt = m.a.hypot(m.b).max(0.01);
    let hpt = m.c.hypot(m.d).max(0.01);
    (w as f32 / (wpt / 72.0)).max(h as f32 / (hpt / 72.0))
}

pub(crate) fn optimize_images(doc: &mut PdfDocument, p: &ImageParams, ctx: &JobContext) -> Result<ImageStats> {
    let pages = doc.page_count().map_err(pdf_err)? as u32;
    let mut seen: HashMap<i32, Seen> = HashMap::new();
    for i in 0..pages {
        ctx.step(i as u64, pages as u64 * 2, "Buscando imágenes")?;
        let page = doc.load_pdf_page(i as i32).map_err(pdf_err)?;
        let infos = page.images().map_err(pdf_err)?;
        if infos.is_empty() {
            continue;
        }
        let placements: Placements = Rc::default();
        let dev = Device::from_native(Collector(placements.clone())).map_err(pdf_err)?;
        page.run_contents(&dev, &Matrix::IDENTITY).map_err(pdf_err)?;
        drop(dev);
        for (img, m) in placements.borrow().iter() {
            let (w, h) = (img.width(), img.height());
            let dpi = effective_dpi(w, h, m);
            // Se asocia por dimensiones con los XObject de la página.
            for info in infos.iter().filter(|i| i.width == w && i.height == h) {
                let e = seen.entry(info.xref).or_insert_with(|| Seen {
                    image: img.clone(),
                    dpi,
                });
                e.dpi = e.dpi.max(dpi);
            }
        }
    }

    let total = count_images(doc)?;
    let mut stats = ImageStats {
        total,
        ..Default::default()
    };
    let mut xrefs: Vec<i32> = seen.keys().copied().collect();
    xrefs.sort_unstable();
    for (n, xref) in xrefs.iter().enumerate() {
        ctx.step(
            pages as u64 + n as u64,
            pages as u64 + xrefs.len() as u64,
            "Recomprimiendo imágenes",
        )?;
        let s = &seen[xref];
        match recompress(doc, *xref, &s.image, s.dpi, p)? {
            Outcome::Skipped => {}
            Outcome::Recompressed { downsampled } => {
                stats.recompressed += 1;
                if downsampled {
                    stats.downsampled += 1;
                }
            }
        }
    }
    stats.skipped = total.saturating_sub(stats.recompressed);
    Ok(stats)
}

fn count_images(doc: &PdfDocument) -> Result<u32> {
    let n = doc.xref_len().map_err(pdf_err)? as i32;
    let mut c = 0;
    for x in 1..n {
        if let Ok(o) = doc.new_indirect(x, 0)
            && o.is_stream().unwrap_or(false)
            && name_of(&o, "Subtype").as_deref() == Some("Image")
        {
            c += 1;
        }
    }
    Ok(c)
}

fn name_of(o: &PdfObject, key: &str) -> Option<String> {
    o.get_dict(key)
        .ok()
        .flatten()
        .and_then(|v| v.as_name().ok())
        .map(|b| String::from_utf8_lossy(&b).into_owned())
}

enum Outcome {
    Skipped,
    Recompressed { downsampled: bool },
}

/// Nombre del filtro final de la imagen (el que la codifica).
fn last_filter(o: &PdfObject) -> Option<String> {
    let f = o.get_dict("Filter").ok().flatten()?;
    let f = if f.is_array().ok()? {
        f.get_array(f.len().ok()?.checked_sub(1)? as i32).ok().flatten()?
    } else {
        f
    };
    f.as_name().ok().map(|b| String::from_utf8_lossy(&b).into_owned())
}

/// Espacios de color que sabemos reescribir sin cambiar su significado.
fn colorspace_components(o: &PdfObject) -> Option<u8> {
    let cs = o.get_dict("ColorSpace").ok().flatten()?;
    if cs.is_name().ok()? {
        return match String::from_utf8_lossy(&cs.as_name().ok()?).as_ref() {
            "DeviceGray" | "CalGray" => Some(1),
            "DeviceRGB" | "CalRGB" => Some(3),
            _ => None,
        };
    }
    if cs.is_array().ok()? {
        let first = cs.get_array(0).ok().flatten()?;
        if String::from_utf8_lossy(&first.as_name().ok()?) == "ICCBased" {
            let profile = cs.get_array(1).ok().flatten()?.resolve().ok().flatten()?;
            return profile.get_dict("N").ok().flatten()?.as_int().ok().map(|n| n as u8);
        }
    }
    None
}

fn recompress(doc: &mut PdfDocument, xref: i32, image: &Image, dpi: f32, p: &ImageParams) -> Result<Outcome> {
    let mut obj = doc.new_indirect(xref, 0).map_err(pdf_err)?;
    let is_true = |k: &str| {
        obj.get_dict(k)
            .ok()
            .flatten()
            .and_then(|v| v.as_bool().ok())
            .unwrap_or(false)
    };
    if is_true("ImageMask") || obj.get_dict("Mask").ok().flatten().is_some() {
        return Ok(Outcome::Skipped);
    }
    let filter = last_filter(&obj);
    if matches!(filter.as_deref(), Some("JBIG2Decode" | "CCITTFaxDecode" | "JPXDecode")) {
        return Ok(Outcome::Skipped);
    }
    let Some(comps) = colorspace_components(&obj) else {
        return Ok(Outcome::Skipped);
    };
    if obj
        .get_dict("BitsPerComponent")
        .ok()
        .flatten()
        .and_then(|b| b.as_int().ok())
        == Some(1)
    {
        return Ok(Outcome::Skipped);
    }
    let old_len = obj.read_raw_stream().map(|s| s.len() as u64).unwrap_or(0);
    if old_len < p.min_bytes {
        return Ok(Outcome::Skipped);
    }

    let pix = image.to_pixmap().map_err(pdf_err)?;
    if pix.alpha() || pix.n() != comps || !(pix.n() == 1 || pix.n() == 3) {
        return Ok(Outcome::Skipped);
    }
    let (w, h) = (pix.width() as usize, pix.height() as usize);
    let n = pix.n() as usize;
    let stride = pix.stride() as usize;
    let mut data = Vec::with_capacity(w * h * n);
    for y in 0..h {
        data.extend_from_slice(&pix.samples()[y * stride..y * stride + w * n]);
    }

    let factor = if dpi > p.target_dpi * 1.1 {
        p.target_dpi / dpi
    } else {
        1.0
    };
    let (nw, nh) = (
        ((w as f32 * factor).round() as usize).max(1),
        ((h as f32 * factor).round() as usize).max(1),
    );
    let downsampled = nw < w || nh < h;
    if downsampled {
        data = resample(&data, w, h, n, nw, nh);
    }
    let mut jpeg = Vec::new();
    let color = if n == 1 { ColorType::Luma } else { ColorType::Rgb };
    Encoder::new(&mut jpeg, p.quality)
        .encode(&data, nw as u16, nh as u16, color)
        .map_err(|e| Error::Other(format!("no se pudo recomprimir una imagen: {e}")))?;

    // Si ya era JPEG y no se reduce, recomprimir solo compensa si el ahorro es claro.
    let min_gain = if downsampled || filter.as_deref() != Some("DCTDecode") {
        0.9
    } else {
        0.8
    };
    if jpeg.len() as f64 > old_len as f64 * min_gain {
        return Ok(Outcome::Skipped);
    }

    let write = |obj: &mut PdfObject| -> std::result::Result<(), mupdf::Error> {
        obj.dict_put("Width", PdfObject::new_int(nw as i32)?)?;
        obj.dict_put("Height", PdfObject::new_int(nh as i32)?)?;
        obj.dict_put("BitsPerComponent", PdfObject::new_int(8)?)?;
        obj.dict_put("Filter", PdfObject::new_name("DCTDecode")?)?;
        obj.dict_delete("DecodeParms")?;
        obj.dict_delete("Decode")?;
        obj.write_raw_stream_buffer(&Buffer::from_bytes(&jpeg)?)
    };
    write(&mut obj).map_err(pdf_err)?;
    Ok(Outcome::Recompressed { downsampled })
}

/// Reducción por promedio de área (filtro de caja) en dos pasadas.
fn resample(src: &[u8], w: usize, h: usize, n: usize, nw: usize, nh: usize) -> Vec<u8> {
    let axis = |len: usize, nlen: usize| -> Vec<(usize, usize, Vec<f32>)> {
        let scale = len as f32 / nlen as f32;
        (0..nlen)
            .map(|i| {
                let (a, b) = (i as f32 * scale, ((i + 1) as f32 * scale).min(len as f32));
                let (s, e) = (a.floor() as usize, (b.ceil() as usize).min(len));
                let weights = (s..e).map(|k| ((k + 1) as f32).min(b) - (k as f32).max(a)).collect();
                (s, e, weights)
            })
            .collect()
    };
    let (xs, ys) = (axis(w, nw), axis(h, nh));
    // Horizontal: h filas × nw columnas.
    let mut tmp = vec![0f32; h * nw * n];
    for y in 0..h {
        for (x, (s, _, wts)) in xs.iter().enumerate() {
            let total: f32 = wts.iter().sum();
            for c in 0..n {
                let acc: f32 = wts
                    .iter()
                    .enumerate()
                    .map(|(k, wt)| src[(y * w + s + k) * n + c] as f32 * wt)
                    .sum();
                tmp[(y * nw + x) * n + c] = acc / total;
            }
        }
    }
    let mut out = vec![0u8; nw * nh * n];
    for (y, (s, _, wts)) in ys.iter().enumerate() {
        let total: f32 = wts.iter().sum();
        for x in 0..nw {
            for c in 0..n {
                let acc: f32 = wts
                    .iter()
                    .enumerate()
                    .map(|(k, wt)| tmp[((s + k) * nw + x) * n + c] * wt)
                    .sum();
                out[(y * nw + x) * n + c] = (acc / total).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resample_halves_and_averages() {
        // 4×2 gris: [0, 100, 200, 200 / 0, 100, 200, 200] → 2×1.
        let src = [0u8, 100, 200, 200, 0, 100, 200, 200];
        assert_eq!(resample(&src, 4, 2, 1, 2, 1), vec![50, 200]);
    }

    #[test]
    fn resample_keeps_constant_color() {
        let src = vec![77u8; 10 * 10 * 3];
        assert!(resample(&src, 10, 10, 3, 3, 4).iter().all(|&v| v == 77));
    }
}
