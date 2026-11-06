//! Imágenes del motor propio: decodificar, incrustar, listar, extraer, insertar, sustituir y quitar (bloque 5 de la
//! ola 5, W39).
//!
//! Misma API que `image.rs` de la raíz (MuPDF). Diferencias, apuntadas en `docs/handoff/flujos/w39.md`:
//! - Solo se decodifican **PNG y JPEG** (crate `image` sin el resto de formatos). GIF, TIFF, BMP, WebP y JPEG 2000
//!   devuelven [`Error::Unsupported`]; quien los necesite los convierte antes (`pliego-convert` ya lo hace).
//! - Un JPEG se incrusta **tal cual** (`/DCTDecode`, sin recomprimir), como hace MuPDF. Un PNG se incrusta como
//!   muestras de 8 bits comprimidas con Flate en `/DeviceGray` o `/DeviceRGB`, con el canal alfa en un `/SMask`
//!   (MuPDF las deja sin comprimir y con un perfil ICC).
//! - [`Image::mask`] siempre es `None` y [`Image::n`] no cuenta el alfa, igual que MuPDF con una imagen recién
//!   decodificada.
//! - `image_placements` (recorrer el contenido de la página) llega con el intérprete de contenido (PR 4).

use std::rc::Rc;

use lopdf::{Dictionary, Object as L, ObjectId, Stream};
use pliego_core::{Error, Result};

use super::appearance::n;
use super::font::flate_stream;

mod decode;
pub(crate) mod ink;
use super::{Object, PdfDocument, PdfPage};
use crate::geom::Rect;
use crate::pixmap::{Colorspace, Pixmap};

/// Lado máximo de píxeles totales que se acepta (evita desbordes con cabeceras manipuladas).
const MAX_PIXELS: u64 = 400_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cs {
    Gray,
    Rgb,
    Cmyk,
}

impl Cs {
    fn comps(self) -> usize {
        match self {
            Cs::Gray => 1,
            Cs::Rgb => 3,
            Cs::Cmyk => 4,
        }
    }
    fn pdf(self) -> &'static str {
        match self {
            Cs::Gray => "DeviceGray",
            Cs::Rgb => "DeviceRGB",
            Cs::Cmyk => "DeviceCMYK",
        }
    }
}

#[derive(Debug)]
enum Pixels {
    /// JPEG sin tocar: se incrusta con `/DCTDecode`.
    Jpeg { bytes: Vec<u8>, comps: u8, inverted: bool },
    /// Imagen de un PDF con un filtro o espacio de color que el motor propio no decodifica (JBIG2, CCITT, JPEG 2000,
    /// Separation…): se conocen sus dimensiones, pero no se puede pasar a píxeles ni reincrustar.
    Unsupported(String),
    /// Muestras de 8 bits entrelazadas, sin alfa, más el alfa aparte.
    Raw {
        cs: Cs,
        samples: Vec<u8>,
        alpha: Option<Vec<u8>>,
    },
}

#[derive(Debug)]
struct Inner {
    width: u32,
    height: u32,
    resolution: (i32, i32),
    pixels: Pixels,
}

/// Imagen decodificada o a punto de decodificarse. Es barata de clonar y no es `Send`.
#[derive(Debug, Clone)]
pub struct Image {
    inner: Rc<Inner>,
}

fn invalid(msg: impl Into<String>) -> Error {
    Error::InvalidInput(msg.into())
}

fn check_size(w: u32, h: u32) -> Result<()> {
    if w == 0 || h == 0 || u64::from(w) * u64::from(h) > MAX_PIXELS {
        return Err(invalid(format!("tamaño de imagen no válido: {w}x{h}")));
    }
    Ok(())
}

fn be16(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(b.get(at..at + 2)?.try_into().ok()?))
}

fn be32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

struct JpegInfo {
    width: u32,
    height: u32,
    comps: u8,
    precision: u8,
    dpi: Option<(i32, i32)>,
    adobe: bool,
}

