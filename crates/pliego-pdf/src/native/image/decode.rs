//! Imágenes que ya están dentro de un PDF (XObject o en línea) → [`Image`].
//!
//! Lo usa `image_placements`. Soporta DCT (se conserva el JPEG), y para los demás filtros genéricos de PDF las
//! muestras de 1, 2, 4, 8 y 16 bits en `DeviceGray/RGB/CMYK`, `Cal*`, `ICCBased` (por su `N`) e `Indexed`, con
//! `/Decode` invertido, `/ImageMask` y `/SMask`. JBIG2, CCITT, JPEG 2000, `Separation`, `DeviceN` y `Lab` dejan una
//! imagen con dimensiones pero sin píxeles ([`Pixels::Unsupported`]).

use lopdf::{Dictionary, Object as L, Stream};

use super::{Cs, Image, Inner, Pixels, check_size, invalid, parse_jpeg};
use crate::native::State;
use crate::native::object::decode_stream;
use pliego_core::Result;

/// Profundidad máxima al seguir `/SMask` (una máscara de una máscara no tiene sentido).
const MAX_DEPTH: u8 = 2;

#[derive(Clone)]
pub(super) enum Space {
    Gray,
    Rgb,
    Cmyk,
    Indexed {
        base: Box<Space>,
        hival: usize,
        table: Vec<u8>,
    },
}

impl Space {
    pub(super) fn comps(&self) -> usize {
        match self {
            Space::Gray => 1,
            Space::Rgb => 3,
            Space::Cmyk => 4,
            Space::Indexed { .. } => 1,
        }
    }
    fn cs(&self) -> Cs {
        match self {
            Space::Gray => Cs::Gray,
            Space::Rgb => Cs::Rgb,
            Space::Cmyk => Cs::Cmyk,
            Space::Indexed { base, .. } => base.cs(),
        }
    }
}

/// Claves de un diccionario de imagen en línea → nombre completo.
const INLINE_KEYS: [(&[u8], &[u8]); 9] = [
    (b"BPC", b"BitsPerComponent"),
    (b"CS", b"ColorSpace"),
    (b"D", b"Decode"),
    (b"DP", b"DecodeParms"),
    (b"F", b"Filter"),
    (b"H", b"Height"),
    (b"IM", b"ImageMask"),
    (b"I", b"Interpolate"),
    (b"W", b"Width"),
];

fn full_name(n: &[u8]) -> Vec<u8> {
    match n {
        b"G" => b"DeviceGray".to_vec(),
        b"RGB" => b"DeviceRGB".to_vec(),
        b"CMYK" => b"DeviceCMYK".to_vec(),
        b"I" => b"Indexed".to_vec(),
        b"AHx" => b"ASCIIHexDecode".to_vec(),
        b"A85" => b"ASCII85Decode".to_vec(),
        b"LZW" => b"LZWDecode".to_vec(),
        b"Fl" => b"FlateDecode".to_vec(),
        b"RL" => b"RunLengthDecode".to_vec(),
        b"CCF" => b"CCITTFaxDecode".to_vec(),
        b"DCT" => b"DCTDecode".to_vec(),
        other => other.to_vec(),
    }
}

fn expand_value(v: &L) -> L {
    match v {
        L::Name(n) => L::Name(full_name(n)),
        L::Array(a) => L::Array(a.iter().map(expand_value).collect()),
        other => other.clone(),
    }
}

/// Copia del flujo con las abreviaturas de las imágenes en línea (`/W`, `/CS /G`, `/F /Fl`…) escritas enteras.
pub(super) fn normalize_inline(s: &Stream) -> Stream {
    let mut d = Dictionary::new();
    for (k, v) in s.dict.iter() {
        let key = INLINE_KEYS
            .iter()
            .find(|(abbr, _)| *abbr == k.as_slice())
            .map_or_else(|| k.clone(), |(_, full)| full.to_vec());
        d.set(key, expand_value(v));
    }
    Stream::new(d, s.content.clone())
}

pub(super) fn num(st: &State, v: Option<&L>) -> Option<f32> {
    match st.deref(v?)? {
        L::Integer(i) => Some(*i as f32),
        L::Real(r) => Some(*r),
        _ => None,
    }
}

