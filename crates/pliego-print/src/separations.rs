use std::collections::HashSet;

use pliego_core::{Error, Result};
use pliego_pdf::{Colorspace, Encryption, ImageFormat, Matrix, Object, PdfDocument, Pixmap, WriteOptions};
use serde::{Deserialize, Serialize};

use crate::content;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlateChannel {
    Cyan,
    Magenta,
    Yellow,
    Black,
}

impl PlateChannel {
    pub const ALL: [PlateChannel; 4] = [Self::Cyan, Self::Magenta, Self::Yellow, Self::Black];

    pub fn name(self) -> &'static str {
        match self {
            Self::Cyan => "Cyan",
            Self::Magenta => "Magenta",
            Self::Yellow => "Yellow",
            Self::Black => "Black",
        }
    }
}

/// Plancha en escala de grises: 0 = sin tinta, 255 = tinta al 100 %.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plate {
    pub channel: PlateChannel,
    pub width: u32,
    pub height: u32,
    pub samples: Vec<u8>,
}

impl Plate {
    /// Cobertura media de la plancha (0.0 a 1.0).
    pub fn coverage(&self) -> f32 {
        if self.samples.is_empty() {
            return 0.0;
        }
        self.samples.iter().map(|&v| v as u64).sum::<u64>() as f32 / (255.0 * self.samples.len() as f32)
    }

    /// PNG en escala de grises como una película de imprenta: negro donde hay tinta, blanco donde no.
    pub fn to_png(&self) -> Result<Vec<u8>> {
        let mut pix = Pixmap::new_with_w_h(Colorspace::Gray, self.width as i32, self.height as i32, false)?;
        let stride = pix.stride();
        let w = self.width as usize;
        let out = pix.samples_mut();
        for (y, row) in self.samples.chunks(w).enumerate() {
            for (x, v) in row.iter().enumerate() {
                out[y * stride + x] = 255 - v;
            }
        }
        let mut png = Vec::new();
        pix.write_to(&mut png, ImageFormat::Png)?;
        Ok(png)
    }
}

/// Planchas CMYK de una página y la cobertura total de tinta (TAC) máxima.
#[derive(Debug, Clone, PartialEq)]
pub struct Separations {
    pub plates: Vec<Plate>,
    /// Suma máxima de C+M+Y+K en un píxel (0.0 a 4.0); 3.0 equivale al 300 %.
    pub max_total_ink: f32,
}

/// Vista previa de separaciones de la página `page` (desde 0) a `dpi` ppp.
///
/// PDFium solo dibuja en RGB, así que las planchas se obtienen dibujando la página **cuatro veces**, una por tinta:
/// en una copia en memoria se reescribe todo lo que pinta para que valga «1 − cantidad de esa tinta» en gris, y se
/// invierte lo dibujado.
///
/// - Vectores y texto: se reescriben los operadores de color del contenido (`k`, `g`, `rg`, `sc`…), también en
///   formularios y patrones de mosaico.
/// - **Imágenes** (XObject y en línea; Gray, RGB, CMYK, ICC, Indexed, JPEG CMYK/YCCK de Adobe, con `/Decode`): se
///   sustituyen por un gris de 8 bits con la tinta de la plancha.
/// - **Degradados** (`sh` y patrones de tipo 2, con funciones de tipo 0, 2 y 3): su función pasa a dar el gris de la tinta.
///
/// Sigue sin separarse, y se dibuja como gris de luminosidad: las imágenes de JBIG2, CCITT, JPEG 2000, `Separation`,
/// `DeviceN` y `Lab`, los degradados con función PostScript o de malla, los patrones sin color (`PaintType 2`) y
/// las transparencias con máscara suave. El gris solo tiñe la plancha negra y el RGB se reparte con la conversión simple.
/// Los colores directos se reparten por su alternativa de tinta (ver [`crate::inks`] para listarlos).
pub fn separations(doc: &PdfDocument, page: usize, dpi: f32) -> Result<Separations> {
    if !(dpi.is_finite() && dpi > 0.0) {
        return Err(Error::InvalidInput("la resolución debe ser un número positivo".into()));
    }
    let count = doc.page_count()?.max(0) as usize;
    if page >= count {
        return Err(Error::InvalidInput(format!(
            "la página {} no existe (el documento tiene {count})",
            page + 1
        )));
    }
    let mut options = WriteOptions::new();
    options.set_encryption(Encryption::None);
    let bytes = doc.to_bytes(&options)?;
    let scale = dpi / 72.0;
    let (mut w, mut h) = (0usize, 0usize);
    let mut planes: Vec<Vec<u8>> = Vec::with_capacity(4);
    for channel in 0..4 {
        let mut copy = PdfDocument::from_bytes(&bytes)?;
        rewrite_page(&mut copy, page, channel)?;
        let p = copy.load_page(page as i32)?;
        let pix = p.to_pixmap(&Matrix::new_scale(scale, scale), Colorspace::Gray, false, false)?;
        (w, h) = (pix.width() as usize, pix.height() as usize);
        let stride = pix.stride();
        let data = pix.samples();
        let mut plane = vec![0u8; w * h];
        for y in 0..h {
            for (dst, &g) in plane[y * w..(y + 1) * w]
                .iter_mut()
                .zip(&data[y * stride..y * stride + w])
            {
                *dst = 255 - g;
            }
        }
        planes.push(plane);
    }
    let mut max_total = 0u32;
    for i in 0..w * h {
        max_total = max_total.max(planes.iter().map(|p| u32::from(p[i])).sum());
    }
    let plates = PlateChannel::ALL
        .into_iter()
        .zip(planes)
        .map(|(channel, samples)| Plate {
            channel,
            width: w as u32,
            height: h as u32,
            samples,
        })
        .collect();
    Ok(Separations {
        plates,
        max_total_ink: max_total as f32 / 255.0,
    })
}