/// Lee de las cabeceras JPEG lo justo para incrustarlo sin decodificar.
fn parse_jpeg(b: &[u8]) -> Option<JpegInfo> {
    if !b.starts_with(&[0xff, 0xd8]) {
        return None;
    }
    let (mut dpi, mut adobe) = (None, false);
    let mut i = 2;
    while i + 4 <= b.len() {
        if b[i] != 0xff {
            return None;
        }
        let marker = b[i + 1];
        if marker == 0xff {
            i += 1;
            continue;
        }
        if marker == 0x01 || (0xd0..=0xd8).contains(&marker) {
            i += 2;
            continue;
        }
        let len = usize::from(be16(b, i + 2)?);
        let seg = b.get(i + 4..i + 2 + len)?;
        match marker {
            0xe0 if seg.starts_with(b"JFIF\0") && seg.len() >= 12 => {
                let (units, xd, yd) = (seg[7], i32::from(be16(seg, 8)?), i32::from(be16(seg, 10)?));
                dpi = match units {
                    1 if xd > 0 && yd > 0 => Some((xd, yd)),
                    2 if xd > 0 && yd > 0 => {
                        Some(((xd as f32 * 2.54).round() as i32, (yd as f32 * 2.54).round() as i32))
                    }
                    _ => None,
                };
            }
            0xee if seg.starts_with(b"Adobe") => adobe = true,
            0xc0..=0xcf if !matches!(marker, 0xc4 | 0xc8 | 0xcc) => {
                return Some(JpegInfo {
                    precision: *seg.first()?,
                    height: u32::from(be16(seg, 1)?),
                    width: u32::from(be16(seg, 3)?),
                    comps: *seg.get(5)?,
                    dpi,
                    adobe,
                });
            }
            0xd9 | 0xda => return None,
            _ => {}
        }
        i += 2 + len;
    }
    None
}

/// Resolución declarada en un PNG (`pHYs` en metros → ppp).
fn png_dpi(b: &[u8]) -> Option<(i32, i32)> {
    let mut i = 8;
    while i + 12 <= b.len() {
        let len = be32(b, i)? as usize;
        let kind = b.get(i + 4..i + 8)?;
        if kind == b"pHYs" {
            let d = b.get(i + 8..i + 8 + len)?;
            if d.len() >= 9 && d[8] == 1 {
                let (x, y) = (be32(d, 0)? as f32 * 0.0254, be32(d, 4)? as f32 * 0.0254);
                return Some((x.round() as i32, y.round() as i32));
            }
            return None;
        }
        if kind == b"IDAT" || kind == b"IEND" {
            return None;
        }
        i += 12 + len;
    }
    None
}

impl Image {
    fn make(width: u32, height: u32, resolution: (i32, i32), pixels: Pixels) -> Result<Image> {
        check_size(width, height)?;
        Ok(Image {
            inner: Rc::new(Inner {
                width,
                height,
                resolution,
                pixels,
            }),
        })
    }

