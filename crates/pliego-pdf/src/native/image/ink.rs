//! Imágenes de un PDF reducidas a la cantidad de **una tinta** (C, M, Y o K), para la vista previa de separaciones
//! de `pliego-print`. PDFium solo dibuja en RGB, así que cada plancha se obtiene reescribiendo el documento para que
//! todo lo que pinta sea un gris que vale «1 − tinta»; aquí se hace con las imágenes.
//!
//! - [`Object::separate_image_ink`] sustituye un XObject de imagen por una imagen `DeviceGray` de 8 bits con la tinta
//!   pedida. Entiende `DeviceGray/RGB/CMYK`, `Cal*`, `ICCBased` (por su `N`), `Indexed`, `/Decode`, JPEG (también
//!   CMYK y YCCK de Adobe) y todos los filtros genéricos. Deja intactas las máscaras de recorte (`/ImageMask`) y lo
//!   que no sabe decodificar (JBIG2, CCITT, JPEG 2000, `Separation`, `DeviceN`, `Lab`): devuelve `false`.
//! - [`PdfDocument::separate_inline_image`] hace lo mismo con una imagen en línea (`BI … ID … EI`).
//!
//! RGB y gris se reparten con la conversión simple (K = 1 − máx(R, G, B)); CMYK pasa tal cual.

use lopdf::{Dictionary, Object as L, Stream};
use pliego_core::Result;

use super::decode::{Space, int, normalize_inline, num, sample_at, space_of};
use super::{check_size, invalid};
use crate::native::object::decode_stream;
use crate::native::{Object, PdfDocument, State};

/// Imagen reducida a CMYK de 8 bits (4 bytes por píxel: C, M, Y, K; 255 = tinta al 100 %).
pub(crate) struct InkImage {
    pub width: u32,
    pub height: u32,
    pub cmyk: Vec<u8>,
}

/// Cantidad de tinta (0..=255) de un color RGB con la conversión simple. Devuelve `[c, m, y, k]`.
pub(crate) fn rgb_to_cmyk(r: u8, g: u8, b: u8) -> [u8; 4] {
    let (r, g, b) = (f32::from(r) / 255.0, f32::from(g) / 255.0, f32::from(b) / 255.0);
    let k = 1.0 - r.max(g).max(b);
    if k >= 1.0 {
        return [0, 0, 0, 255];
    }
    let q = |v: f32| (((1.0 - v - k) / (1.0 - k)).clamp(0.0, 1.0) * 255.0).round() as u8;
    [q(r), q(g), q(b), (k * 255.0).round() as u8]
}