pub(super) fn int(st: &State, d: &Dictionary, key: &[u8]) -> Option<i64> {
    num(st, d.get(key).ok()).map(|v| v as i64)
}

/// Bytes de una cadena o de un flujo (la tabla de un espacio `Indexed`).
fn bytes_of(st: &State, v: &L) -> Option<Vec<u8>> {
    match st.deref(v)? {
        L::String(s, _) => Some(s.clone()),
        L::Stream(s) => decode_stream(s).ok(),
        _ => None,
    }
}

pub(super) fn space_of(st: &State, v: &L, depth: u8) -> Option<Space> {
    if depth > 4 {
        return None;
    }
    match st.deref(v)? {
        L::Name(n) => match full_name(n).as_slice() {
            b"DeviceGray" | b"CalGray" => Some(Space::Gray),
            b"DeviceRGB" | b"CalRGB" => Some(Space::Rgb),
            b"DeviceCMYK" => Some(Space::Cmyk),
            _ => None,
        },
        L::Array(a) => {
            let kind = match st.deref(a.first()?)? {
                L::Name(n) => full_name(n),
                _ => return None,
            };
            match kind.as_slice() {
                b"CalGray" => Some(Space::Gray),
                b"CalRGB" => Some(Space::Rgb),
                b"DeviceGray" | b"DeviceRGB" | b"DeviceCMYK" => space_of(st, a.first()?, depth + 1),
                b"ICCBased" => match st.dict_of(a.get(1)?)?.get(b"N").ok().and_then(|n| num(st, Some(n)))? as i64 {
                    1 => Some(Space::Gray),
                    3 => Some(Space::Rgb),
                    4 => Some(Space::Cmyk),
                    _ => None,
                },
                b"Indexed" => {
                    let base = space_of(st, a.get(1)?, depth + 1)?;
                    if matches!(base, Space::Indexed { .. }) {
                        return None;
                    }
                    let hival = num(st, a.get(2))? as usize;
                    let table = bytes_of(st, a.get(3)?)?;
                    Some(Space::Indexed {
                        base: Box::new(base),
                        hival: hival.min(255),
                        table,
                    })
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// Valor de la muestra `i` (de `bpc` bits) en una fila.
pub(super) fn sample_at(row: &[u8], i: usize, bpc: usize) -> u32 {
    match bpc {
        8 => u32::from(*row.get(i).unwrap_or(&0)),
        16 => u32::from(*row.get(i * 2).unwrap_or(&0)) << 8 | u32::from(*row.get(i * 2 + 1).unwrap_or(&0)),
        _ => {
            let bit = i * bpc;
            let byte = *row.get(bit / 8).unwrap_or(&0);
            let shift = 8 - bpc - (bit % 8);
            u32::from((byte >> shift) & ((1u8 << bpc) - 1))
        }
    }
}

impl Image {
    /// Decodifica un XObject de imagen (`inline = false`) o una imagen en línea (`BI … EI`, con claves abreviadas).
    pub(crate) fn from_stream(st: &State, stream: &Stream, inline: bool) -> Result<Image> {
        Image::from_stream_at(st, stream, inline, 0)
    }

    fn from_stream_at(st: &State, stream: &Stream, inline: bool, depth: u8) -> Result<Image> {
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
        let make = |pixels: Pixels| Image {
            inner: std::rc::Rc::new(Inner {
                width: w,
                height: h,
                resolution: (96, 96),
                pixels,
            }),
        };

        // Primer filtro que no es genérico: DCT u otro de imagen.
        let filters: Vec<Vec<u8>> = s
            .filters()
            .map(|f| f.into_iter().map(<[u8]>::to_vec).collect())
            .unwrap_or_default();
        const GENERIC: [&[u8]; 6] = [
            b"FlateDecode",
            b"LZWDecode",
            b"ASCII85Decode",
            b"ASCIIHexDecode",
            b"RunLengthDecode",
            b"BrotliDecode",
        ];
        let image_filter = filters.iter().find(|f| !GENERIC.contains(&f.as_slice()));
        let data = decode_stream(s)?;
        if let Some(f) = image_filter {
            if f.as_slice() == b"DCTDecode" {
                let j = parse_jpeg(&data).ok_or_else(|| invalid("JPEG incrustado no válido"))?;
                if j.precision != 8 || !matches!(j.comps, 1 | 3 | 4) {
                    return Ok(make(Pixels::Unsupported(format!("JPEG de {} bits", j.precision))));
                }
                return Ok(make(Pixels::Jpeg {
                    bytes: data,
                    comps: j.comps,
                    inverted: j.comps == 4 && j.adobe,
                }));
            }
            return Ok(make(Pixels::Unsupported(String::from_utf8_lossy(f).into_owned())));
        }

        let mask = d
            .get(b"ImageMask")
            .ok()
            .and_then(|v| st.deref(v))
            .is_some_and(|v| matches!(v, L::Boolean(true)));
        let space = if mask {
            Space::Gray
        } else {
            match d.get(b"ColorSpace").ok().and_then(|v| space_of(st, v, 0)) {
                Some(sp) => sp,
                None => return Ok(make(Pixels::Unsupported("espacio de color no admitido".into()))),
            }
        };
        let bpc = if mask {
            1
        } else {
            int(st, d, b"BitsPerComponent").unwrap_or(8) as usize
        };
        if !matches!(bpc, 1 | 2 | 4 | 8 | 16) {
            return Ok(make(Pixels::Unsupported(format!("{bpc} bits por componente"))));
        }
        let (wu, hu, comps) = (w as usize, h as usize, space.comps());
        let row_bytes = (wu * comps * bpc).div_ceil(8);
        let maxv = (1u64 << bpc) - 1;
        let invert = !matches!(space, Space::Indexed { .. })
            && match d.get(b"Decode").ok().and_then(|v| st.deref(v)) {
                Some(L::Array(a)) => {
                    a.len() >= 2
                        && a.chunks(2)
                            .all(|p| p.len() == 2 && num(st, p.first()) == Some(1.0) && num(st, p.get(1)) == Some(0.0))
                }
                _ => false,
            };

        let out_comps = space.cs().comps();
        let mut samples = Vec::with_capacity(wu * hu * out_comps);
        for y in 0..hu {
            let start = y * row_bytes;
            let row = data.get(start..).map_or(&[][..], |r| &r[..row_bytes.min(r.len())]);
            for x in 0..wu {
                match &space {
                    Space::Indexed { base, hival, table } => {
                        let idx = (sample_at(row, x, bpc) as usize).min(*hival);
                        let n = base.comps();
                        let entry = table.get(idx * n..idx * n + n);
                        for c in 0..n {
                            samples.push(entry.map_or(0, |e| e[c]));
                        }
                    }
                    _ => {
                        for c in 0..comps {
                            let v = sample_at(row, x * comps + c, bpc);
                            let mut b = ((u64::from(v) * 255) / maxv) as u8;
                            if invert {
                                b = 255 - b;
                            }
                            samples.push(b);
                        }
                    }
                }
            }
        }

        let alpha = if depth < MAX_DEPTH {
            d.get(b"SMask")
                .ok()
                .and_then(|v| st.deref(v))
                .and_then(|v| match v {
                    L::Stream(m) => Image::from_stream_at(st, m, false, depth + 1).ok(),
                    _ => None,
                })
                .and_then(|m| m.to_pixmap().ok())
                .filter(|p| p.n() == 1)
                .map(|p| resize_nearest(p.samples(), p.stride(), p.width() as usize, p.height() as usize, wu, hu))
        } else {
            None
        };
        Ok(make(Pixels::Raw {
            cs: space.cs(),
            samples,
            alpha,
        }))
    }
}

/// Reescala un plano de 1 byte por píxel al vecino más cercano.
fn resize_nearest(src: &[u8], stride: usize, sw: usize, sh: usize, dw: usize, dh: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(dw * dh);
    for y in 0..dh {
        let sy = (y * sh / dh).min(sh.saturating_sub(1));
        for x in 0..dw {
            let sx = (x * sw / dw).min(sw.saturating_sub(1));
            out.push(*src.get(sy * stride + sx).unwrap_or(&255));
        }
    }
    out
}
