//! Imágenes de mapa de bits (JPEG, PNG, TIFF, WebP, GIF, BMP) para meter en un PDF **sin MuPDF** (W40, bloque 8).
//!
//! - [`Raster::decode`] lee el archivo con crates de Rust (`image`, `tiff`): un [`Raster`] por imagen (un TIFF puede
//!   traer varias páginas). Los JPEG de 8 bits en gris, RGB o CMYK **no se recomprimen**: se guardan tal cual con
//!   `DCTDecode`. El resto se decodifica a gris, RGB o CMYK de 8 bits más un canal alfa opcional.
//! - [`PdfDocument::add_raster`] crea el `XObject` de imagen (con `SMask` si hay transparencia) y
//!   [`PdfPage::insert_raster`] lo coloca en un rectángulo de la página (espacio de página: origen arriba a la
//!   izquierda, como `Page::bounds`).
//!
//! Los flujos se guardan sin comprimir: se comprimen al guardar con `WriteOptions::set_compress(true)`.

use std::io::Cursor;

use lopdf::{Dictionary, Object as L, Stream};
use pliego_core::{Error, Result};

use super::{PdfDocument, PdfPage, pdf_err};
use crate::geom::{Matrix, Rect};

/// Límite de seguridad por imagen (≈ 300 MB en RGB), como el del visor.
const MAX_PIXELS: u64 = 100_000_000;

/// Una imagen lista para incrustar.
#[derive(Debug, Clone)]
pub struct Raster {
    width: u32,
    height: u32,
    pixels: Pixels,
    /// Canal alfa de 8 bits (`width * height`), si la imagen tiene transparencia real.
    alpha: Option<Vec<u8>>,
}

#[derive(Debug, Clone)]
enum Pixels {
    /// Flujo JPEG original (`components` = 1, 3 o 4). `inverted` = CMYK de Adobe (valores invertidos).
    Jpeg {
        data: Vec<u8>,
        components: u8,
        inverted: bool,
    },
    Gray(Vec<u8>),
    Rgb(Vec<u8>),
    Cmyk(Vec<u8>),
}

impl Raster {
    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// `true` si el JPEG original se incrusta tal cual.
    pub fn is_passthrough_jpeg(&self) -> bool {
        matches!(self.pixels, Pixels::Jpeg { .. })
    }

    /// `true` si tiene transparencia.
    pub fn has_alpha(&self) -> bool {
        self.alpha.is_some()
    }

    /// Formatos que reconoce [`Raster::decode`], por el principio del archivo.
    pub fn sniff(bytes: &[u8]) -> Option<&'static str> {
        if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
            Some("jpeg")
        } else if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
            Some("png")
        } else if bytes.len() > 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
            Some("webp")
        } else if bytes.starts_with(b"II*\0") || bytes.starts_with(b"MM\0*") {
            Some("tiff")
        } else if bytes.starts_with(b"GIF8") {
            Some("gif")
        } else if bytes.starts_with(b"BM") {
            Some("bmp")
        } else {
            None
        }
    }

    /// Decodifica una imagen; un TIFF multipágina da una por página.
    pub fn decode(bytes: &[u8]) -> Result<Vec<Raster>> {
        match Self::sniff(bytes) {
            Some("jpeg") => Ok(vec![decode_jpeg(bytes)?]),
            Some("tiff") => decode_tiff(bytes),
            Some(fmt) => {
                let format = match fmt {
                    "png" => image::ImageFormat::Png,
                    "webp" => image::ImageFormat::WebP,
                    "gif" => image::ImageFormat::Gif,
                    _ => image::ImageFormat::Bmp,
                };
                let img = image::load_from_memory_with_format(bytes, format)
                    .map_err(|e| Error::InvalidInput(format!("imagen {fmt} dañada ({e})")))?;
                Ok(vec![from_dynamic(img)?])
            }
            None => Err(Error::InvalidInput("formato de imagen no reconocido".into())),
        }
    }
}

fn check_size(w: u32, h: u32) -> Result<()> {
    if w == 0 || h == 0 || u64::from(w) * u64::from(h) > MAX_PIXELS {
        return Err(Error::InvalidInput(format!("tamaño de imagen no válido: {w}x{h}")));
    }
    Ok(())
}

