//! Recorre el contenido de una página (y de sus formularios XObject) y resume lo que usa:
//! colores, trazos, imágenes y fuentes.

use std::collections::BTreeSet;

use pliego_core::Result;
use pliego_pdf::{Object, PdfDocument};

use crate::content::{self, Op};

const MAX_DEPTH: usize = 8;

/// Espacio de color simplificado.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Cs {
    Gray,
    Rgb,
    Cmyk,
    Lab,
    /// Tintas de color directo (Separation o DeviceN). `process` indica tintas CMYK dentro de un DeviceN.
    Spot {
        names: Vec<String>,
    },
    Pattern,
    Other,
}

#[derive(Debug, Clone)]
pub(crate) struct ImageUse {
    pub width: u32,
    pub height: u32,
    /// Resolución efectiva (la menor de los dos ejes) en ppp.
    pub dpi: f32,
    pub cs: Cs,
}

#[derive(Debug, Clone)]
pub(crate) struct FontUse {
    pub name: String,
    pub embedded: bool,
}

/// Cambio propuesto sobre un operador `w` demasiado fino.
#[derive(Debug, Clone)]
pub(crate) struct WidthEdit {
    /// Número de objeto del formulario XObject, o 0 para el contenido de la página.
    pub key: i32,
    /// Posición del operando dentro del flujo.
    pub span: (usize, usize),
    pub new_width: f32,
}

#[derive(Debug, Default)]
pub(crate) struct PageScan {
    /// Grosor efectivo (pt) de cada trazo pintado.
    pub strokes: Vec<f32>,
    pub images: Vec<ImageUse>,
    pub fonts: Vec<FontUse>,
    pub gray: u32,
    pub rgb: u32,
    pub cmyk: u32,
    pub spots: BTreeSet<String>,
    pub edits: Vec<WidthEdit>,
}

type Matrix = [f32; 6];
type Span = (usize, usize);
type Saved = (Matrix, f32, Option<Span>);
const IDENTITY: Matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// `a × b` (primero `a`, luego `b`), como `cm` en PDF.
fn mul(a: Matrix, b: Matrix) -> Matrix {
    [
        a[0] * b[0] + a[1] * b[2],
        a[0] * b[1] + a[1] * b[3],
        a[2] * b[0] + a[3] * b[2],
        a[2] * b[1] + a[3] * b[3],
        a[4] * b[0] + a[5] * b[2] + b[4],
        a[4] * b[1] + a[5] * b[3] + b[5],
    ]
}

fn name_of(obj: &Object) -> Option<String> {
    obj.as_name().ok().map(|b| String::from_utf8_lossy(&b).into_owned())
}

fn get(obj: &Object, key: &str) -> Option<Object> {
    obj.get_dict(key).ok().flatten()
}

fn is_dict_like(o: &Object) -> bool {
    o.is_dict().unwrap_or(false) || o.is_stream().unwrap_or(false)
}

fn number(obj: &Object) -> Option<f32> {
    obj.as_float().ok()
}

/// Interpreta un espacio de color (nombre o matriz).
pub(crate) fn parse_cs(cs: &Object, resources: Option<&Object>, depth: usize) -> Cs {
    if depth > 4 {
        return Cs::Other;
    }
    if cs.is_name().unwrap_or(false) {
        let n = name_of(cs).unwrap_or_default();
        return match n.as_str() {
            "DeviceGray" | "G" | "CalGray" => Cs::Gray,
            "DeviceRGB" | "RGB" | "CalRGB" => Cs::Rgb,
            "DeviceCMYK" | "CMYK" => Cs::Cmyk,
            "Pattern" => Cs::Pattern,
            other => resources
                .and_then(|r| get(r, "ColorSpace"))
                .and_then(|d| get(&d, other))
                .map_or(Cs::Other, |o| parse_cs(&o, resources, depth + 1)),
        };
    }
    if !cs.is_array().unwrap_or(false) {
        return Cs::Other;
    }
    let first = cs
        .get_array(0)
        .ok()
        .flatten()
        .and_then(|f| name_of(&f))
        .unwrap_or_default();
    let arg = |i: i32| cs.get_array(i).ok().flatten();
    match first.as_str() {
        "DeviceGray" | "CalGray" | "G" => Cs::Gray,
        "DeviceRGB" | "CalRGB" | "RGB" => Cs::Rgb,
        "DeviceCMYK" | "CMYK" => Cs::Cmyk,
        "Lab" => Cs::Lab,
        "ICCBased" => match arg(1).and_then(|s| get(&s, "N")).and_then(|n| n.as_int().ok()) {
            Some(1) => Cs::Gray,
            Some(3) => Cs::Rgb,
            Some(4) => Cs::Cmyk,
            _ => Cs::Other,
        },
        "Indexed" | "I" | "Pattern" => match arg(1) {
            Some(base) if first != "Pattern" => parse_cs(&base, resources, depth + 1),
            _ => Cs::Pattern,
        },
        "Separation" => Cs::Spot {
            names: arg(1).and_then(|n| name_of(&n)).into_iter().collect(),
        },
        "DeviceN" => {
            let mut names = Vec::new();
            if let Some(list) = arg(1) {
                for i in 0..list.len().unwrap_or(0) {
                    if let Some(n) = list.get_array(i as i32).ok().flatten().and_then(|n| name_of(&n)) {
                        names.push(n);
                    }
                }
            }
            Cs::Spot { names }
        }
        _ => Cs::Other,
    }
}

