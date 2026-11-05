//! Borrado de píxeles en imágenes. Devuelve una imagen **nueva** (la original puede estar compartida) o `None` si
//! no se sabe hacer con seguridad (entonces el redactor quita la imagen entera).

use lopdf::{Dictionary, Object as L, Stream};

use super::super::State;
use super::super::object::decode_stream;
use super::geom::{M, R};

/// Con qué se rellenan los píxeles borrados.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Target {
    /// Blanco (sin tinta) del espacio de color de la imagen: la zona queda en blanco, como en MuPDF.
    Blank,
    /// Opaco del todo (máscaras suaves).
    Opaque,
    /// Máscara de recorte (`ImageMask`): «sin pintar».
    Unpainted,
    /// `/Mask` explícita: «pintar» (deja ver la imagen, ya en negro).
    Painted,
}

#[derive(Clone)]
struct Layout {
    w: usize,
    h: usize,
    bpc: usize,
    ncomp: usize,
    /// Valor de muestra (por componente) que debe escribirse en los píxeles borrados.
    fill: Vec<u32>,
}

fn number(st: &State, o: &L) -> Option<f64> {
    match st.deref(o)? {
        L::Integer(i) => Some(*i as f64),
        L::Real(r) => Some(f64::from(*r)),
        _ => None,
    }
}

fn get<'a>(st: &'a State, d: &'a Dictionary, k: &[u8]) -> Option<&'a L> {
    d.get(k).ok().and_then(|o| st.deref(o))
}

fn decode_array(st: &State, d: &Dictionary) -> Option<Vec<f64>> {
    match get(st, d, b"Decode") {
        Some(L::Array(a)) => Some(a.iter().filter_map(|o| number(st, o)).collect()),
        _ => None,
    }
}

/// Muestra que, con `decode`, da el valor decodificado `v` en el componente `i`.
fn sample_for(v: f64, i: usize, decode: &Option<Vec<f64>>, bpc: usize) -> u32 {
    let max = ((1u64 << bpc) - 1) as f64;
    let (lo, hi) = match decode {
        Some(d) if d.len() >= 2 * i + 2 => (d[2 * i], d[2 * i + 1]),
        _ => (0.0, 1.0),
    };
    if (hi - lo).abs() < f64::EPSILON {
        return 0;
    }
    (((v - lo) / (hi - lo)) * max).round().clamp(0.0, max) as u32
}