// --- JPEG ---

struct JpegInfo {
    width: u32,
    height: u32,
    components: u8,
    precision: u8,
    /// Marcador SOF (0xC0 base, 0xC1 extendido, 0xC2 progresivo, otros no admitidos).
    sof: u8,
    adobe: bool,
}

fn jpeg_info(b: &[u8]) -> Option<JpegInfo> {
    let mut i = 2;
    let mut adobe = false;
    while i + 4 <= b.len() {
        if b[i] != 0xFF {
            return None;
        }
        let marker = b[i + 1];
        if marker == 0xFF {
            i += 1;
            continue;
        }
        if (0xD0..=0xD9).contains(&marker) || marker == 0x01 {
            i += 2;
            continue;
        }
        let len = usize::from(u16::from_be_bytes([b[i + 2], b[i + 3]]));
        let body = b.get(i + 4..i + 2 + len)?;
        if marker == 0xEE && body.starts_with(b"Adobe") {
            adobe = true;
        }
        if matches!(marker, 0xC0..=0xCF) && !matches!(marker, 0xC4 | 0xC8 | 0xCC) {
            if body.len() < 6 {
                return None;
            }
            return Some(JpegInfo {
                precision: body[0],
                height: u32::from(u16::from_be_bytes([body[1], body[2]])),
                width: u32::from(u16::from_be_bytes([body[3], body[4]])),
                components: body[5],
                sof: marker,
                adobe,
            });
        }
        i += 2 + len;
    }
    None
}

fn decode_jpeg(bytes: &[u8]) -> Result<Raster> {
    let broken = |what: &str| Error::InvalidInput(format!("imagen JPEG dañada ({what})"));
    let info = jpeg_info(bytes).ok_or_else(|| broken("sin cabecera"))?;
    check_size(info.width, info.height)?;
    let direct = info.precision == 8 && matches!(info.sof, 0xC0..=0xC2) && matches!(info.components, 1 | 3 | 4);
    if direct {
        // Se comprueba que se puede decodificar (no se incrusta un flujo roto), pero se guarda el original.
        if info.components != 4 {
            image::load_from_memory_with_format(bytes, image::ImageFormat::Jpeg).map_err(|e| broken(&e.to_string()))?;
        }
        return Ok(Raster {
            width: info.width,
            height: info.height,
            pixels: Pixels::Jpeg {
                data: bytes.to_vec(),
                components: info.components,
                inverted: info.components == 4 && info.adobe,
            },
            alpha: None,
        });
    }
    // JPEG de 12 bits, sin pérdida o aritmético: se decodifica y se guarda como píxeles.
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Jpeg)
        .map_err(|e| Error::Unsupported(format!("JPEG no admitido ({e})")))?;
    from_dynamic(img)
}

// --- Resto de formatos (crate `image`) ---

fn from_dynamic(img: image::DynamicImage) -> Result<Raster> {
    use image::ColorType as C;
    let (width, height) = (img.width(), img.height());
    check_size(width, height)?;
    let (pixels, alpha) = match img.color() {
        C::L8 | C::L16 => (Pixels::Gray(img.into_luma8().into_raw()), None),
        C::La8 | C::La16 => {
            let la = img.into_luma_alpha8().into_raw();
            let (g, a): (Vec<u8>, Vec<u8>) = la.chunks(2).map(|p| (p[0], p[1])).unzip();
            (Pixels::Gray(g), Some(a))
        }
        C::Rgba8 | C::Rgba16 | C::Rgba32F => split_rgba(img.into_rgba8().into_raw()),
        _ => (Pixels::Rgb(img.into_rgb8().into_raw()), None),
    };
    Ok(Raster {
        width,
        height,
        pixels,
        alpha: alpha.filter(|a| a.iter().any(|&v| v != 255)),
    })
}

fn split_rgba(raw: Vec<u8>) -> (Pixels, Option<Vec<u8>>) {
    let mut rgb = Vec::with_capacity(raw.len() / 4 * 3);
    let mut alpha = Vec::with_capacity(raw.len() / 4);
    for p in raw.chunks_exact(4) {
        rgb.extend_from_slice(&p[..3]);
        alpha.push(p[3]);
    }
    (Pixels::Rgb(rgb), Some(alpha))
}