/// Aplica `/Decode` (pares mín-máx por componente) a una muestra de 8 bits.
fn apply_decode(b: u8, pair: (f32, f32)) -> u8 {
    let v = pair.0 + (f32::from(b) / 255.0) * (pair.1 - pair.0);
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Pares de `/Decode`, o `None` si no hay o es el predeterminado (que no cambia nada).
fn decode_pairs(st: &State, d: &Dictionary, comps: usize) -> Option<Vec<(f32, f32)>> {
    let L::Array(a) = st.deref(d.get(b"Decode").ok()?)? else {
        return None;
    };
    if a.len() < comps * 2 {
        return None;
    }
    let pairs: Vec<(f32, f32)> = (0..comps)
        .map(|i| {
            (
                num(st, a.get(i * 2)).unwrap_or(0.0),
                num(st, a.get(i * 2 + 1)).unwrap_or(1.0),
            )
        })
        .collect();
    (!pairs.iter().all(|p| *p == (0.0, 1.0))).then_some(pairs)
}

/// Pasa muestras de 8 bits (ya con `/Decode`) del espacio `comps` componentes a CMYK.
fn samples_to_cmyk(samples: &[u8], comps: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(samples.len() / comps.max(1) * 4);
    match comps {
        1 => {
            for &g in samples {
                out.extend_from_slice(&[0, 0, 0, 255 - g]);
            }
        }
        3 => {
            for p in samples.as_chunks::<3>().0 {
                out.extend_from_slice(&rgb_to_cmyk(p[0], p[1], p[2]));
            }
        }
        _ => out.extend_from_slice(samples),
    }
    out
}

/// Decodifica una imagen de un PDF a CMYK. `None` si no se puede (véase el comentario del módulo).
pub(crate) fn ink_image(st: &State, stream: &Stream, inline: bool) -> Result<Option<InkImage>> {
    let normalized;
    let s = if inline {
        normalized = normalize_inline(stream);
        &normalized
    } else {
        stream
    };
    let d = &s.dict;
    let w = u32::try_from(int(st, d, b"Width").unwrap_or(0)).unwrap_or(0);
    let h = u32::try_from(int(st, d, b"Height").unwrap_or(0)).unwrap_or(0);
    check_size(w, h)?;
    let is_mask = d
        .get(b"ImageMask")
        .ok()
        .and_then(|v| st.deref(v))
        .is_some_and(|v| matches!(v, L::Boolean(true)));
    if is_mask {
        return Ok(None);
    }

    const GENERIC: [&[u8]; 6] = [
        b"FlateDecode",
        b"LZWDecode",
        b"ASCII85Decode",
        b"ASCIIHexDecode",
        b"RunLengthDecode",
        b"BrotliDecode",
    ];
    let filters: Vec<Vec<u8>> = s
        .filters()
        .map(|f| f.into_iter().map(<[u8]>::to_vec).collect())
        .unwrap_or_default();
    let image_filter = filters.iter().find(|f| !GENERIC.contains(&f.as_slice()));
    let data = decode_stream(s)?;
    if let Some(f) = image_filter {
        if f.as_slice() != b"DCTDecode" {
            return Ok(None);
        }
        return Ok(jpeg_ink(st, d, &data, w, h));
    }

    let Some(space) = d.get(b"ColorSpace").ok().and_then(|v| space_of(st, v, 0)) else {
        return Ok(None);
    };
    let bpc = int(st, d, b"BitsPerComponent").unwrap_or(8) as usize;
    if !matches!(bpc, 1 | 2 | 4 | 8 | 16) {
        return Ok(None);
    }
    let (wu, hu, comps) = (w as usize, h as usize, space.comps());
    let row_bytes = (wu * comps * bpc).div_ceil(8);
    let maxv = (1u64 << bpc) - 1;
    let decode = match space {
        Space::Indexed { .. } => None,
        _ => decode_pairs(st, d, comps),
    };
    let out_comps = match &space {
        Space::Indexed { base, .. } => base.comps(),
        other => other.comps(),
    };
    let mut samples = Vec::with_capacity(wu * hu * out_comps);
    for y in 0..hu {
        let start = y * row_bytes;
        let row = data.get(start..).map_or(&[][..], |r| &r[..row_bytes.min(r.len())]);
        for x in 0..wu {
            if let Space::Indexed { base, hival, table } = &space {
                let idx = (sample_at(row, x, bpc) as usize).min(*hival);
                let n = base.comps();
                let entry = table.get(idx * n..idx * n + n);
                for c in 0..n {
                    samples.push(entry.map_or(0, |e| e[c]));
                }
            } else {
                for c in 0..comps {
                    let v = sample_at(row, x * comps + c, bpc);
                    let mut b = ((u64::from(v) * 255) / maxv) as u8;
                    if let Some(p) = &decode {
                        b = apply_decode(b, p[c]);
                    }
                    samples.push(b);
                }
            }
        }
    }
    Ok(Some(InkImage {
        width: w,
        height: h,
        cmyk: samples_to_cmyk(&samples, out_comps),
    }))
}

/// JPEG de 1, 3 o 4 componentes. Los de 4 (Adobe) guardan CMYK **invertido**; el `/Decode [1 0 …]` del PDF lo corrige.
fn jpeg_ink(st: &State, d: &Dictionary, data: &[u8], w: u32, h: u32) -> Option<InkImage> {
    use zune_jpeg::JpegDecoder;
    use zune_jpeg::zune_core::bytestream::ZCursor;
    use zune_jpeg::zune_core::colorspace::ColorSpace;
    use zune_jpeg::zune_core::options::DecoderOptions;

    let mut probe = JpegDecoder::new(ZCursor::new(data));
    probe.decode_headers().ok()?;
    let info = probe.info()?;
    if u32::from(info.width) != w || u32::from(info.height) != h {
        return None;
    }
    let comps = usize::from(info.components);
    let input = probe.input_colorspace()?;
    let out_cs = match comps {
        1 => ColorSpace::Luma,
        3 => ColorSpace::RGB,
        4 if input == ColorSpace::YCCK => ColorSpace::YCCK,
        4 => ColorSpace::CMYK,
        _ => return None,
    };
    let opts = DecoderOptions::default().jpeg_set_out_colorspace(out_cs);
    let mut dec = JpegDecoder::new_with_options(ZCursor::new(data), opts);
    let mut px = dec.decode().ok()?;
    if px.len() < w as usize * h as usize * comps {
        return None;
    }
    if out_cs == ColorSpace::YCCK {
        // YCC → RGB y, como libjpeg, C = 255 − R… (sigue «invertido»); K queda igual.
        for p in px.as_chunks_mut::<4>().0.iter_mut() {
            let (y, cb, cr) = (f32::from(p[0]), f32::from(p[1]) - 128.0, f32::from(p[2]) - 128.0);
            let clamp = |v: f32| v.round().clamp(0.0, 255.0) as u8;
            p[0] = 255 - clamp(y + 1.402 * cr);
            p[1] = 255 - clamp(y - 0.344_136 * cb - 0.714_136 * cr);
            p[2] = 255 - clamp(y + 1.772 * cb);
        }
    }
    if let Some(pairs) = decode_pairs(st, d, comps) {
        for p in px.chunks_exact_mut(comps) {
            for (v, pair) in p.iter_mut().zip(&pairs) {
                *v = apply_decode(*v, *pair);
            }
        }
    }
    Some(InkImage {
        width: w,
        height: h,
        cmyk: samples_to_cmyk(&px, comps),
    })
}

/// Pasa el gris `1 − tinta` de la plancha `channel` a un flujo de imagen `DeviceGray` de 8 bits comprimido.
fn gray_stream(base: &Dictionary, ink: &InkImage, channel: usize) -> Stream {
    let gray: Vec<u8> = ink.cmyk.as_chunks::<4>().0.iter().map(|p| 255 - p[channel]).collect();
    let mut dict = base.clone();
    for key in ["Filter", "DecodeParms", "Decode", "Mask", "Length", "DL"] {
        dict.remove(key.as_bytes());
    }
    dict.set("Width", L::Integer(i64::from(ink.width)));
    dict.set("Height", L::Integer(i64::from(ink.height)));
    dict.set("ColorSpace", L::Name(b"DeviceGray".to_vec()));
    dict.set("BitsPerComponent", L::Integer(8));
    crate::native::font::flate_stream(dict, &gray)
}

impl Object {
    /// Sustituye este XObject de imagen por una imagen `DeviceGray` de 8 bits con el gris `1 − tinta` de la plancha
    /// `channel` (0 = cian, 1 = magenta, 2 = amarillo, 3 = negro). Las máscaras (`/SMask`, `/Mask` de flujo) se
    /// conservan. `false` si la imagen no se puede decodificar o es una máscara de recorte: queda como estaba.
    pub fn separate_image_ink(&self, channel: usize) -> Result<bool> {
        if channel > 3 {
            return Err(invalid("la plancha debe estar entre 0 y 3"));
        }
        let st = self.state()?;
        let ink = {
            let guard = st.borrow();
            self.read(|v| match v {
                L::Stream(s) => ink_image(&guard, s, false),
                _ => Ok(None),
            })?
        };
        let Some(ink) = ink else { return Ok(false) };
        self.modify(|v| {
            if let L::Stream(s) = v {
                *s = gray_stream(&s.dict, &ink, channel);
            }
            Ok(())
        })?;
        Ok(true)
    }
}

impl PdfDocument {
    /// Reescribe una imagen en línea completa (`BI … ID <datos> EI`) como otra `DeviceGray` con la tinta de `channel`.
    /// `None` si no se puede (espacio de color por nombre de recurso, filtro no admitido…): se deja como está.
    pub fn separate_inline_image(&self, region: &[u8], channel: usize) -> Option<Vec<u8>> {
        if channel > 3 {
            return None;
        }
        let stream = parse_inline(region)?;
        let stream = &stream;
        let ink = {
            let guard = self.st.borrow();
            ink_image(&guard, stream, true).ok()??
        };
        let gray: Vec<u8> = ink.cmyk.as_chunks::<4>().0.iter().map(|p| 255 - p[channel]).collect();
        // Los datos van en hexadecimal + Flate: así el marcador `EI` no puede aparecer dentro de ellos.
        let packed = crate::native::font::deflate(&gray);
        let mut out = format!("BI /W {} /H {} /CS /G /BPC 8 /F [/AHx /Fl] ID\n", ink.width, ink.height).into_bytes();
        for b in packed {
            out.extend_from_slice(format!("{b:02x}").as_bytes());
        }
        out.extend_from_slice(b">\nEI");
        Some(out)
    }
}

/// Separa `BI <diccionario> ID <datos> EI` en un flujo con las claves tal como vienen (abreviadas). lopdf no admite
/// filtros en las imágenes en línea, así que se hace aquí.
fn parse_inline(region: &[u8]) -> Option<Stream> {
    let ws = |b: u8| matches!(b, 0 | 9 | 10 | 12 | 13 | 32);
    let body = region.strip_prefix(b"BI")?;
    let id = (0..body.len().saturating_sub(2))
        .find(|&i| body[i] == b'I' && body[i + 1] == b'D' && (i == 0 || ws(body[i - 1])) && ws(body[i + 2]))?;
    let dict_src = std::str::from_utf8(&body[..id]).ok()?;
    let data = body.get(id + 3..)?;
    let data = data.strip_suffix(b"EI").unwrap_or(data);
    let parsed = crate::native::Object::parse_in(&None, &format!("<< {dict_src} >>")).ok()?;
    let dict = parsed
        .read(|v| match v {
            L::Dictionary(d) => Ok(d.clone()),
            _ => Ok(Dictionary::new()),
        })
        .ok()?;
    Some(Stream::new(dict, data.to_vec()))
}