    /// Decodifica PNG y JPEG (los demás formatos no están en el motor propio).
    pub fn from_bytes(bytes: &[u8]) -> Result<Image> {
        if let Some(j) = parse_jpeg(bytes) {
            if j.precision != 8 || !matches!(j.comps, 1 | 3 | 4) {
                return Err(Error::Unsupported(format!(
                    "JPEG de {} bits y {} componentes",
                    j.precision, j.comps
                )));
            }
            return Image::make(
                j.width,
                j.height,
                j.dpi.unwrap_or((96, 96)),
                Pixels::Jpeg {
                    bytes: bytes.to_vec(),
                    comps: j.comps,
                    inverted: j.comps == 4 && j.adobe,
                },
            );
        }
        if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            let dynamic = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
                .map_err(|e| invalid(format!("PNG no válido: {e}")))?;
            let (w, h) = (dynamic.width(), dynamic.height());
            check_size(w, h)?;
            let color = dynamic.color();
            let gray = color.channel_count() <= 2;
            let (cs, samples, alpha) = match (gray, color.has_alpha()) {
                (true, false) => (Cs::Gray, dynamic.to_luma8().into_raw(), None),
                (false, false) => (Cs::Rgb, dynamic.to_rgb8().into_raw(), None),
                (true, true) => {
                    let (g, a) = dynamic
                        .to_luma_alpha8()
                        .into_raw()
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|p| (p[0], p[1]))
                        .unzip();
                    (Cs::Gray, g, Some(a))
                }
                (false, true) => {
                    let (c, a) = split_alpha(&dynamic.to_rgba8().into_raw(), 3);
                    (Cs::Rgb, c, Some(a))
                }
            };
            // Un alfa opaco del todo no hace falta.
            let alpha = alpha.filter(|a: &Vec<u8>| a.iter().any(|v| *v != 255));
            return Image::make(
                w,
                h,
                png_dpi(bytes).unwrap_or((96, 96)),
                Pixels::Raw { cs, samples, alpha },
            );
        }
        Err(Error::Unsupported(
            "formato de imagen no admitido por el motor propio (solo PNG y JPEG)".into(),
        ))
    }

    pub fn from_file(path: impl AsRef<std::path::Path>) -> Result<Image> {
        let path = path.as_ref();
        let data = std::fs::read(path).map_err(|e| Error::io_at(path, e))?;
        Image::from_bytes(&data)
    }

    pub fn from_pixmap(pixmap: &Pixmap) -> Result<Image> {
        let (w, h) = (pixmap.width(), pixmap.height());
        check_size(w, h)?;
        let cs_in = pixmap.colorspace();
        let comps = usize::from(cs_in.n());
        let n_all = comps + usize::from(pixmap.alpha());
        let (stride, data) = (pixmap.stride(), pixmap.samples());
        let mut samples = Vec::with_capacity(w as usize * h as usize * comps);
        let mut alpha = pixmap.alpha().then(|| Vec::with_capacity(w as usize * h as usize));
        for y in 0..h as usize {
            let row = data
                .get(y * stride..y * stride + w as usize * n_all)
                .ok_or_else(|| invalid("el mapa de bits es más corto de lo que indica su tamaño"))?;
            for px in row.chunks_exact(n_all) {
                if cs_in == Colorspace::Bgr {
                    samples.extend_from_slice(&[px[2], px[1], px[0]]);
                } else {
                    samples.extend_from_slice(&px[..comps]);
                }
                if let Some(a) = alpha.as_mut() {
                    a.push(px[comps]);
                }
            }
        }
        let cs = match cs_in {
            Colorspace::Gray => Cs::Gray,
            Colorspace::Rgb | Colorspace::Bgr => Cs::Rgb,
            Colorspace::Cmyk => Cs::Cmyk,
        };
        Image::make(w, h, (96, 96), Pixels::Raw { cs, samples, alpha })
    }

    pub fn width(&self) -> u32 {
        self.inner.width
    }
    pub fn height(&self) -> u32 {
        self.inner.height
    }
    /// Componentes de color por píxel (sin contar el alfa, que va aparte, como en MuPDF).
    pub fn n(&self) -> u8 {
        match &self.inner.pixels {
            Pixels::Jpeg { comps, .. } => *comps,
            Pixels::Raw { cs, .. } => cs.comps() as u8,
            Pixels::Unsupported(_) => 3,
        }
    }
    pub fn bits_per_component(&self) -> u8 {
        8
    }
    /// Resolución (ppp horizontal, vertical) declarada; 96 si el archivo no dice nada.
    pub fn resolution(&self) -> (i32, i32) {
        self.inner.resolution
    }
    /// Máscara de la imagen: una imagen recién decodificada no la expone (como MuPDF).
    pub fn mask(&self) -> Option<Image> {
        None
    }

    /// Decodifica a un mapa de bits (con alfa si el PNG lo traía).
    pub fn to_pixmap(&self) -> Result<Pixmap> {
        let (w, h) = (self.inner.width, self.inner.height);
        match &self.inner.pixels {
            Pixels::Jpeg { bytes, comps, .. } => {
                let dynamic = image::load_from_memory_with_format(bytes, image::ImageFormat::Jpeg)
                    .map_err(|e| invalid(format!("JPEG no válido: {e}")))?;
                if *comps == 1 {
                    Pixmap::from_samples(Colorspace::Gray, w, h, false, dynamic.to_luma8().into_raw())
                } else {
                    Pixmap::from_samples(Colorspace::Rgb, w, h, false, dynamic.to_rgb8().into_raw())
                }
            }
            Pixels::Unsupported(what) => Err(Error::Unsupported(format!("imagen con {what}"))),
            Pixels::Raw { cs, samples, alpha } => {
                let colorspace = match cs {
                    Cs::Gray => Colorspace::Gray,
                    Cs::Rgb => Colorspace::Rgb,
                    Cs::Cmyk => Colorspace::Cmyk,
                };
                match alpha {
                    None => Pixmap::from_samples(colorspace, w, h, false, samples.clone()),
                    Some(a) => {
                        let c = cs.comps();
                        let mut out = Vec::with_capacity(samples.len() + a.len());
                        for (px, al) in samples.chunks_exact(c).zip(a) {
                            out.extend_from_slice(px);
                            out.push(*al);
                        }
                        Pixmap::from_samples(colorspace, w, h, true, out)
                    }
                }
            }
        }
    }
}

/// Separa el último canal de píxeles de `n_color + 1` bytes.
fn split_alpha(rgba: &[u8], n_color: usize) -> (Vec<u8>, Vec<u8>) {
    let step = n_color + 1;
    let mut color = Vec::with_capacity(rgba.len() / step * n_color);
    let mut alpha = Vec::with_capacity(rgba.len() / step);
    for px in rgba.chunks_exact(step) {
        color.extend_from_slice(&px[..n_color]);
        alpha.push(px[n_color]);
    }
    (color, alpha)
}