// --- TIFF (crate `tiff`, páginas) ---

fn decode_tiff(bytes: &[u8]) -> Result<Vec<Raster>> {
    use tiff::ColorType as T;
    use tiff::decoder::{Decoder, DecodingResult as D};
    use tiff::tags::Tag;

    let broken = |e: tiff::TiffError| Error::InvalidInput(format!("imagen TIFF dañada ({e})"));
    let mut dec = Decoder::new(Cursor::new(bytes)).map_err(broken)?;
    let mut out = Vec::new();
    loop {
        let (width, height) = dec.dimensions().map_err(broken)?;
        check_size(width, height)?;
        let color = dec.colortype().map_err(broken)?;
        let palette = if matches!(color, T::Palette(_)) {
            dec.find_tag_unsigned_vec::<u16>(Tag::ColorMap).map_err(broken)?
        } else {
            None
        };
        let data = dec.read_image().map_err(broken)?;
        // Todo a 8 bits.
        let samples: Vec<u8> = match data {
            D::U8(v) => v,
            D::U16(v) => v.into_iter().map(|s| (s >> 8) as u8).collect(),
            _ => return Err(Error::Unsupported("TIFF con muestras de más de 16 bits".into())),
        };
        let n = (width as usize) * (height as usize);
        let bad = |what: &str| Error::InvalidInput(format!("imagen TIFF dañada ({what})"));
        let (pixels, alpha) = match color {
            T::Gray(8 | 16) if samples.len() == n => (Pixels::Gray(samples), None),
            T::Gray(1) => {
                // Cada fila va empaquetada en bits; la expansión a 8 bits la hace `tiff` solo con 8 bits,
                // así que aquí se desempaqueta (1 = blanco).
                let row = (width as usize).div_ceil(8);
                if samples.len() < row * height as usize {
                    return Err(bad("datos incompletos"));
                }
                let mut g = Vec::with_capacity(n);
                for y in 0..height as usize {
                    for x in 0..width as usize {
                        g.push(if samples[y * row + x / 8] >> (7 - x % 8) & 1 == 1 {
                            255
                        } else {
                            0
                        });
                    }
                }
                (Pixels::Gray(g), None)
            }
            T::GrayA(8 | 16) if samples.len() == n * 2 => {
                let (g, a): (Vec<u8>, Vec<u8>) = samples.chunks(2).map(|p| (p[0], p[1])).unzip();
                (Pixels::Gray(g), Some(a))
            }
            T::RGB(8 | 16) if samples.len() == n * 3 => (Pixels::Rgb(samples), None),
            T::RGBA(8 | 16) if samples.len() == n * 4 => split_rgba(samples),
            T::CMYK(8 | 16) if samples.len() == n * 4 => (Pixels::Cmyk(samples), None),
            T::Palette(8) if samples.len() == n => {
                let map = palette
                    .filter(|m| m.len() >= 3 * 256)
                    .ok_or_else(|| bad("sin paleta"))?;
                let mut rgb = Vec::with_capacity(n * 3);
                for &i in &samples {
                    let i = usize::from(i);
                    rgb.extend_from_slice(&[
                        (map[i] >> 8) as u8,
                        (map[256 + i] >> 8) as u8,
                        (map[512 + i] >> 8) as u8,
                    ]);
                }
                (Pixels::Rgb(rgb), None)
            }
            other => return Err(Error::Unsupported(format!("TIFF con color {other:?} no admitido"))),
        };
        out.push(Raster {
            width,
            height,
            pixels,
            alpha: alpha.filter(|a| a.iter().any(|&v| v != 255)),
        });
        if !dec.more_images() {
            break;
        }
        dec.next_image().map_err(broken)?;
    }
    Ok(out)
}

// --- Inserción en el documento ---

