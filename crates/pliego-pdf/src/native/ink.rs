//! Degradados (`/Shading`) reducidos a la cantidad de **una tinta**, para la vista previa de separaciones de
//! `pliego-print` (véase también `image/ink.rs`, que hace lo mismo con las imágenes).
//!
//! [`Object::separate_shading_ink`] reescribe el degradado en el sitio: su `/ColorSpace` pasa a `DeviceGray` y su
//! `/Function` se sustituye por otra de una sola salida con `1 − tinta` de la plancha pedida. Entiende funciones de
//! tipo 0 (muestreadas), 2 (exponenciales), 3 (a trozos) y una matriz de funciones de una salida (una por tinta, en
//! `DeviceCMYK`). Las de tipo 4 (PostScript), los espacios `Separation`/`DeviceN`/`Lab` y las mallas sin `/Function`
//! se dejan como están: devuelve `false`.

use pliego_core::Result;

use super::Object;
use super::image::ink::rgb_to_cmyk;

/// Espacio de color de dispositivo de un degradado.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Dev {
    Gray,
    Rgb,
    Cmyk,
    /// Un solo componente que **es** la cantidad de tinta (la función de una matriz de una salida por tinta).
    Ink,
}

impl Dev {
    fn comps(self) -> usize {
        match self {
            Dev::Gray | Dev::Ink => 1,
            Dev::Rgb => 3,
            Dev::Cmyk => 4,
        }
    }

    /// Cantidad de tinta (0..=1) de `channel` para un color con estos componentes (cada uno en 0..=1).
    fn ink(self, c: &[f32], channel: usize) -> f32 {
        let at = |i: usize| c.get(i).copied().unwrap_or(0.0).clamp(0.0, 1.0);
        match self {
            Dev::Gray => {
                if channel == 3 {
                    1.0 - at(0)
                } else {
                    0.0
                }
            }
            Dev::Rgb => {
                let b = |v: f32| (v * 255.0).round() as u8;
                f32::from(rgb_to_cmyk(b(at(0)), b(at(1)), b(at(2)))[channel]) / 255.0
            }
            Dev::Cmyk => at(channel),
            Dev::Ink => at(0),
        }
    }

    /// Valor de gris que hay que dibujar: `1 − tinta`.
    fn gray(self, c: &[f32], channel: usize) -> f32 {
        1.0 - self.ink(c, channel)
    }
}

fn device_of(cs: &Object) -> Option<Dev> {
    if cs.is_name().ok()? {
        return match cs.as_name().ok()?.as_slice() {
            b"DeviceGray" | b"G" | b"CalGray" => Some(Dev::Gray),
            b"DeviceRGB" | b"RGB" | b"CalRGB" => Some(Dev::Rgb),
            b"DeviceCMYK" | b"CMYK" => Some(Dev::Cmyk),
            _ => None,
        };
    }
    if !cs.is_array().ok()? {
        return None;
    }
    let first = cs.get_array(0).ok()??;
    match first.as_name().ok()?.as_slice() {
        b"ICCBased" => match cs.get_array(1).ok()??.get_dict("N").ok()??.as_int().ok()? {
            1 => Some(Dev::Gray),
            3 => Some(Dev::Rgb),
            4 => Some(Dev::Cmyk),
            _ => None,
        },
        b"CalGray" => Some(Dev::Gray),
        b"CalRGB" => Some(Dev::Rgb),
        b"DeviceGray" | b"DeviceRGB" | b"DeviceCMYK" => device_of(&first),
        _ => None,
    }
}

fn floats(o: &Object) -> Option<Vec<f32>> {
    if !o.is_array().ok()? {
        return None;
    }
    (0..o.len().ok()?)
        .map(|i| o.get_array(i as i32).ok()?.and_then(|n| n.as_float().ok()))
        .collect()
}

fn array_of(st: &Object, values: &[f32]) -> Result<Object> {
    let text: Vec<String> = values.iter().map(|v| format!("{v:.5}")).collect();
    Object::parse_in(&st.st, &format!("[{}]", text.join(" ")))
}

fn fn_type(f: &Object) -> Option<i32> {
    f.get_dict("FunctionType").ok()??.as_int().ok()
}

/// Comprueba que la función (y las de dentro) se pueden convertir.
fn can_convert(f: &Object, dev: Dev, depth: usize) -> bool {
    if depth > 8 {
        return false;
    }
    match fn_type(f) {
        Some(2) => {
            let n = dev.comps();
            let len = |k: &str| f.get_dict(k).ok().flatten().and_then(|a| floats(&a)).map(|v| v.len());
            len("C0").unwrap_or(1) == n && len("C1").unwrap_or(1) == n
        }
        Some(3) => f.get_dict("Functions").ok().flatten().is_some_and(|fs| {
            let list = fs.array_iter().unwrap_or_default();
            !list.is_empty() && list.iter().all(|g| can_convert(g, dev, depth + 1))
        }),
        Some(0) => {
            let range = f.get_dict("Range").ok().flatten().and_then(|r| floats(&r));
            let bps = f.get_dict("BitsPerSample").ok().flatten().and_then(|b| b.as_int().ok());
            range.is_some_and(|r| r.len() == dev.comps() * 2) && matches!(bps, Some(1 | 2 | 4 | 8 | 12 | 16 | 24 | 32))
        }
        _ => false,
    }
}