/// Sustituye en la copia los colores de la página (y de sus formularios, patrones, imágenes y degradados) por el gris
/// de la tinta `channel`.
fn rewrite_page(doc: &mut PdfDocument, page: usize, channel: usize) -> Result<()> {
    let mut page_obj = doc.find_page(page as i32)?;
    let data = crate::scan::page_content(doc, page)?;
    // Sin color explícito el contenido es negro: no lleva tinta en C, M ni Y.
    let mut out = if channel < 3 { b"1 g 1 G\n".to_vec() } else { Vec::new() };
    out.extend(rewrite_content(doc, &data, channel));
    let stream = doc.add_stream(&out, None, false)?;
    page_obj.dict_put("Contents", stream)?;
    let resources = page_obj.get_dict_inheritable("Resources")?;
    let mut seen = HashSet::new();
    rewrite_resources(doc, resources.as_ref(), channel, &mut seen, 0)
}

/// `true` la primera vez que se ve un objeto indirecto (los directos cuentan siempre como nuevos).
fn first_visit(obj: &Object, seen: &mut HashSet<i32>) -> bool {
    match obj.as_indirect() {
        Ok(id) if id != 0 => seen.insert(id),
        _ => true,
    }
}

fn rewrite_resources(
    doc: &PdfDocument,
    resources: Option<&Object>,
    channel: usize,
    seen: &mut HashSet<i32>,
    depth: usize,
) -> Result<()> {
    let Some(resources) = resources else { return Ok(()) };
    if depth > 8 {
        return Ok(());
    }
    // Imágenes y formularios.
    if let Some(xobjects) = resources.get_dict("XObject").ok().flatten() {
        for (_, mut x) in xobjects.dict_iter()? {
            let subtype = x.get_dict("Subtype")?.and_then(|s| s.as_name().ok());
            if !x.is_stream()? || !first_visit(&x, seen) {
                continue;
            }
            match subtype.as_deref() {
                Some(b"Image") => {
                    // Lo que no se sabe decodificar queda como está: se dibuja en gris.
                    let _ = x.separate_image_ink(channel);
                }
                Some(b"Form") => {
                    let data = x.read_stream()?;
                    x.write_stream(&rewrite_content(doc, &data, channel))?;
                    let own = x.get_dict("Resources")?;
                    rewrite_resources(doc, own.as_ref().or(Some(resources)), channel, seen, depth + 1)?;
                }
                _ => {}
            }
        }
    }
    // Degradados de `sh`.
    if let Some(shadings) = resources.get_dict("Shading").ok().flatten() {
        for (_, sh) in shadings.dict_iter()? {
            if first_visit(&sh, seen) {
                let _ = sh.separate_shading_ink(channel);
            }
        }
    }
    // Patrones: de degradado (tipo 2) y de mosaico (tipo 1, que es un flujo de contenido con sus recursos).
    if let Some(patterns) = resources.get_dict("Pattern").ok().flatten() {
        for (_, mut pat) in patterns.dict_iter()? {
            if !first_visit(&pat, seen) {
                continue;
            }
            match pat.dict_get_int("PatternType")? {
                Some(2) => {
                    if let Some(sh) = pat.get_dict("Shading")?
                        && first_visit(&sh, seen)
                    {
                        let _ = sh.separate_shading_ink(channel);
                    }
                }
                Some(1) if pat.is_stream()? => {
                    let data = pat.read_stream()?;
                    pat.write_stream(&rewrite_content(doc, &data, channel))?;
                    let own = pat.get_dict("Resources")?;
                    rewrite_resources(doc, own.as_ref().or(Some(resources)), channel, seen, depth + 1)?;
                }
                _ => {}
            }
        }
    }
    Ok(())
}