/// Origen de la imagen que se inserta en una página.
pub enum ImageSource<'a> {
    Image(&'a Image),
    Pixmap(&'a Pixmap),
    /// Bytes de un archivo de imagen; `format_hint` es la extensión si se conoce (aquí se detecta por contenido).
    Bytes {
        data: &'a [u8],
        format_hint: Option<&'a str>,
    },
    /// Un objeto de imagen que ya está en el documento.
    ExistingXref(i32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InsertImageOptions {
    /// Encima (`true`, por defecto) o debajo del contenido existente.
    pub overlay: bool,
    pub opacity: Option<f32>,
    /// Número de objeto de la capa a la que pertenece.
    pub optional_content: Option<i32>,
}

impl Default for InsertImageOptions {
    fn default() -> Self {
        Self {
            overlay: true,
            opacity: None,
            optional_content: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImagePlacement {
    /// Nombre de recurso de la imagen en la página.
    pub name: String,
    pub xref: i32,
    pub rect: Rect,
    pub contents_xref: i32,
}

/// Imagen de los recursos de una página.
#[derive(Debug, Clone, PartialEq)]
pub struct PageImageInfo {
    pub name: String,
    pub xref: i32,
    pub width: u32,
    pub height: u32,
    pub bits_per_component: Option<i32>,
    pub color_space: Option<String>,
    /// Filtro principal (`DCTDecode`, `JPXDecode`, `FlateDecode`…).
    pub filter: Option<String>,
}

/// Imagen tal como está codificada en el PDF (p. ej. los bytes de un JPEG).
#[derive(Debug, Clone, PartialEq)]
pub struct ExtractedImage {
    pub xref: i32,
    pub width: u32,
    pub height: u32,
    pub bits_per_component: Option<i32>,
    pub color_space: Option<String>,
    pub filter: Option<String>,
    pub encoded: Vec<u8>,
}

fn name(s: &str) -> L {
    L::Name(s.as_bytes().to_vec())
}

fn image_dict(w: u32, h: u32, cs: Cs) -> Dictionary {
    let mut d = Dictionary::new();
    d.set("Type", name("XObject"));
    d.set("Subtype", name("Image"));
    d.set("Width", L::Integer(i64::from(w)));
    d.set("Height", L::Integer(i64::from(h)));
    d.set("ColorSpace", name(cs.pdf()));
    d.set("BitsPerComponent", L::Integer(8));
    d
}

impl PdfDocument {
    /// Imagen codificada del objeto `xref`: los bytes del JPEG/JPEG 2000 tal cual, o las muestras ya decodificadas
    /// de cualquier otro filtro (`filter` = `None`).
    pub fn extract_image(&self, xref: i32) -> Result<ExtractedImage> {
        let obj = self.new_indirect(xref, 0)?;
        let Some(dict) = obj.resolve()? else {
            return Err(invalid(format!("el objeto {xref} no existe")));
        };
        let is_image = dict.dict_get_name("Subtype")?.as_deref() == Some("Image");
        if !is_image || !obj.is_stream()? {
            return Err(invalid(format!("el objeto {xref} no es una imagen")));
        }
        let dim = |k: &str| -> Result<u32> { Ok(u32::try_from(dict.dict_get_int(k)?.unwrap_or(0)).unwrap_or(0)) };
        let filter = dict.get_dict("Filter")?.map(|f| f.to_pdf_string());
        let keep_encoded = filter
            .as_deref()
            .is_some_and(|f| (f.contains("DCTDecode") || f.contains("JPXDecode")) && !f.contains(' '));
        Ok(ExtractedImage {
            xref,
            width: dim("Width")?,
            height: dim("Height")?,
            bits_per_component: dict.dict_get_int("BitsPerComponent")?,
            color_space: dict.get_dict("ColorSpace")?.map(|c| c.to_pdf_string()),
            filter: keep_encoded.then_some(filter).flatten(),
            encoded: if keep_encoded {
                obj.read_raw_stream()?
            } else {
                obj.read_stream()?
            },
        })
    }

    /// Añade una imagen como XObject y devuelve su referencia.
    pub fn add_image(&mut self, image: &Image) -> Result<Object> {
        self.guard()?;
        let (w, h) = (image.inner.width, image.inner.height);
        let mut st = self.state().borrow_mut();
        let id: ObjectId = match &image.inner.pixels {
            Pixels::Jpeg { bytes, comps, inverted } => {
                let cs = match comps {
                    1 => Cs::Gray,
                    3 => Cs::Rgb,
                    _ => Cs::Cmyk,
                };
                let mut d = image_dict(w, h, cs);
                d.set("Filter", name("DCTDecode"));
                if *inverted {
                    d.set("Decode", L::Array([1, 0, 1, 0, 1, 0, 1, 0].map(L::Integer).to_vec()));
                }
                st.add(L::Stream(Stream::new(d, bytes.clone())))
            }
            Pixels::Unsupported(what) => return Err(Error::Unsupported(format!("imagen con {what}"))),
            Pixels::Raw { cs, samples, alpha } => {
                let mut d = image_dict(w, h, *cs);
                if let Some(a) = alpha {
                    let m = image_dict(w, h, Cs::Gray);
                    let mid = st.add(L::Stream(flate_stream(m, a)));
                    d.set("SMask", L::Reference(mid));
                }
                st.add(L::Stream(flate_stream(d, samples)))
            }
        };
        drop(st);
        Ok(Object::reference(self.state(), id))
    }
}

impl PdfDocument {
    /// Resuelve el origen a un objeto de imagen del documento.
    fn image_xref(&mut self, source: ImageSource<'_>) -> Result<i32> {
        match source {
            ImageSource::ExistingXref(x) => {
                let o = self
                    .xref_object(x)?
                    .ok_or_else(|| invalid(format!("el objeto {x} no existe")))?;
                let is_image = o.is_dict()? || o.is_stream()?;
                if !is_image || o.dict_get_name("Subtype")?.as_deref() != Some("Image") {
                    return Err(invalid(format!("el objeto {x} no es una imagen")));
                }
                Ok(x)
            }
            ImageSource::Image(i) => self.add_image(i)?.as_indirect(),
            ImageSource::Pixmap(p) => self.add_image(&Image::from_pixmap(p)?)?.as_indirect(),
            ImageSource::Bytes { data, .. } => self.add_image(&Image::from_bytes(data)?)?.as_indirect(),
        }
    }
}

impl PdfPage {
    /// Entradas `(nombre, objeto)` de `/Resources /XObject` que son imágenes.
    fn xobject_images(&self) -> Result<Vec<(String, Object)>> {
        let Some(res) = self.page.get_dict_inheritable("Resources")? else {
            return Ok(Vec::new());
        };
        let Some(xo) = res.get_dict("XObject")? else {
            return Ok(Vec::new());
        };
        let mut out = Vec::new();
        for (key, val) in xo.dict_iter()? {
            let Some(target) = val.resolve()? else { continue };
            if target.is_dict()? && target.dict_get_name("Subtype")?.as_deref() == Some("Image") {
                out.push((key, val));
            }
        }
        Ok(out)
    }

    /// Imágenes de los recursos de la página.
    pub fn images(&self) -> Result<Vec<PageImageInfo>> {
        let mut out = Vec::new();
        for (name, val) in self.xobject_images()? {
            let Some(d) = val.resolve()? else { continue };
            let dim = |k: &str| -> Result<u32> { Ok(u32::try_from(d.dict_get_int(k)?.unwrap_or(0)).unwrap_or(0)) };
            out.push(PageImageInfo {
                name,
                xref: if val.is_indirect()? { val.as_indirect()? } else { 0 },
                width: dim("Width")?,
                height: dim("Height")?,
                bits_per_component: d.dict_get_int("BitsPerComponent")?,
                color_space: d.get_dict("ColorSpace")?.map(|c| c.to_pdf_string()),
                filter: d.get_dict("Filter")?.map(|f| f.to_pdf_string()),
            });
        }
        Ok(out)
    }

    /// Inserta una imagen que ocupa `rect` (espacio de usuario de la página).
    pub fn insert_image(
        &mut self,
        doc: &mut PdfDocument,
        rect: Rect,
        source: ImageSource<'_>,
        options: InsertImageOptions,
    ) -> Result<ImagePlacement> {
        doc.guard()?;
        if !(rect.x1 > rect.x0 && rect.y1 > rect.y0) {
            return Err(invalid("image insertion requires a non-empty rectangle"));
        }
        let xref = doc.image_xref(source)?;
        let img_ref = doc.new_indirect(xref, 0)?;

        let res = self.resources()?;
        // Nombre de recurso: el que ya apunta a esta imagen, o el primer `ImN` libre.
        let mut xo = sub_dict(doc, &res, "XObject")?;
        let mut taken = Vec::new();
        let mut name_in_use = None;
        for (key, val) in xo.dict_iter()? {
            if val.is_indirect()? && val.as_indirect()? == xref {
                name_in_use = Some(key.clone());
            }
            taken.push(key);
        }
        let img_name = match name_in_use {
            Some(n) => n,
            None => {
                let n = free_name("Im", &taken);
                xo.dict_put(&n, img_ref)?;
                n
            }
        };

        let mut content = String::from("q\n");
        if let Some(o) = options.opacity {
            let mut ext = sub_dict(doc, &res, "ExtGState")?;
            let existing: Vec<String> = ext.dict_iter()?.into_iter().map(|(k, _)| k).collect();
            let mut found = None;
            for (k, g) in ext.dict_iter()? {
                let ca = g.resolve()?.and_then(|d| d.get_dict("ca").ok().flatten());
                if g.resolve()?.is_some_and(|d| d.dict_len().unwrap_or(0) == 2)
                    && ca.is_some_and(|c| c.as_float().is_ok_and(|v| (v - o).abs() < 1e-4))
                {
                    found = Some(k);
                    break;
                }
            }
            let gs = match found {
                Some(k) => k,
                None => {
                    let k = free_name("A", &existing);
                    let mut g = doc.new_dict()?;
                    g.dict_put_name("Type", "ExtGState")?;
                    g.dict_put_real("ca", o)?;
                    ext.dict_put(&k, g)?;
                    k
                }
            };
            content.push_str(&format!("/{gs} gs\n"));
        }
        if let Some(oc) = options.optional_content {
            let mut props = sub_dict(doc, &res, "Properties")?;
            let existing: Vec<String> = props.dict_iter()?.into_iter().map(|(k, _)| k).collect();
            let k = free_name("oc", &existing);
            props.dict_put(&k, doc.new_indirect(oc, 0)?)?;
            content.push_str(&format!("/OC /{k} BDC\n"));
        }
        content.push_str(&format!(
            "{} 0 0 {} {} {} cm\n/{img_name} Do\n",
            n(rect.x1 - rect.x0),
            n(rect.y1 - rect.y0),
            n(rect.x0),
            n(rect.y0)
        ));
        if options.optional_content.is_some() {
            content.push_str("EMC\n");
        }
        content.push_str("Q\n");
        let contents_xref = self.insert_contents(doc, content.as_bytes(), options.overlay)?;
        Ok(ImagePlacement {
            name: img_name,
            xref,
            rect,
            contents_xref,
        })
    }

    /// Sustituye la imagen del objeto `xref` por otra, en todos los sitios donde esta página la usa.
    pub fn replace_image(
        &mut self,
        doc: &mut PdfDocument,
        xref: i32,
        source: ImageSource<'_>,
    ) -> Result<ImagePlacement> {
        doc.guard()?;
        if doc.xref_object(xref)?.is_none() {
            return Err(invalid(format!("el objeto {xref} no existe")));
        }
        let users: Vec<String> = self
            .xobject_images()?
            .into_iter()
            .filter(|(_, v)| v.is_indirect().unwrap_or(false) && v.as_indirect().ok() == Some(xref))
            .map(|(k, _)| k)
            .collect();
        if users.is_empty() {
            return Err(invalid(format!("la imagen {xref} no se usa en esta página")));
        }
        let new_xref = doc.image_xref(source)?;
        let res = self.resources()?;
        let mut xo = sub_dict(doc, &res, "XObject")?;
        for key in &users {
            xo.dict_put(key, doc.new_indirect(new_xref, 0)?)?;
        }
        Ok(ImagePlacement {
            name: users[0].clone(),
            xref: new_xref,
            rect: Rect::new(0.0, 0.0, 0.0, 0.0),
            contents_xref: 0,
        })
    }

    /// Quita la imagen `xref` de los recursos de la página. `true` si estaba. El contenido que la dibuja no se toca.
    pub fn delete_image(&mut self, doc: &mut PdfDocument, xref: i32) -> Result<bool> {
        doc.guard()?;
        let users: Vec<String> = self
            .xobject_images()?
            .into_iter()
            .filter(|(_, v)| v.is_indirect().unwrap_or(false) && v.as_indirect().ok() == Some(xref))
            .map(|(k, _)| k)
            .collect();
        if users.is_empty() {
            return Ok(false);
        }
        let res = self.resources()?;
        let mut xo = sub_dict(doc, &res, "XObject")?;
        for key in &users {
            xo.dict_delete(key)?;
        }
        Ok(true)
    }
}

/// Subdiccionario propio de `res` (se crea vacío si falta).
fn sub_dict(doc: &PdfDocument, res: &Object, key: &str) -> Result<Object> {
    if let Some(d) = res.get_dict(key)?
        && d.is_dict()?
    {
        return Ok(d);
    }
    let mut res = res.clone();
    res.dict_put(key, doc.new_dict()?)?;
    res.get_dict(key)?
        .ok_or_else(|| invalid(format!("no se pudo crear /{key}")))
}

/// Primer nombre `<prefijo><n>` (n = 0, 1…) que no está en `taken`.
fn free_name(prefix: &str, taken: &[String]) -> String {
    (0..)
        .map(|i| format!("{prefix}{i}"))
        .find(|c| !taken.contains(c))
        .unwrap_or_else(|| format!("{prefix}0"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::Size;

    /// PNG RGBA de 4×2 con alfa 128 (generado con PIL; 144 ppp).
    fn tiny_rgba_png() -> Vec<u8> {
        let mut out = Vec::new();
        let img = image::RgbaImage::from_pixel(4, 2, image::Rgba([255, 0, 0, 128]));
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
            .unwrap();
        out
    }

    fn tiny_jpeg() -> Vec<u8> {
        let mut out = Vec::new();
        let img = image::RgbImage::from_pixel(8, 6, image::Rgb([0, 128, 255]));
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Jpeg)
            .unwrap();
        out
    }

    #[test]
    fn decodes_png_and_jpeg_and_rejects_the_rest() {
        let png = Image::from_bytes(&tiny_rgba_png()).unwrap();
        assert_eq!(
            (png.width(), png.height(), png.n(), png.bits_per_component()),
            (4, 2, 3, 8)
        );
        assert!(png.mask().is_none());
        let pix = png.to_pixmap().unwrap();
        assert!(pix.alpha());
        assert_eq!(&pix.samples()[..4], &[255, 0, 0, 128]);

        let jpg = Image::from_bytes(&tiny_jpeg()).unwrap();
        assert_eq!((jpg.width(), jpg.height(), jpg.n()), (8, 6, 3));
        assert_eq!(jpg.resolution(), (96, 96));
        assert!(!jpg.to_pixmap().unwrap().samples().is_empty());

        assert!(Image::from_bytes(b"GIF89a....").is_err());
        assert!(Image::from_bytes(b"basura").is_err());
        assert!(Image::from_file("/no/existe.png").is_err());
    }

    #[test]
    fn add_image_embeds_jpeg_untouched_and_png_with_smask() {
        let mut doc = PdfDocument::new();
        let jpeg = tiny_jpeg();
        let o = doc.add_image(&Image::from_bytes(&jpeg).unwrap()).unwrap();
        let d = o.resolve().unwrap().unwrap();
        assert_eq!(d.dict_get_name("Filter").unwrap().as_deref(), Some("DCTDecode"));
        assert_eq!(d.dict_get_name("ColorSpace").unwrap().as_deref(), Some("DeviceRGB"));
        let e = doc.extract_image(o.as_indirect().unwrap()).unwrap();
        assert_eq!((e.width, e.height, e.encoded.as_slice()), (8, 6, jpeg.as_slice()));
        assert_eq!(e.filter.as_deref(), Some("/DCTDecode"));

        let o = doc.add_image(&Image::from_bytes(&tiny_rgba_png()).unwrap()).unwrap();
        let d = o.resolve().unwrap().unwrap();
        assert_eq!(d.dict_get_name("Filter").unwrap().as_deref(), Some("FlateDecode"));
        assert!(d.get_dict("SMask").unwrap().is_some());
        let e = doc.extract_image(o.as_indirect().unwrap()).unwrap();
        assert_eq!(e.filter, None);
        assert_eq!(e.encoded.len(), 4 * 2 * 3);
        assert_eq!(&e.encoded[..3], &[255, 0, 0]);
        let mask = d.get_dict("SMask").unwrap().unwrap();
        assert_eq!(mask.read_stream().unwrap(), vec![128; 8]);
        assert!(doc.extract_image(1).is_err());
    }

    #[test]
    fn from_pixmap_handles_bgr_alpha_and_cmyk() {
        let bgr = Pixmap::from_samples(Colorspace::Bgr, 2, 1, true, vec![10, 20, 30, 255, 1, 2, 3, 0]).unwrap();
        let img = Image::from_pixmap(&bgr).unwrap();
        let back = img.to_pixmap().unwrap();
        assert_eq!(back.colorspace(), Colorspace::Rgb);
        assert_eq!(back.samples(), &[30, 20, 10, 255, 3, 2, 1, 0]);
        let cmyk = Pixmap::from_samples(Colorspace::Cmyk, 1, 1, false, vec![0, 10, 20, 30]).unwrap();
        let mut doc = PdfDocument::new();
        let o = doc.add_image(&Image::from_pixmap(&cmyk).unwrap()).unwrap();
        assert_eq!(
            o.resolve()
                .unwrap()
                .unwrap()
                .dict_get_name("ColorSpace")
                .unwrap()
                .as_deref(),
            Some("DeviceCMYK")
        );
    }

    #[test]
    fn insert_replace_and_delete_images_on_a_page() {
        let mut doc = PdfDocument::new();
        let mut page = doc
            .new_page(Size {
                width: 200.0,
                height: 200.0,
            })
            .unwrap();
        let jpeg = tiny_jpeg();
        let p = page
            .insert_image(
                &mut doc,
                Rect::new(10.0, 20.0, 110.0, 90.0),
                ImageSource::Bytes {
                    data: &jpeg,
                    format_hint: None,
                },
                InsertImageOptions {
                    opacity: Some(0.5),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(p.name, "Im0");
        let stream = doc.new_indirect(p.contents_xref, 0).unwrap().read_stream().unwrap();
        assert_eq!(
            String::from_utf8(stream).unwrap(),
            "q\n/A0 gs\n100 0 0 70 10 20 cm\n/Im0 Do\nQ\n"
        );
        let imgs = page.images().unwrap();
        assert_eq!(imgs.len(), 1);
        assert_eq!(
            (imgs[0].name.as_str(), imgs[0].xref, imgs[0].width, imgs[0].height),
            ("Im0", p.xref, 8, 6)
        );
        assert_eq!(imgs[0].filter.as_deref(), Some("/DCTDecode"));

        // La misma imagen otra vez reutiliza el recurso y la opacidad.
        let p2 = page
            .insert_image(
                &mut doc,
                Rect::new(1.0, 1.5, 2.25, 3.0),
                ImageSource::ExistingXref(p.xref),
                InsertImageOptions {
                    overlay: false,
                    opacity: Some(0.5),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!((p2.name.as_str(), p2.xref), ("Im0", p.xref));
        let s2 = doc.new_indirect(p2.contents_xref, 0).unwrap().read_stream().unwrap();
        assert_eq!(
            String::from_utf8(s2).unwrap(),
            "q\n/A0 gs\n1.25 0 0 1.5 1 1.5 cm\n/Im0 Do\nQ\n"
        );
        let contents = page.contents().unwrap().unwrap();
        assert_eq!(contents.len().unwrap(), 2);
        assert_eq!(
            contents.get_array(0).unwrap().unwrap().as_indirect().unwrap(),
            p2.contents_xref
        );

        assert!(
            page.insert_image(
                &mut doc,
                Rect::new(5.0, 5.0, 5.0, 9.0),
                ImageSource::ExistingXref(p.xref),
                InsertImageOptions::default()
            )
            .is_err()
        );
        assert!(
            page.insert_image(
                &mut doc,
                Rect::new(0.0, 0.0, 5.0, 5.0),
                ImageSource::Bytes {
                    data: b"x",
                    format_hint: None
                },
                InsertImageOptions::default()
            )
            .is_err()
        );

        let png = tiny_rgba_png();
        let r = page
            .replace_image(
                &mut doc,
                p.xref,
                ImageSource::Bytes {
                    data: &png,
                    format_hint: Some("png"),
                },
            )
            .unwrap();
        assert_eq!(r.name, "Im0");
        assert_ne!(r.xref, p.xref);
        let imgs = page.images().unwrap();
        assert_eq!((imgs.len(), imgs[0].xref, imgs[0].width), (1, r.xref, 4));
        assert!(
            page.replace_image(&mut doc, p.xref, ImageSource::ExistingXref(r.xref))
                .is_err()
        );

        assert!(page.delete_image(&mut doc, r.xref).unwrap());
        assert!(page.images().unwrap().is_empty());
        assert!(!page.delete_image(&mut doc, r.xref).unwrap());
    }

    #[test]
    fn optional_content_wraps_the_image() {
        let mut doc = PdfDocument::new();
        let mut page = doc
            .new_page(Size {
                width: 100.0,
                height: 100.0,
            })
            .unwrap();
        let ocg = doc.new_object_from_str("<< /Type /OCG /Name (Marca) >>").unwrap();
        let ocg = doc.add_object(&ocg).unwrap().as_indirect().unwrap();
        let jpeg = tiny_jpeg();
        let p = page
            .insert_image(
                &mut doc,
                Rect::new(0.0, 0.0, 10.0, 10.0),
                ImageSource::Bytes {
                    data: &jpeg,
                    format_hint: None,
                },
                InsertImageOptions {
                    optional_content: Some(ocg),
                    ..Default::default()
                },
            )
            .unwrap();
        let s = doc.new_indirect(p.contents_xref, 0).unwrap().read_stream().unwrap();
        assert_eq!(
            String::from_utf8(s).unwrap(),
            "q\n/OC /oc0 BDC\n10 0 0 10 0 0 cm\n/Im0 Do\nEMC\nQ\n"
        );
        let props = page.resources().unwrap().get_dict("Properties").unwrap().unwrap();
        assert_eq!(props.get_dict("oc0").unwrap().unwrap().as_indirect().unwrap(), ocg);
    }
}