struct Scanner {
    out: PageScan,
    /// Si se indica, los trazos más finos que esto generan un [`WidthEdit`].
    min_width: Option<f32>,
    stack: Vec<i32>,
    seen_fonts: BTreeSet<String>,
}

pub(crate) fn scan_page(doc: &PdfDocument, page_index: usize) -> Result<PageScan> {
    scan_page_with(doc, page_index, None)
}

/// Une los flujos de contenido de una página tal como los lee el escáner.
pub(crate) fn page_content(doc: &PdfDocument, page_index: usize) -> Result<Vec<u8>> {
    let page = doc.find_page(page_index as i32)?;
    Ok(get(&page, "Contents").map(|c| stream_bytes(&c)).unwrap_or_default())
}

pub(crate) fn scan_page_with(doc: &PdfDocument, page_index: usize, min_width: Option<f32>) -> Result<PageScan> {
    let page = doc.find_page(page_index as i32)?;
    let resources = page.get_dict_inheritable("Resources")?;
    let data = match get(&page, "Contents") {
        Some(c) => stream_bytes(&c),
        None => Vec::new(),
    };
    let mut s = Scanner {
        out: PageScan::default(),
        min_width,
        stack: Vec::new(),
        seen_fonts: BTreeSet::new(),
    };
    s.run(&data, resources.as_ref(), IDENTITY, 0, 0);
    Ok(s.out)
}

/// Une los flujos de contenido (uno o una matriz de ellos).
fn stream_bytes(obj: &Object) -> Vec<u8> {
    if obj.is_array().unwrap_or(false) {
        let mut all = Vec::new();
        for i in 0..obj.len().unwrap_or(0) {
            if let Some(part) = obj.get_array(i as i32).ok().flatten() {
                all.extend(part.read_stream().unwrap_or_default());
                all.push(b'\n');
            }
        }
        all
    } else {
        obj.read_stream().unwrap_or_default()
    }
}

impl Scanner {
    fn color(&mut self, cs: &Cs) {
        match cs {
            Cs::Gray => self.out.gray += 1,
            Cs::Rgb | Cs::Lab => self.out.rgb += 1,
            Cs::Cmyk => self.out.cmyk += 1,
            Cs::Spot { names } => self.out.spots.extend(names.iter().cloned()),
            Cs::Pattern | Cs::Other => {}
        }
    }

    fn run(&mut self, data: &[u8], resources: Option<&Object>, base: Matrix, depth: usize, key: i32) {
        let ops = content::parse(data);
        let mut ctm = base;
        let mut gstack: Vec<Saved> = Vec::new();
        let mut width = 1.0f32;
        let mut width_src: Option<(usize, usize)> = None;
        for op in &ops {
            match op.name.as_str() {
                "q" => gstack.push((ctm, width, width_src)),
                "Q" => {
                    if let Some((m, w, src)) = gstack.pop() {
                        ctm = m;
                        width = w;
                        width_src = src;
                    }
                }
                "cm" => {
                    if let [a, b, c, d, e, f] = op.nums()[..] {
                        ctm = mul([a, b, c, d, e, f], ctm);
                    }
                }
                "w" => {
                    width = op.num(0).unwrap_or(width);
                    width_src = op.spans.first().copied();
                }
                "g" | "G" => self.color(&Cs::Gray),
                "rg" | "RG" => self.color(&Cs::Rgb),
                "k" | "K" => self.color(&Cs::Cmyk),
                "cs" | "CS" => {
                    if let Some(n) = op.name_arg(0) {
                        let cs = match n {
                            "DeviceGray" | "DeviceRGB" | "DeviceCMYK" | "Pattern" => None,
                            other => resources
                                .and_then(|r| get(r, "ColorSpace"))
                                .and_then(|d| get(&d, other))
                                .map(|o| parse_cs(&o, resources, 0)),
                        };
                        // Los espacios de dispositivo se cuentan al fijar el color (g, rg, k); con `cs` solo
                        // importan los nombrados porque `scn` puede no traer operandos reconocibles.
                        if let Some(cs) = cs {
                            self.color(&cs);
                        }
                    }
                }
                "S" | "s" | "B" | "B*" | "b" | "b*" => {
                    let scale = (ctm[0] * ctm[3] - ctm[1] * ctm[2]).abs().sqrt();
                    let effective = width * scale;
                    self.out.strokes.push(effective);
                    if let (Some(min), Some(span)) = (self.min_width, width_src)
                        && effective < min
                        && scale > 0.0
                    {
                        self.out.edits.push(WidthEdit {
                            key,
                            span,
                            new_width: min / scale,
                        });
                    }
                }
                "Tf" => self.font(op, resources),
                "Do" => self.xobject(op, resources, ctm, depth),
                _ => {}
            }
        }
    }