/// Cantidad de tinta `channel` (0..4 = C, M, Y, K) de un color RGB con la conversión simple.
fn rgb_ink(r: f32, g: f32, b: f32, channel: usize) -> f32 {
    let k = 1.0 - r.max(g).max(b);
    if channel == 3 {
        return k;
    }
    if k >= 1.0 {
        return 0.0;
    }
    ([r, g, b][channel].mul_add(-1.0, 1.0 - k)) / (1.0 - k)
}

fn gray_for_ink(ink: f32) -> String {
    let v = (1.0 - ink).clamp(0.0, 1.0);
    format!("{v:.4}")
}

/// Reescribe los operadores de color de un flujo de contenido.
fn rewrite_content(doc: &PdfDocument, data: &[u8], channel: usize) -> Vec<u8> {
    let ops = content::parse(data);
    let mut out = Vec::with_capacity(data.len() + 64);
    let mut cursor = 0usize;
    for op in &ops {
        if op.name == "BI" {
            if let Some(gray) = doc.separate_inline_image(&data[op.range.0..op.range.1.min(data.len())], channel) {
                out.extend_from_slice(&data[cursor..op.range.0]);
                out.extend_from_slice(&gray);
                cursor = op.range.1.min(data.len());
            }
            continue;
        }
        let (Some(first), Some(last)) = (op.spans.first(), op.spans.last()) else {
            continue;
        };
        let stroke = op.name.chars().next().is_some_and(|c| c.is_ascii_uppercase());
        let nums = op.nums();
        let n_args = op.args.len();
        let all_numbers = nums.len() == n_args;
        let ink = match op.name.as_str() {
            "k" | "K" if nums.len() == 4 && all_numbers => Some(nums[channel]),
            "g" | "G" if nums.len() == 1 && all_numbers => Some(if channel == 3 { 1.0 - nums[0] } else { 0.0 }),
            "rg" | "RG" if nums.len() == 3 && all_numbers => Some(rgb_ink(nums[0], nums[1], nums[2], channel)),
            "sc" | "scn" | "SC" | "SCN" if all_numbers => match nums.len() {
                4 => Some(nums[channel]),
                3 => Some(rgb_ink(nums[0], nums[1], nums[2], channel)),
                _ => None,
            },
            _ => None,
        };
        // Posición del operador: tras el último operando (y los espacios que los separan).
        let mut op_start = last.1;
        while data
            .get(op_start)
            .is_some_and(|b| matches!(b, 0 | 9 | 10 | 12 | 13 | 32))
        {
            op_start += 1;
        }
        let op_end = op_start + op.name.len();
        if let Some(ink) = ink {
            out.extend_from_slice(&data[cursor..first.0]);
            out.extend_from_slice(gray_for_ink(ink).as_bytes());
            out.extend_from_slice(if stroke { b" G" } else { b" g" });
            cursor = op_end.min(data.len());
        } else if matches!(op.name.as_str(), "cs" | "CS")
            && matches!(op.name_arg(0), Some("DeviceCMYK" | "DeviceRGB" | "CMYK" | "RGB"))
        {
            out.extend_from_slice(&data[cursor..first.0]);
            out.extend_from_slice(b"/DeviceGray");
            out.extend_from_slice(&data[last.1..op_end.min(data.len())]);
            cursor = op_end.min(data.len());
        }
    }
    out.extend_from_slice(&data[cursor.min(data.len())..]);
    out
}