/// Convierte la función en el sitio a una de una salida con `1 − tinta`. Antes hay que haber comprobado `can_convert`.
fn convert(f: &Object, dev: Dev, channel: usize) -> Result<()> {
    let mut f = f.clone();
    let get = |k: &str| f.get_dict(k).ok().flatten().and_then(|a| floats(&a));
    match fn_type(&f) {
        Some(2) => {
            let c0 = get("C0").unwrap_or_else(|| vec![0.0; dev.comps()]);
            let c1 = get("C1").unwrap_or_else(|| vec![1.0; dev.comps()]);
            let (g0, g1) = (dev.gray(&c0, channel), dev.gray(&c1, channel));
            let (a0, a1) = (array_of(&f, &[g0])?, array_of(&f, &[g1])?);
            f.dict_put("C0", a0)?;
            f.dict_put("C1", a1)?;
        }
        Some(3) => {
            let fs = f
                .get_dict("Functions")?
                .map(|a| a.array_iter())
                .transpose()?
                .unwrap_or_default();
            for g in &fs {
                convert(g, dev, channel)?;
            }
        }
        Some(0) => {
            let range = get("Range").unwrap_or_default();
            let decode = get("Decode")
                .filter(|d| d.len() == range.len())
                .unwrap_or_else(|| range.clone());
            let bps = f
                .get_dict("BitsPerSample")?
                .map(|b| b.as_int())
                .transpose()?
                .unwrap_or(8) as usize;
            let n = dev.comps();
            let data = f.read_stream()?;
            let maxv = ((1u64 << bps) - 1) as f64;
            let total_bits = data.len() * 8;
            let tuples = total_bits / (bps * n);
            let read = |idx: usize| -> u64 {
                let mut v = 0u64;
                for bit in idx * bps..(idx + 1) * bps {
                    let byte = data.get(bit / 8).copied().unwrap_or(0);
                    v = (v << 1) | u64::from((byte >> (7 - bit % 8)) & 1);
                }
                v
            };
            let mut out = Vec::with_capacity(tuples);
            let mut color = vec![0.0f32; n];
            for t in 0..tuples {
                for (c, slot) in color.iter_mut().enumerate() {
                    let raw = read(t * n + c) as f64 / maxv;
                    let (lo, hi) = (decode[c * 2], decode[c * 2 + 1]);
                    *slot = lo + (raw as f32) * (hi - lo);
                }
                out.push((dev.gray(&color, channel).clamp(0.0, 1.0) * 255.0).round() as u8);
            }
            f.dict_put("Range", array_of(&f, &[0.0, 1.0])?)?;
            f.dict_put_int("BitsPerSample", 8)?;
            f.dict_delete("Decode")?;
            f.dict_delete("DecodeParms")?;
            f.write_stream(&out)?;
        }
        _ => {}
    }
    // Una salida: el `/Range` de los tipos 2 y 3 hablaba de n componentes.
    if matches!(fn_type(&f), Some(2 | 3)) && f.get_dict("Range")?.is_some() {
        f.dict_put("Range", array_of(&f, &[0.0, 1.0])?)?;
    }
    Ok(())
}

impl Object {
    /// Sustituye este degradado por uno `DeviceGray` que vale `1 − tinta` de la plancha `channel` (0 = cian,
    /// 1 = magenta, 2 = amarillo, 3 = negro). `false` si no se puede convertir (véase el comentario del módulo): queda
    /// como estaba. Llamarlo dos veces sobre el mismo objeto lo estropea: quien recorra el documento debe recordar
    /// qué objetos ya ha tratado.
    pub fn separate_shading_ink(&self, channel: usize) -> Result<bool> {
        if channel > 3 {
            return Err(pliego_core::Error::InvalidInput(
                "la plancha debe estar entre 0 y 3".into(),
            ));
        }
        let Some(dev) = self.get_dict("ColorSpace")?.and_then(|cs| device_of(&cs)) else {
            return Ok(false);
        };
        let Some(function) = self.get_dict("Function")? else {
            return Ok(false);
        };
        if function.is_array()? {
            // Una función de una salida por componente: solo se entiende la de la propia tinta en CMYK.
            if dev != Dev::Cmyk || function.len()? != 4 {
                return Ok(false);
            }
            let Some(own) = function.get_array(channel as i32)? else {
                return Ok(false);
            };
            if !can_convert_single(&own) {
                return Ok(false);
            }
            // La salida de esa función **es** la tinta: se reinterpreta como un componente de un espacio de gris
            // cuyo valor es `1 − tinta`.
            convert_ink_function(&own)?;
            let mut shading = self.clone();
            shading.dict_put("Function", own)?;
            shading.dict_put_name("ColorSpace", "DeviceGray")?;
            shading.dict_delete("Background")?;
            return Ok(true);
        }
        if !can_convert(&function, dev, 0) {
            return Ok(false);
        }
        convert(&function, dev, channel)?;
        let mut shading = self.clone();
        if let Some(bg) = shading.get_dict("Background")?.and_then(|b| floats(&b)) {
            let value = dev.gray(&bg, channel);
            shading.dict_put("Background", array_of(self, &[value])?)?;
        }
        shading.dict_put_name("ColorSpace", "DeviceGray")?;
        Ok(true)
    }
}

/// Función de una salida de las que se acompañan en una matriz (una por tinta): tipos 2 y 3 con salida simple.
fn can_convert_single(f: &Object) -> bool {
    can_convert(f, Dev::Ink, 0)
}

/// La salida es una tinta `v`; se quiere `1 − v`.
fn convert_ink_function(f: &Object) -> Result<()> {
    convert(f, Dev::Ink, 0)
}