    fn font(&mut self, op: &Op, resources: Option<&Object>) {
        let Some(key) = op.name_arg(0) else { return };
        let Some(font) = resources.and_then(|r| get(r, "Font")).and_then(|d| get(&d, key)) else {
            return;
        };
        let name = get(&font, "BaseFont")
            .and_then(|n| name_of(&n))
            .unwrap_or_else(|| key.to_string());
        if !self.seen_fonts.insert(name.clone()) {
            return;
        }
        self.out.fonts.push(FontUse {
            embedded: font_is_embedded(&font),
            name,
        });
    }

    fn xobject(&mut self, op: &Op, resources: Option<&Object>, ctm: Matrix, depth: usize) {
        let Some(key) = op.name_arg(0) else { return };
        let Some(xobj) = resources.and_then(|r| get(r, "XObject")).and_then(|d| get(&d, key)) else {
            return;
        };
        match get(&xobj, "Subtype").and_then(|s| name_of(&s)).as_deref() {
            Some("Image") => {
                let w = get(&xobj, "Width").and_then(|n| number(&n)).unwrap_or(0.0);
                let h = get(&xobj, "Height").and_then(|n| number(&n)).unwrap_or(0.0);
                let cs = get(&xobj, "ColorSpace")
                    .map(|c| parse_cs(&c, resources, 0))
                    .unwrap_or(Cs::Gray);
                let is_mask = get(&xobj, "ImageMask").and_then(|m| m.as_bool().ok()).unwrap_or(false);
                let size_x = ctm[0].hypot(ctm[1]) / 72.0;
                let size_y = ctm[2].hypot(ctm[3]) / 72.0;
                if size_x > 0.0 && size_y > 0.0 && w > 0.0 && !is_mask {
                    self.color(&cs);
                    self.out.images.push(ImageUse {
                        width: w as u32,
                        height: h as u32,
                        dpi: (w / size_x).min(h / size_y),
                        cs,
                    });
                }
            }
            Some("Form") if depth < MAX_DEPTH => {
                let id = xobj.as_indirect().unwrap_or(0);
                if id != 0 && self.stack.contains(&id) {
                    return;
                }
                self.stack.push(id);
                let m = get(&xobj, "Matrix")
                    .map(|a| {
                        let mut v = [0.0; 6];
                        for (i, slot) in v.iter_mut().enumerate() {
                            *slot = a
                                .get_array(i as i32)
                                .ok()
                                .flatten()
                                .and_then(|n| number(&n))
                                .unwrap_or(0.0);
                        }
                        v
                    })
                    .unwrap_or(IDENTITY);
                let own = get(&xobj, "Resources").filter(is_dict_like);
                let data = xobj.read_stream().unwrap_or_default();
                self.run(&data, own.as_ref().or(resources), mul(m, ctm), depth + 1, id);
                self.stack.pop();
            }
            _ => {}
        }
    }
}

fn font_is_embedded(font: &Object) -> bool {
    let subtype = get(font, "Subtype").and_then(|s| name_of(&s)).unwrap_or_default();
    if subtype == "Type3" {
        return true;
    }
    let target = if subtype == "Type0" {
        match get(font, "DescendantFonts").and_then(|d| d.get_array(0).ok().flatten()) {
            Some(d) => d,
            None => return false,
        }
    } else {
        font.try_clone().unwrap_or_else(|_| Object::new_null())
    };
    get(&target, "FontDescriptor").is_some_and(|d| {
        ["FontFile", "FontFile2", "FontFile3"]
            .iter()
            .any(|k| get(&d, k).is_some())
    })
}