/// Componentes y valores decodificados del «negro» según el espacio de color. `None` si no se sabe.
fn color_info(st: &State, cs: &L, depth: u8) -> Option<(usize, Vec<f64>)> {
    if depth > 4 {
        return None;
    }
    let cs = st.deref(cs)?;
    match cs {
        L::Name(n) => match n.as_slice() {
            b"DeviceGray" | b"G" | b"CalGray" => Some((1, vec![1.0])),
            b"DeviceRGB" | b"RGB" | b"CalRGB" => Some((3, vec![1.0, 1.0, 1.0])),
            b"DeviceCMYK" | b"CMYK" => Some((4, vec![0.0, 0.0, 0.0, 0.0])),
            _ => None,
        },
        L::Array(a) => {
            let kind = match a.first().and_then(|o| st.deref(o))? {
                L::Name(n) => n.clone(),
                _ => return None,
            };
            match kind.as_slice() {
                b"CalGray" => Some((1, vec![1.0])),
                b"CalRGB" => Some((3, vec![1.0; 3])),
                b"ICCBased" => {
                    let s = match st.deref(a.get(1)?)? {
                        L::Stream(s) => s,
                        _ => return None,
                    };
                    match s.dict.get(b"N").ok().and_then(|o| number(st, o))? as usize {
                        1 => Some((1, vec![1.0])),
                        3 => Some((3, vec![1.0; 3])),
                        4 => Some((4, vec![0.0, 0.0, 0.0, 0.0])),
                        _ => None,
                    }
                }
                b"Indexed" | b"I" => Some((1, vec![f64::NAN])), // el índice se resuelve aparte
                b"Separation" => Some((1, vec![0.0])),
                b"DeviceN" => {
                    let names = match st.deref(a.get(1)?)? {
                        L::Array(n) => n.len(),
                        _ => return None,
                    };
                    Some((names, vec![0.0; names]))
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// Índice de la paleta con más luminosidad (el «blanco» de las imágenes indexadas).
fn lightest_index(st: &State, cs: &L) -> Option<u32> {
    let L::Array(a) = st.deref(cs)? else { return None };
    let base = a.get(1)?;
    let (n, _) = color_info(st, base, 1)?;
    let hival = number(st, a.get(2)?)? as usize;
    let lookup: Vec<u8> = match st.deref(a.get(3)?)? {
        L::String(s, _) => s.clone(),
        L::Stream(s) => decode_stream(s).ok()?,
        _ => return None,
    };
    let mut best: Option<(f64, u32)> = None;
    for i in 0..=hival {
        let e = lookup.get(i * n..i * n + n)?;
        let lum = match n {
            1 => f64::from(e[0]),
            3 => 0.3 * f64::from(e[0]) + 0.59 * f64::from(e[1]) + 0.11 * f64::from(e[2]),
            4 => 255.0 - f64::from(e[3]).max((f64::from(e[0]) + f64::from(e[1]) + f64::from(e[2])) / 3.0),
            _ => return None,
        };
        if best.is_none_or(|(b, _)| lum > b) {
            best = Some((lum, i as u32));
        }
    }
    best.map(|(_, i)| i)
}

fn layout(st: &State, d: &Dictionary, target: Target) -> Option<Layout> {
    let w = number(st, d.get(b"Width").ok()?)? as usize;
    let h = number(st, d.get(b"Height").ok()?)? as usize;
    if w == 0 || h == 0 || w.checked_mul(h)? > 400_000_000 {
        return None;
    }
    let is_mask = matches!(get(st, d, b"ImageMask"), Some(L::Boolean(true)));
    let decode = decode_array(st, d);
    if is_mask {
        // Sin pintar = la muestra que NO pinta; por defecto Decode [0 1]: 0 pinta, 1 no pinta.
        let inverted = decode.as_ref().is_some_and(|v| v.first() == Some(&1.0));
        let paint_sample = u32::from(inverted);
        let fill = match target {
            Target::Painted => paint_sample,
            _ => 1 - paint_sample,
        };
        return Some(Layout {
            w,
            h,
            bpc: 1,
            ncomp: 1,
            fill: vec![fill],
        });
    }
    let bpc = d
        .get(b"BitsPerComponent")
        .ok()
        .and_then(|o| number(st, o))
        .map_or(8, |v| v as usize);
    if ![1, 2, 4, 8, 16].contains(&bpc) {
        return None;
    }
    let cs = d.get(b"ColorSpace").ok()?;
    let (ncomp, black) = color_info(st, cs, 0)?;
    let mut fill = Vec::with_capacity(ncomp);
    if black.first().is_some_and(|v| v.is_nan()) {
        fill.push(lightest_index(st, cs)?);
    } else {
        for (i, v) in black.iter().enumerate() {
            // Opaco en una máscara suave (gris, 1.0 = opaco).
            let target_v = if target == Target::Opaque { 1.0 } else { *v };
            fill.push(sample_for(target_v, i, &decode, bpc));
        }
    }
    Some(Layout { w, h, bpc, ncomp, fill })
}

fn put_sample(row: &mut [u8], idx: usize, bpc: usize, v: u32) {
    match bpc {
        8 => row[idx] = v as u8,
        16 => {
            row[idx * 2] = (v >> 8) as u8;
            row[idx * 2 + 1] = v as u8;
        }
        _ => {
            let bit = idx * bpc;
            let byte = bit / 8;
            let shift = 8 - bpc - (bit % 8);
            let mask = (((1u16 << bpc) - 1) as u8) << shift;
            row[byte] = (row[byte] & !mask) | (((v as u8) << shift) & mask);
        }
    }
}

/// Rango de píxeles (columnas, filas) que pueden tocar `area` según la matriz de la imagen.
fn pixel_box(inv: &M, area: &R, l: &Layout) -> Option<(usize, usize, usize, usize)> {
    let corners = [
        (area.x0, area.y0),
        (area.x1, area.y0),
        (area.x0, area.y1),
        (area.x1, area.y1),
    ];
    let (mut umin, mut umax, mut vmin, mut vmax) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for (x, y) in corners {
        let (u, v) = inv.apply(x, y);
        umin = umin.min(u);
        umax = umax.max(u);
        vmin = vmin.min(v);
        vmax = vmax.max(v);
    }
    if umax < 0.0 || umin > 1.0 || vmax < 0.0 || vmin > 1.0 {
        return None;
    }
    let c0 = ((umin.max(0.0) * l.w as f64).floor() as isize - 1).max(0) as usize;
    let c1 = (((umax.min(1.0) * l.w as f64).ceil() as usize) + 1).min(l.w);
    // La fila 0 es la de arriba (v = 1).
    let r0 = (((1.0 - vmax.min(1.0)) * l.h as f64).floor() as isize - 1).max(0) as usize;
    let r1 = ((((1.0 - vmin.max(0.0)) * l.h as f64).ceil() as usize) + 1).min(l.h);
    (c0 < c1 && r0 < r1).then_some((c0, c1, r0, r1))
}

/// Borra en `data` (muestras sin filtro) los píxeles cuya celda toca alguna zona. Devuelve cuántos.
fn erase_samples(data: &mut [u8], l: &Layout, ctm: &M, areas: &[R]) -> usize {
    let Some(inv) = ctm.invert() else { return 0 };
    let stride = (l.w * l.ncomp * l.bpc).div_ceil(8);
    // Medio píxel en unidades de página (por exceso).
    let px = ((ctm.a.hypot(ctm.b)) / l.w as f64).max(ctm.c.hypot(ctm.d) / l.h as f64) * 0.5;
    let mut n = 0;
    for area in areas {
        let grown = R {
            x0: area.x0 - px,
            y0: area.y0 - px,
            x1: area.x1 + px,
            y1: area.y1 + px,
        };
        let Some((c0, c1, r0, r1)) = pixel_box(&inv, area, l) else {
            continue;
        };
        for r in r0..r1 {
            let v = 1.0 - (r as f64 + 0.5) / l.h as f64;
            for c in c0..c1 {
                let u = (c as f64 + 0.5) / l.w as f64;
                let (x, y) = ctm.apply(u, v);
                if grown.contains(x, y) {
                    let row = &mut data[r * stride..(r + 1) * stride];
                    for k in 0..l.ncomp {
                        put_sample(row, c * l.ncomp + k, l.bpc, l.fill[k]);
                    }
                    n += 1;
                }
            }
        }
    }
    n
}

fn new_stream(base: &Stream, data: Vec<u8>, jpeg: bool) -> Stream {
    let mut dict = base.dict.clone();
    dict.remove(b"Filter");
    dict.remove(b"DecodeParms");
    dict.remove(b"DP");
    dict.remove(b"F");
    // Un JPEG nuevo no puede arrastrar metadatos ni versiones alternativas de la imagen original.
    dict.remove(b"Alternates");
    dict.remove(b"Metadata");
    dict.remove(b"OPI");
    let mut s = Stream::new(dict, data);
    if jpeg {
        s.dict.set("Filter", L::Name(b"DCTDecode".to_vec()));
        s.dict.set("Length", L::Integer(s.content.len() as i64));
    } else {
        s.allows_compression = true;
        let _ = s.compress();
    }
    s
}

/// Resultado de borrar píxeles.
pub(super) enum Erased {
    /// Ningún píxel cae bajo las zonas: la imagen sirve tal cual.
    Same,
    /// Imagen nueva.
    New(Stream),
}

/// Borra los píxeles de `img` bajo las zonas. `None` = no se sabe hacerlo (formato no soportado o datos dañados).
pub(super) fn erase(st: &State, img: &Stream, ctm: &M, areas: &[R], target: Target) -> Option<Erased> {
    let l = layout(st, &img.dict, target)?;
    let filters: Vec<Vec<u8>> = img
        .filters()
        .map(|f| f.into_iter().map(<[u8]>::to_vec).collect())
        .unwrap_or_default();
    let image_filter = filters.iter().find(|f| {
        matches!(
            f.as_slice(),
            b"DCTDecode" | b"DCT" | b"JPXDecode" | b"CCITTFaxDecode" | b"CCF" | b"JBIG2Decode"
        )
    });
    let raw = decode_stream(img).ok()?;
    match image_filter.map(Vec::as_slice) {
        None => {
            let stride = (l.w * l.ncomp * l.bpc).div_ceil(8);
            let mut data = raw;
            if data.len() < stride * l.h {
                return None;
            }
            data.truncate(stride * l.h);
            if erase_samples(&mut data, &l, ctm, areas) == 0 {
                return Some(Erased::Same);
            }
            Some(Erased::New(new_stream(img, data, false)))
        }
        Some(b"DCTDecode" | b"DCT") => erase_jpeg(img, &raw, &l, ctm, areas),
        _ => None,
    }
}

fn erase_jpeg(img: &Stream, jpeg: &[u8], l: &Layout, ctm: &M, areas: &[R]) -> Option<Erased> {
    use zune_jpeg::JpegDecoder;
    use zune_jpeg::zune_core::bytestream::ZCursor;
    use zune_jpeg::zune_core::colorspace::ColorSpace;
    use zune_jpeg::zune_core::options::DecoderOptions;

    let mut probe = JpegDecoder::new(ZCursor::new(jpeg));
    probe.decode_headers().ok()?;
    let info = probe.info()?;
    let comps = usize::from(info.components);
    let (cs, ct) = match comps {
        1 => (ColorSpace::Luma, jpeg_encoder::ColorType::Luma),
        3 => (ColorSpace::RGB, jpeg_encoder::ColorType::Rgb),
        _ => return None,
    };
    if l.bpc != 8 || l.ncomp != comps || usize::from(info.width) != l.w || usize::from(info.height) != l.h {
        return None;
    }
    let opts = DecoderOptions::default().jpeg_set_out_colorspace(cs);
    let mut dec = JpegDecoder::new_with_options(ZCursor::new(jpeg), opts);
    let mut pixels = dec.decode().ok()?;
    if erase_samples(&mut pixels, l, ctm, areas) == 0 {
        return Some(Erased::Same);
    }
    let mut out = Vec::new();
    jpeg_encoder::Encoder::new(&mut out, 92)
        .encode(&pixels, info.width, info.height, ct)
        .ok()?;
    Some(Erased::New(new_stream(img, out, true)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn put_sample_packs_bits() {
        let mut row = [0xFFu8; 2];
        put_sample(&mut row, 1, 1, 0);
        assert_eq!(row[0], 0b1011_1111);
        put_sample(&mut row, 2, 4, 0);
        assert_eq!(row[1], 0x0F);
        let mut r16 = [0xFFu8; 4];
        put_sample(&mut r16, 1, 16, 0x0102);
        assert_eq!(r16, [0xFF, 0xFF, 1, 2]);
    }

    #[test]
    fn decode_inverts_blank() {
        let d = Some(vec![1.0, 0.0]);
        assert_eq!(sample_for(0.0, 0, &d, 8), 255);
        assert_eq!(sample_for(0.0, 0, &None, 8), 0);
        assert_eq!(sample_for(1.0, 0, &None, 8), 255);
    }
}