impl PdfDocument {
    /// Crea el `XObject` de imagen de `raster` (más su `SMask` si tiene transparencia) y devuelve la referencia.
    pub fn add_raster(&mut self, raster: &Raster) -> Result<crate::native::Object> {
        self.guard()?;
        let mut dict = Dictionary::new();
        dict.set("Type", L::Name(b"XObject".to_vec()));
        dict.set("Subtype", L::Name(b"Image".to_vec()));
        dict.set("Width", i64::from(raster.width));
        dict.set("Height", i64::from(raster.height));
        let (cs, data): (&[u8], &[u8]) = match &raster.pixels {
            Pixels::Jpeg {
                data,
                components,
                inverted,
            } => {
                dict.set("Filter", L::Name(b"DCTDecode".to_vec()));
                if *inverted {
                    dict.set("Decode", L::Array([1, 0, 1, 0, 1, 0, 1, 0].map(L::Integer).to_vec()));
                }
                let cs: &[u8] = match components {
                    1 => b"DeviceGray",
                    3 => b"DeviceRGB",
                    _ => b"DeviceCMYK",
                };
                (cs, data)
            }
            Pixels::Gray(d) => (b"DeviceGray", d),
            Pixels::Rgb(d) => (b"DeviceRGB", d),
            Pixels::Cmyk(d) => (b"DeviceCMYK", d),
        };
        dict.set("ColorSpace", L::Name(cs.to_vec()));
        dict.set("BitsPerComponent", 8i64);
        if let Some(alpha) = &raster.alpha {
            let mut m = Dictionary::new();
            m.set("Type", L::Name(b"XObject".to_vec()));
            m.set("Subtype", L::Name(b"Image".to_vec()));
            m.set("Width", i64::from(raster.width));
            m.set("Height", i64::from(raster.height));
            m.set("ColorSpace", L::Name(b"DeviceGray".to_vec()));
            m.set("BitsPerComponent", 8i64);
            let id = self.state().borrow_mut().add(L::Stream(Stream::new(m, alpha.clone())));
            dict.set("SMask", L::Reference(id));
        }
        let id = self
            .state()
            .borrow_mut()
            .add(L::Stream(Stream::new(dict, data.to_vec())));
        Ok(crate::native::Object::reference(self.state(), id))
    }
}

impl PdfPage {
    /// Dibuja `raster` ocupando `rect` (espacio de página: origen arriba a la izquierda, como `Page::bounds`).
    pub fn insert_raster(&mut self, doc: &mut PdfDocument, rect: Rect, raster: &Raster) -> Result<()> {
        if rect.is_empty()
            || !rect.x0.is_finite()
            || !rect.y0.is_finite()
            || !rect.x1.is_finite()
            || !rect.y1.is_finite()
        {
            return Err(Error::InvalidInput("rectángulo de imagen no válido".into()));
        }
        let image = doc.add_raster(raster)?;
        let xobject = L::Reference(
            image
                .obj_id()
                .ok_or_else(|| pdf_err("la imagen no es un objeto indirecto"))?,
        );
        // Recurso con un nombre libre dentro de /XObject.
        let res = self.resources()?;
        let xo = match res.get_dict("XObject")? {
            Some(x) if x.is_dict()? => x,
            _ => {
                res.dict_set(b"XObject", L::Dictionary(Dictionary::new()))?;
                res.get_dict("XObject")?
                    .ok_or_else(|| pdf_err("no se pudo crear /XObject"))?
            }
        };
        let mut n = 0usize;
        while xo.get_dict(&format!("Im{n}"))?.is_some() {
            n += 1;
        }
        let name = format!("Im{n}");
        xo.dict_set(name.as_bytes(), xobject)?;
        // La imagen ocupa el cuadrado unidad; la matriz lo lleva a `rect` en el espacio de la página y de ahí
        // al espacio de usuario del PDF (inversa de la matriz de la página).
        let to_page = Matrix::new(rect.width(), 0.0, 0.0, -rect.height(), rect.x0, rect.y1);
        let inv = self
            .ctm()?
            .invert()
            .ok_or_else(|| pdf_err("la matriz de la página no es invertible"))?;
        let mut m = to_page;
        m.concat(inv);
        let content = format!(
            "q\n{} {} {} {} {} {} cm\n/{name} Do\nQ\n",
            fnum(m.a),
            fnum(m.b),
            fnum(m.c),
            fnum(m.d),
            fnum(m.e),
            fnum(m.f)
        );
        self.insert_contents(doc, content.as_bytes(), true)?;
        Ok(())
    }
}

fn fnum(v: f32) -> String {
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-0" {
        "0".into()
    } else {
        s.into()
    }
}
