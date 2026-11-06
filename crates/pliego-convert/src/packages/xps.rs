//! XPS (documento fijo de Windows): un paquete OPC con páginas en XML (`.fpage`) en unidades de 1/96 de pulgada.
//!
//! # Qué se convierte
//! - `Path` (geometría abreviada `Data="M 0,0 L …"`, `PathGeometry` con `Figures` o figuras con segmentos), con relleno
//!   de color liso (`#RRGGBB`, `#AARRGGBB`, `sc#…`), de imagen (`ImageBrush`: JPEG, PNG, TIFF, GIF, BMP, WebP) y trazo
//!   (grosor, guiones, remates y uniones).
//! - `Glyphs`: el texto con **su fuente incrustada** (TrueType, también las ofuscadas `.odttf`), glifo a glifo con los
//!   avances de `Indices`; el texto se puede seleccionar y buscar.
//! - `Canvas` anidados, `RenderTransform`, `Clip`, `Opacity` y `ResourceDictionary` (`{StaticResource clave}`).
//!
//! # Qué no
//! Degradados (se aproximan con el color medio de sus paradas), `VisualBrush`, `OpacityMask`, imágenes HD Photo
//! (`.wdp`/`.jxr`), fuentes CFF y colecciones `.ttc`, firmas y permisos del paquete. Lo que se omite se cuenta en
//! el informe.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::path::Path;

use pliego_core::{Error, JobContext, Report, Result};
use pliego_pdf::Size;
use pliego_pdf::native::{Font, Object, PdfDocument, Raster};
use roxmltree::{Document as Xml, Node};

use super::zipio::{Package, resolve};
use crate::util::{ensure_not_same, write_atomic};

const MAX_XML_BYTES: u64 = 64 << 20;
const MAX_PART_BYTES: u64 = 128 << 20;
const MAX_PAGES: usize = 20_000;
const MAX_DEPTH: usize = 64;
/// 1 unidad de XPS (1/96 pulgada) en puntos.
const PX: f32 = 0.75;
const XAML_NS: &str = "http://schemas.microsoft.com/winfx/2006/xaml";

/// Convierte un XPS en un PDF con una página por página fija (los documentos del paquete se concatenan en orden).
pub fn xps_to_pdf(input: &Path, output: &Path, ctx: &JobContext) -> Result<Report> {
    ensure_not_same(input, output)?;
    let mut package = Package::open(input)?;
    let pages = page_list(&mut package, input)?;
    if pages.len() > MAX_PAGES {
        return Err(Error::InvalidInput(format!(
            "{}: demasiadas páginas ({})",
            input.display(),
            pages.len()
        )));
    }
    let mut pdf = PdfDocument::new();
    let mut state = Shared::default();
    for (n, page_path) in pages.iter().enumerate() {
        ctx.step(n as u64, pages.len() as u64, &format!("página {}", n + 1))?;
        let bytes = package.require(page_path, MAX_XML_BYTES)?;
        let text = String::from_utf8_lossy(&bytes).into_owned();
        let xml = Xml::parse_with_options(
            &text,
            roxmltree::ParsingOptions {
                allow_dtd: true,
                ..Default::default()
            },
        )
        .map_err(|e| Error::InvalidInput(format!("{}: «{page_path}» no es XML válido ({e})", input.display())))?;
        render_page(&mut pdf, &mut package, &mut state, &xml, page_path)?;
    }
    let bytes = crate::typeset::save_bytes(&pdf)?;
    write_atomic(output, &bytes)?;
    ctx.step(pages.len() as u64, pages.len() as u64, "listo")?;
    let mut report = Report::with_output(output, format!("{} página(s) creadas", pages.len()));
    for (what, count) in &state.skipped {
        report
            .warnings
            .push(format!("{what}: {count} vez/veces (omitido o aproximado)"));
    }
    Ok(report)
}

/// Rutas de las páginas, en orden: `FixedDocumentSequence` → `FixedDocument` → `PageContent`.
fn page_list(package: &mut Package, input: &Path) -> Result<Vec<String>> {
    let mut sequence: Option<String> = None;
    if let Some(rels) = package.read("_rels/.rels", MAX_XML_BYTES)? {
        let rels = String::from_utf8_lossy(&rels).into_owned();
        if let Ok(xml) = Xml::parse(&rels) {
            sequence = xml
                .descendants()
                .filter(|n| n.tag_name().name() == "Relationship")
                .find(|n| {
                    n.attribute("Type")
                        .is_some_and(|t| t.to_lowercase().ends_with("/fixedrepresentation"))
                })
                .and_then(|n| n.attribute("Target"))
                .and_then(|t| resolve("_rels/.rels", t));
        }
    }
    let sequence = sequence.or_else(|| {
        package
            .names()
            .iter()
            .find(|n| n.to_lowercase().ends_with(".fdseq"))
            .cloned()
    });
    let mut pages = Vec::new();
    if let Some(seq) = sequence {
        let bytes = package.require(&seq, MAX_XML_BYTES)?;
        let text = String::from_utf8_lossy(&bytes).into_owned();
        let xml = Xml::parse(&text).map_err(|_| bad(input, "la secuencia de documentos"))?;
        let docs: Vec<String> = xml
            .descendants()
            .filter(|n| n.tag_name().name() == "DocumentReference")
            .filter_map(|n| n.attribute("Source"))
            .filter_map(|s| resolve(&seq, s))
            .collect();
        for doc in docs {
            let Some(bytes) = package.read(&doc, MAX_XML_BYTES)? else {
                continue;
            };
            let text = String::from_utf8_lossy(&bytes).into_owned();
            let xml = Xml::parse(&text).map_err(|_| bad(input, "un documento"))?;
            pages.extend(
                xml.descendants()
                    .filter(|n| n.tag_name().name() == "PageContent")
                    .filter_map(|n| n.attribute("Source"))
                    .filter_map(|s| resolve(&doc, s)),
            );
        }
    }
    if pages.is_empty() {
        // Sin índice legible: todas las páginas por nombre.
        let mut all: Vec<String> = package
            .names()
            .iter()
            .filter(|n| n.to_lowercase().ends_with(".fpage"))
            .cloned()
            .collect();
        all.sort_by_key(|n| natural_key(n));
        pages = all;
    }
    if pages.is_empty() {
        return Err(Error::InvalidInput(format!(
            "{}: el XPS no contiene páginas",
            input.display()
        )));
    }
    Ok(pages)
}

fn bad(input: &Path, what: &str) -> Error {
    Error::InvalidInput(format!("{}: XPS no válido ({what})", input.display()))
}

/// Clave de orden que compara los números por valor (`2.fpage` antes de `10.fpage`).
fn natural_key(name: &str) -> Vec<(u8, String, u64)> {
    let mut out = Vec::new();
    let mut digits = String::new();
    let mut text = String::new();
    let flush_text = |text: &mut String, out: &mut Vec<(u8, String, u64)>| {
        if !text.is_empty() {
            out.push((0, std::mem::take(text).to_lowercase(), 0));
        }
    };
    for c in name.chars() {
        if c.is_ascii_digit() {
            flush_text(&mut text, &mut out);
            digits.push(c);
        } else {
            if !digits.is_empty() {
                out.push((1, String::new(), digits.parse().unwrap_or(0)));
                digits.clear();
            }
            text.push(c);
        }
    }
    flush_text(&mut text, &mut out);
    if !digits.is_empty() {
        out.push((1, String::new(), digits.parse().unwrap_or(0)));
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------------
// Estado compartido por todas las páginas

struct LoadedFont {
    resource: Object,
    font: Font,
}

#[derive(Default)]
struct Shared {
    /// Fuentes por ruta de la parte (o `?` para la de respaldo): se incrustan una sola vez.
    fonts: HashMap<String, std::rc::Rc<LoadedFont>>,
    /// Imágenes por ruta de la parte; `None` si no se pudo leer.
    images: HashMap<String, Option<(Object, u32, u32)>>,
    /// Lo omitido o aproximado, contado por tipo.
    skipped: BTreeMap<&'static str, usize>,
}

impl Shared {
    fn skip(&mut self, what: &'static str) {
        *self.skipped.entry(what).or_default() += 1;
    }
}

/// Recursos de la página en curso.
#[derive(Default)]
struct PageRes {
    fonts: BTreeMap<String, Object>,
    images: BTreeMap<String, Object>,
    gstates: BTreeMap<String, (f32, f32)>,
}

struct Renderer<'a> {
    pdf: &'a mut PdfDocument,
    package: &'a mut Package,
    shared: &'a mut Shared,
    page_path: &'a str,
    res: PageRes,
    content: String,
    /// Recursos `x:Key` de la página (se buscan por nombre, sin distinguir el ámbito).
    keys: HashMap<String, Node<'a, 'a>>,
}

fn render_page(
    pdf: &mut PdfDocument,
    package: &mut Package,
    shared: &mut Shared,
    xml: &Xml<'_>,
    page_path: &str,
) -> Result<()> {
    let root = xml.root_element();
    let width = root
        .attribute("Width")
        .and_then(|v| v.trim().parse::<f32>().ok())
        .unwrap_or(816.0);
    let height = root
        .attribute("Height")
        .and_then(|v| v.trim().parse::<f32>().ok())
        .unwrap_or(1056.0);
    if !(width.is_finite()
        && height.is_finite()
        && width > 0.0
        && height > 0.0
        && width < 20_000.0
        && height < 20_000.0)
    {
        return Err(Error::InvalidInput(format!(
            "«{page_path}»: tamaño de página no válido"
        )));
    }
    let mut keys = HashMap::new();
    for n in root.descendants().filter(Node::is_element) {
        if let Some(k) = n.attribute((XAML_NS, "Key")) {
            keys.insert(k.to_string(), n);
        }
    }
    let mut r = Renderer {
        pdf,
        package,
        shared,
        page_path,
        res: PageRes::default(),
        content: String::new(),
        keys,
    };
    let _ = writeln!(r.content, "q {} 0 0 {} 0 {} cm", num(PX), num(-PX), num(height * PX));
    for child in root.children().filter(Node::is_element) {
        r.element(child, 0);
    }
    r.content.push_str("Q\n");

    let Renderer { content, res, .. } = r;
    let mut page = pdf.new_page(Size::new(width * PX, height * PX))?;
    page.insert_contents(pdf, content.as_bytes(), true)?;
    let mut resources = page.resources()?;
    if !res.fonts.is_empty() {
        let mut d = pdf.new_dict()?;
        for (k, v) in res.fonts {
            d.dict_put(&k, v)?;
        }
        resources.dict_put("Font", d)?;
    }
    if !res.images.is_empty() {
        let mut d = pdf.new_dict()?;
        for (k, v) in res.images {
            d.dict_put(&k, v)?;
        }
        resources.dict_put("XObject", d)?;
    }
    if !res.gstates.is_empty() {
        let mut d = pdf.new_dict()?;
        for (k, (fill, stroke)) in res.gstates {
            let mut g = pdf.new_dict()?;
            g.dict_put_name("Type", "ExtGState")?;
            g.dict_put_real("ca", fill)?;
            g.dict_put_real("CA", stroke)?;
            d.dict_put(&k, g)?;
        }
        resources.dict_put("ExtGState", d)?;
    }
    Ok(())
}

fn num(v: f32) -> String {
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-0" {
        "0".into()
    } else {
        s.into()
    }
}

fn floats(s: &str) -> Vec<f32> {
    s.split([',', ' ', '\t', '\r', '\n'])
        .filter(|p| !p.is_empty())
        .filter_map(|p| p.parse::<f32>().ok())
        .collect()
}

// ---------------------------------------------------------------------------------------------------------------------
// Color y pinceles

#[derive(Clone, Copy, Debug, PartialEq)]
struct Rgba {
    r: f32,
    g: f32,
    b: f32,
    a: f32,
}

fn parse_color(s: &str) -> Option<Rgba> {
    let s = s.trim();
    let hex = |h: &str| u8::from_str_radix(h, 16).ok().map(|v| f32::from(v) / 255.0);
    if let Some(h) = s.strip_prefix('#') {
        return match h.len() {
            6 => Some(Rgba {
                r: hex(&h[0..2])?,
                g: hex(&h[2..4])?,
                b: hex(&h[4..6])?,
                a: 1.0,
            }),
            8 => Some(Rgba {
                a: hex(&h[0..2])?,
                r: hex(&h[2..4])?,
                g: hex(&h[4..6])?,
                b: hex(&h[6..8])?,
            }),
            _ => None,
        };
    }
    if let Some(rest) = s.strip_prefix("sc#") {
        let v = floats(rest);
        let c = |x: f32| x.clamp(0.0, 1.0);
        return match v.len() {
            3 => Some(Rgba {
                r: c(v[0]),
                g: c(v[1]),
                b: c(v[2]),
                a: 1.0,
            }),
            4 => Some(Rgba {
                a: c(v[0]),
                r: c(v[1]),
                g: c(v[2]),
                b: c(v[3]),
            }),
            _ => None,
        };
    }
    // `ContextColor /ruta/perfil.icc 0.1,0.2,0.3,0.4`: los componentes con el perfil se tratan como RGB/CMYK simples.
    if let Some(rest) = s.strip_prefix("ContextColor") {
        let comps: Vec<f32> = rest.split_whitespace().skip(1).flat_map(floats).collect();
        let c = |x: f32| x.clamp(0.0, 1.0);
        return match comps.len() {
            4 => Some(Rgba {
                r: (1.0 - c(comps[0])) * (1.0 - c(comps[3])),
                g: (1.0 - c(comps[1])) * (1.0 - c(comps[3])),
                b: (1.0 - c(comps[2])) * (1.0 - c(comps[3])),
                a: 1.0,
            }),
            3 => Some(Rgba {
                r: c(comps[0]),
                g: c(comps[1]),
                b: c(comps[2]),
                a: 1.0,
            }),
            1 => Some(Rgba {
                r: c(comps[0]),
                g: c(comps[0]),
                b: c(comps[0]),
                a: 1.0,
            }),
            _ => None,
        };
    }
    None
}

enum Brush {
    Solid(Rgba),
    Image {
        source: String,
        viewport: Option<[f32; 4]>,
    },
    /// Degradado aproximado con un color.
    Approx(Rgba),
}

// ---------------------------------------------------------------------------------------------------------------------
// Elementos

impl<'a> Renderer<'a> {
    fn element(&mut self, node: Node<'a, 'a>, depth: usize) {
        if depth > MAX_DEPTH {
            return;
        }
        match node.tag_name().name() {
            "Canvas" => {
                self.content.push_str("q\n");
                self.common(node);
                for child in node.children().filter(Node::is_element) {
                    if !child.tag_name().name().contains('.') {
                        self.element(child, depth + 1);
                    }
                }
                self.content.push_str("Q\n");
            }
            "Path" => self.path(node),
            "Glyphs" => self.glyphs(node),
            _ => {}
        }
    }

    /// Valor de una propiedad dada como atributo o como elemento hijo `Tipo.Propiedad`.
    fn prop_node(&self, node: Node<'a, 'a>, name: &str) -> Option<Node<'a, 'a>> {
        let wanted = format!("{}.{name}", node.tag_name().name());
        node.children()
            .find(|c| c.is_element() && c.tag_name().name() == wanted)
            .and_then(|c| c.children().find(Node::is_element))
    }

    /// Atributo con resolución de `{StaticResource clave}`.
    fn attr(&self, node: Node<'a, 'a>, name: &str) -> Option<&'a str> {
        node.attribute(name)
    }

    fn resource_for(&self, value: &str) -> Option<Node<'a, 'a>> {
        let v = value.trim();
        let key = v
            .strip_prefix("{StaticResource")
            .or_else(|| v.strip_prefix("{DynamicResource"))?
            .trim_end_matches('}')
            .trim();
        self.keys.get(key).copied()
    }

    /// `q` ya está abierto: aplica transformación, recorte y opacidad de un elemento.
    fn common(&mut self, node: Node<'a, 'a>) {
        let matrix = self.attr(node, "RenderTransform").map(str::to_string).or_else(|| {
            self.prop_node(node, "RenderTransform")
                .and_then(|m| m.attribute("Matrix").map(str::to_string))
        });
        if let Some(m) = matrix.map(|m| floats(&m)).filter(|m| m.len() == 6) {
            let _ = writeln!(
                self.content,
                "{} {} {} {} {} {} cm",
                num(m[0]),
                num(m[1]),
                num(m[2]),
                num(m[3]),
                num(m[4]),
                num(m[5])
            );
        }
        let clip = self
            .attr(node, "Clip")
            .and_then(|c| self.geometry_text(c))
            .or_else(|| self.prop_node(node, "Clip").and_then(|g| self.geometry_node(g)));
        if let Some((ops, even_odd)) = clip {
            let _ = writeln!(self.content, "{ops}{} n", if even_odd { "W*" } else { "W" });
        }
        if let Some(op) = node.attribute("Opacity").and_then(|o| o.trim().parse::<f32>().ok())
            && op < 1.0
        {
            let name = self.gstate(op.clamp(0.0, 1.0), op.clamp(0.0, 1.0));
            let _ = writeln!(self.content, "/{name} gs");
        }
        if node.attribute("OpacityMask").is_some() || self.prop_node(node, "OpacityMask").is_some() {
            self.shared.skip("máscaras de opacidad (OpacityMask)");
        }
    }

    fn gstate(&mut self, fill: f32, stroke: f32) -> String {
        let key = (fill, stroke);
        if let Some((name, _)) = self.res.gstates.iter().find(|(_, v)| **v == key) {
            return name.clone();
        }
        let name = format!("GS{}", self.res.gstates.len());
        self.res.gstates.insert(name.clone(), key);
        name
    }

    // ---- geometría ----

    /// Operadores de trazado PDF de un texto de geometría abreviada (`Data="…"`).
    fn geometry_text(&self, data: &str) -> Option<(String, bool)> {
        if let Some(res) = self.resource_for(data) {
            return self.geometry_node(res);
        }
        parse_path_data(data)
    }

    fn geometry_node(&self, node: Node<'a, 'a>) -> Option<(String, bool)> {
        match node.tag_name().name() {
            "PathGeometry" => {
                let mut even_odd = node
                    .attribute("FillRule")
                    .is_some_and(|f| f.eq_ignore_ascii_case("EvenOdd"));
                let mut ops = String::new();
                if let Some(f) = node.attribute("Figures") {
                    let (o, eo) = self.geometry_text(f)?;
                    ops.push_str(&o);
                    even_odd |= eo;
                }
                for fig in node
                    .children()
                    .filter(|c| c.is_element() && c.tag_name().name() == "PathFigure")
                {
                    ops.push_str(&figure_ops(fig));
                }
                (!ops.is_empty()).then_some((ops, even_odd))
            }
            "StreamGeometry" => parse_path_data(node.text().unwrap_or("")),
            _ => None,
        }
    }

    // ---- pinceles ----

    fn brush(&mut self, node: Node<'a, 'a>, prop: &str) -> Option<Brush> {
        if let Some(v) = node.attribute(prop) {
            if let Some(res) = self.resource_for(v) {
                return self.brush_node(res);
            }
            return parse_color(v).map(Brush::Solid);
        }
        let child = self.prop_node(node, prop)?;
        self.brush_node(child)
    }

    fn brush_node(&mut self, n: Node<'a, 'a>) -> Option<Brush> {
        match n.tag_name().name() {
            "SolidColorBrush" => {
                let mut c = parse_color(n.attribute("Color")?)?;
                if let Some(o) = n.attribute("Opacity").and_then(|o| o.trim().parse::<f32>().ok()) {
                    c.a *= o.clamp(0.0, 1.0);
                }
                Some(Brush::Solid(c))
            }
            "ImageBrush" => {
                let source = n.attribute("ImageSource")?;
                // `{ColorConvertedBitmap /ruta.tif /perfil.icc}`: la imagen es el primer argumento.
                let source = source
                    .trim()
                    .strip_prefix("{ColorConvertedBitmap")
                    .map_or(source.trim(), |s| s.split_whitespace().next().unwrap_or(""));
                let viewport = n
                    .attribute("Viewport")
                    .map(floats)
                    .filter(|v| v.len() == 4)
                    .map(|v| [v[0], v[1], v[2], v[3]]);
                Some(Brush::Image {
                    source: resolve(self.page_path, source)?,
                    viewport,
                })
            }
            "LinearGradientBrush" | "RadialGradientBrush" => {
                let stops: Vec<Rgba> = n
                    .descendants()
                    .filter(|c| c.is_element() && c.tag_name().name() == "GradientStop")
                    .filter_map(|s| s.attribute("Color").and_then(parse_color))
                    .collect();
                if stops.is_empty() {
                    return None;
                }
                let k = stops.len() as f32;
                self.shared.skip("degradados (aproximados con su color medio)");
                Some(Brush::Approx(Rgba {
                    r: stops.iter().map(|c| c.r).sum::<f32>() / k,
                    g: stops.iter().map(|c| c.g).sum::<f32>() / k,
                    b: stops.iter().map(|c| c.b).sum::<f32>() / k,
                    a: stops.iter().map(|c| c.a).sum::<f32>() / k,
                }))
            }
            "VisualBrush" => {
                self.shared.skip("pinceles de visual (VisualBrush)");
                None
            }
            _ => None,
        }
    }

    // ---- Path ----

    fn path(&mut self, node: Node<'a, 'a>) {
        let geometry = node
            .attribute("Data")
            .and_then(|d| self.geometry_text(d))
            .or_else(|| self.prop_node(node, "Data").and_then(|g| self.geometry_node(g)));
        let Some((ops, even_odd)) = geometry else { return };
        let fill = self.brush(node, "Fill");
        let stroke = self.brush(node, "Stroke");
        if fill.is_none() && stroke.is_none() {
            return;
        }
        self.content.push_str("q\n");
        self.common(node);
        match &fill {
            Some(Brush::Solid(c) | Brush::Approx(c)) => self.set_color(*c, true),
            Some(Brush::Image { source, viewport }) => {
                self.image_fill(&ops, even_odd, source, viewport.as_ref());
            }
            None => {}
        }
        let stroke_color = match &stroke {
            Some(Brush::Solid(c) | Brush::Approx(c)) => Some(*c),
            _ => None,
        };
        if let Some(c) = stroke_color {
            self.set_color(c, false);
            self.stroke_style(node);
        }
        let solid_fill = matches!(fill, Some(Brush::Solid(_) | Brush::Approx(_)));
        let paint = match (solid_fill, stroke_color.is_some(), even_odd) {
            (true, true, false) => "B",
            (true, true, true) => "B*",
            (true, false, false) => "f",
            (true, false, true) => "f*",
            (false, true, _) => "S",
            (false, false, _) => "n",
        };
        if paint != "n" {
            let _ = writeln!(self.content, "{ops}{paint}");
        }
        self.content.push_str("Q\n");
    }

    fn set_color(&mut self, c: Rgba, fill: bool) {
        let _ = writeln!(
            self.content,
            "{} {} {} {}",
            num(c.r),
            num(c.g),
            num(c.b),
            if fill { "rg" } else { "RG" }
        );
        if c.a < 0.999 {
            let a = c.a.clamp(0.0, 1.0);
            let name = if fill { self.gstate(a, 1.0) } else { self.gstate(1.0, a) };
            let _ = writeln!(self.content, "/{name} gs");
        }
    }

    fn stroke_style(&mut self, node: Node<'a, 'a>) {
        let w = node
            .attribute("StrokeThickness")
            .and_then(|v| v.trim().parse::<f32>().ok())
            .unwrap_or(1.0)
            .max(0.0);
        let _ = writeln!(self.content, "{} w", num(w));
        let cap = node
            .attribute("StrokeStartLineCap")
            .or_else(|| node.attribute("StrokeEndLineCap"))
            .or_else(|| node.attribute("StrokeDashCap"));
        let cap = match cap {
            Some("Round") => 1,
            Some("Square") => 2,
            _ => 0,
        };
        let _ = writeln!(self.content, "{cap} J");
        let join = match node.attribute("StrokeLineJoin") {
            Some("Round") => 1,
            Some("Bevel") => 2,
            _ => 0,
        };
        let _ = writeln!(self.content, "{join} j");
        if let Some(m) = node
            .attribute("StrokeMiterLimit")
            .and_then(|v| v.trim().parse::<f32>().ok())
        {
            let _ = writeln!(self.content, "{} M", num(m.max(1.0)));
        }
        if let Some(d) = node.attribute("StrokeDashArray") {
            let mut dash = floats(d);
            if dash.len() % 2 == 1 {
                let copy = dash.clone();
                dash.extend(copy);
            }
            if !dash.is_empty() && dash.iter().all(|v| *v >= 0.0) && dash.iter().sum::<f32>() > 0.0 {
                let off = node
                    .attribute("StrokeDashOffset")
                    .and_then(|v| v.trim().parse::<f32>().ok())
                    .unwrap_or(0.0);
                // Los guiones de XPS se miden en grosores de trazo.
                let parts: Vec<String> = dash.iter().map(|v| num(v * w.max(0.0001))).collect();
                let _ = writeln!(self.content, "[{}] {} d", parts.join(" "), num(off * w));
            }
        }
    }

    /// Rellena el trazado con una imagen: recorta al trazado y dibuja la imagen en el `Viewport`. `true` si lo hizo.
    fn image_fill(&mut self, ops: &str, even_odd: bool, source: &str, viewport: Option<&[f32; 4]>) -> bool {
        let Some(name) = self.image_resource(source) else {
            return false;
        };
        let Some(vp) = viewport else { return false };
        let _ = writeln!(self.content, "q\n{ops}{} n", if even_odd { "W*" } else { "W" });
        // La imagen ocupa el cuadrado unidad con y hacia arriba; con la página en y hacia abajo hay que volcarla.
        let _ = writeln!(
            self.content,
            "{} 0 0 {} {} {} cm\n/{name} Do\nQ",
            num(vp[2]),
            num(-vp[3]),
            num(vp[0]),
            num(vp[1] + vp[3])
        );
        true
    }

    fn image_resource(&mut self, path: &str) -> Option<String> {
        let cached = if let Some(c) = self.shared.images.get(path) {
            c.as_ref().map(|(o, _, _)| o.clone())
        } else {
            let loaded = self
                .package
                .read(path, MAX_PART_BYTES)
                .ok()
                .flatten()
                .and_then(|b| Raster::decode(&b).ok())
                .and_then(|mut frames| (!frames.is_empty()).then(|| frames.swap_remove(0)))
                .and_then(|r| self.pdf.add_raster(&r).ok().map(|o| (o, r.width(), r.height())));
            let obj = loaded.as_ref().map(|(o, _, _)| o.clone());
            if loaded.is_none() {
                self.shared
                    .skip("imágenes que no se pudieron leer (HD Photo u otro formato)");
            }
            self.shared.images.insert(path.to_string(), loaded);
            obj
        }?;
        if let Some((k, _)) = self.res.images.iter().find(|(_, o)| same_object(o, &cached)) {
            return Some(k.clone());
        }
        let name = format!("Im{}", self.res.images.len());
        self.res.images.insert(name.clone(), cached);
        Some(name)
    }

    // ---- Glyphs ----

    fn glyphs(&mut self, node: Node<'a, 'a>) {
        let em = node
            .attribute("FontRenderingEmSize")
            .and_then(|v| v.trim().parse::<f32>().ok())
            .filter(|v| v.is_finite() && *v > 0.0)
            .unwrap_or(0.0);
        let text = node.attribute("UnicodeString").unwrap_or("");
        let indices = node.attribute("Indices").unwrap_or("");
        if em == 0.0 || (text.is_empty() && indices.is_empty()) {
            return;
        }
        let fill = self.brush(node, "Fill");
        let color = match fill {
            Some(Brush::Solid(c) | Brush::Approx(c)) => c,
            Some(Brush::Image { .. }) | None => Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
        };
        let Some(font_uri) = node.attribute("FontUri") else {
            return;
        };
        let font_path = resolve(self.page_path, font_uri);
        let Some((font_name, loaded)) = self.font(font_path.as_deref()) else {
            return;
        };
        let origin_x = node
            .attribute("OriginX")
            .and_then(|v| v.trim().parse::<f32>().ok())
            .unwrap_or(0.0);
        let origin_y = node
            .attribute("OriginY")
            .and_then(|v| v.trim().parse::<f32>().ok())
            .unwrap_or(0.0);
        let rtl = node
            .attribute("BidiLevel")
            .and_then(|v| v.trim().parse::<u32>().ok())
            .is_some_and(|l| l % 2 == 1);
        let runs = glyph_runs(text, indices, &loaded.font);

        self.content.push_str("q\n");
        self.common(node);
        self.set_color(color, true);
        self.content.push_str("BT\n");
        let _ = writeln!(self.content, "/{font_name} 1 Tf");
        let mut x = origin_x;
        for g in runs {
            let advance = g.advance.map_or_else(
                || loaded.font.advance_glyph(g.gid).unwrap_or(0.5) * em,
                |a| a * em / 100.0,
            );
            if rtl {
                x -= advance;
            }
            let gx = x + g.dx * em / 100.0;
            let gy = origin_y - g.dy * em / 100.0;
            let _ = writeln!(
                self.content,
                "{} 0 0 {} {} {} Tm <{:04X}> Tj",
                num(em),
                num(-em),
                num(gx),
                num(gy),
                g.gid.clamp(0, 0xFFFF)
            );
            if !rtl {
                x += advance;
            }
        }
        self.content.push_str("ET\nQ\n");
    }

    /// Fuente del paquete (o la de respaldo) incrustada una sola vez; devuelve su nombre de recurso en esta página.
    fn font(&mut self, path: Option<&str>) -> Option<(String, std::rc::Rc<LoadedFont>)> {
        let key = path.unwrap_or("?").to_string();
        let loaded = match self.shared.fonts.get(&key) {
            Some(f) => f.clone(),
            None => {
                let from_package = path.and_then(|p| {
                    let bytes = self.package.read(p, MAX_PART_BYTES).ok().flatten()?;
                    let bytes = if p.to_lowercase().ends_with(".odttf") {
                        deobfuscate(p, bytes)
                    } else {
                        bytes
                    };
                    let name = p
                        .rsplit('/')
                        .next()
                        .unwrap_or("Fuente")
                        .split('.')
                        .next()
                        .unwrap_or("Fuente");
                    Font::from_bytes(name, &bytes).ok()
                });
                let font = match from_package {
                    Some(f) => f,
                    None => {
                        self.shared.skip("fuentes no incrustables (se usa una de respaldo)");
                        Font::new("Helvetica").ok()?
                    }
                };
                let resource = self.pdf.add_font(&font).ok()?;
                let f = std::rc::Rc::new(LoadedFont { resource, font });
                self.shared.fonts.insert(key.clone(), f.clone());
                f
            }
        };
        let name = match self.res.fonts.iter().find(|(_, o)| same_object(o, &loaded.resource)) {
            Some((k, _)) => k.clone(),
            None => {
                let n = format!("F{}", self.res.fonts.len());
                self.res.fonts.insert(n.clone(), loaded.resource.clone());
                n
            }
        };
        Some((name, loaded))
    }
}

fn same_object(a: &Object, b: &Object) -> bool {
    matches!((a.as_indirect(), b.as_indirect()), (Ok(x), Ok(y)) if x != 0 && x == y)
}

/// Quita la ofuscación de una fuente `.odttf`: los 32 primeros bytes se mezclan (XOR) con la GUID del nombre de la
/// parte, leída de derecha a izquierda.
fn deobfuscate(path: &str, mut data: Vec<u8>) -> Vec<u8> {
    let stem = path.rsplit('/').next().unwrap_or("").split('.').next().unwrap_or("");
    let hex: String = stem.chars().filter(char::is_ascii_hexdigit).collect();
    if hex.len() != 32 {
        return data;
    }
    let mut key = [0u8; 16];
    for (i, k) in key.iter_mut().enumerate() {
        // Pareja hexadecimal i-ésima contando desde el final.
        let at = 30 - 2 * i;
        *k = u8::from_str_radix(&hex[at..at + 2], 16).unwrap_or(0);
    }
    for (i, b) in data.iter_mut().take(32).enumerate() {
        *b ^= key[i % 16];
    }
    data
}

struct GlyphRun {
    gid: i32,
    /// Avance en centésimas de em; `None` = el de la fuente.
    advance: Option<f32>,
    dx: f32,
    dy: f32,
}

/// Lista de glifos de un `Glyphs`: `Indices` (`[(cluster)]glifo[,avance[,dx,dy[,flags]]];…`) o, si falta, los
/// caracteres de `UnicodeString` mapeados por la fuente.
fn glyph_runs(text: &str, indices: &str, font: &Font) -> Vec<GlyphRun> {
    let mut chars = text.chars();
    let map = |c: Option<char>| -> i32 { c.and_then(|c| font.encode_character(c as i32).ok()).unwrap_or(0) };
    if indices.trim().is_empty() {
        return chars
            .map(|c| GlyphRun {
                gid: map(Some(c)),
                advance: None,
                dx: 0.0,
                dy: 0.0,
            })
            .collect();
    }
    let mut out = Vec::new();
    for entry in indices.split(';') {
        let mut e = entry.trim();
        if e.is_empty() {
            // Entrada vacía: glifo por defecto de la cmap para el siguiente carácter (Windows escribe
            // `;;;,26` y deja los índices en blanco). Si ya no quedan caracteres es un `;` sobrante.
            if let Some(c) = chars.next() {
                out.push(GlyphRun {
                    gid: map(Some(c)),
                    advance: None,
                    dx: 0.0,
                    dy: 0.0,
                });
            }
            continue;
        }
        // Agrupación `(n:m)`: n caracteres → m glifos. Se usa solo para saber cuántos caracteres se consumen.
        let mut consumed = 1usize;
        if e.starts_with('(')
            && let Some(close) = e.find(')')
        {
            consumed = e[1..close].split(':').next().and_then(|n| n.parse().ok()).unwrap_or(1);
            e = &e[close + 1..];
        }
        let mut f = e.split(',');
        let gid_text = f.next().unwrap_or("").trim();
        let first_char = chars.next();
        for _ in 1..consumed {
            chars.next();
        }
        let gid = gid_text.parse::<i32>().unwrap_or_else(|_| map(first_char));
        let num = |s: Option<&str>| s.and_then(|v| v.trim().parse::<f32>().ok());
        let advance = num(f.next());
        let dx = num(f.next()).unwrap_or(0.0);
        let dy = num(f.next()).unwrap_or(0.0);
        out.push(GlyphRun { gid, advance, dx, dy });
    }
    // La lista de índices puede ser más corta que `UnicodeString`: el resto se mapea con la cmap.
    out.extend(chars.map(|c| GlyphRun {
        gid: map(Some(c)),
        advance: None,
        dx: 0.0,
        dy: 0.0,
    }));
    out
}

// ---------------------------------------------------------------------------------------------------------------------
// Geometría abreviada

struct PathWriter {
    ops: String,
    cur: (f32, f32),
    start: (f32, f32),
    /// Último punto de control de una curva (para `S`/`T`).
    last_ctrl: Option<(f32, f32)>,
    last_cmd: u8,
}

impl PathWriter {
    fn new() -> Self {
        PathWriter {
            ops: String::new(),
            cur: (0.0, 0.0),
            start: (0.0, 0.0),
            last_ctrl: None,
            last_cmd: 0,
        }
    }

    fn move_to(&mut self, x: f32, y: f32) {
        let _ = writeln!(self.ops, "{} {} m", num(x), num(y));
        self.cur = (x, y);
        self.start = (x, y);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let _ = writeln!(self.ops, "{} {} l", num(x), num(y));
        self.cur = (x, y);
    }

    fn curve_to(&mut self, c1: (f32, f32), c2: (f32, f32), p: (f32, f32)) {
        let _ = writeln!(
            self.ops,
            "{} {} {} {} {} {} c",
            num(c1.0),
            num(c1.1),
            num(c2.0),
            num(c2.1),
            num(p.0),
            num(p.1)
        );
        self.cur = p;
        self.last_ctrl = Some(c2);
    }

    fn quad_to(&mut self, c: (f32, f32), p: (f32, f32)) {
        let (x0, y0) = self.cur;
        let c1 = (x0 + 2.0 / 3.0 * (c.0 - x0), y0 + 2.0 / 3.0 * (c.1 - y0));
        let c2 = (p.0 + 2.0 / 3.0 * (c.0 - p.0), p.1 + 2.0 / 3.0 * (c.1 - p.1));
        self.curve_to(c1, c2, p);
        self.last_ctrl = Some(c);
    }

    fn close(&mut self) {
        self.ops.push_str("h\n");
        self.cur = self.start;
    }

    #[allow(clippy::too_many_arguments)]
    fn arc_to(&mut self, rx: f32, ry: f32, rot_deg: f32, large: bool, sweep: bool, x: f32, y: f32) {
        let (x0, y0) = self.cur;
        if (x0 - x).abs() < 1e-6 && (y0 - y).abs() < 1e-6 {
            return;
        }
        let (mut rx, mut ry) = (rx.abs(), ry.abs());
        if rx < 1e-6 || ry < 1e-6 {
            self.line_to(x, y);
            return;
        }
        let phi = rot_deg.to_radians();
        let (cos, sin) = (phi.cos(), phi.sin());
        let (dx, dy) = ((x0 - x) / 2.0, (y0 - y) / 2.0);
        let (x1p, y1p) = (cos * dx + sin * dy, -sin * dx + cos * dy);
        let lambda = (x1p * x1p) / (rx * rx) + (y1p * y1p) / (ry * ry);
        if lambda > 1.0 {
            let s = lambda.sqrt();
            rx *= s;
            ry *= s;
        }
        let num_ = rx * rx * ry * ry - rx * rx * y1p * y1p - ry * ry * x1p * x1p;
        let den = rx * rx * y1p * y1p + ry * ry * x1p * x1p;
        let mut coef = if den == 0.0 { 0.0 } else { (num_ / den).max(0.0).sqrt() };
        if large == sweep {
            coef = -coef;
        }
        let (cxp, cyp) = (coef * rx * y1p / ry, -coef * ry * x1p / rx);
        let (cx, cy) = (
            cos * cxp - sin * cyp + (x0 + x) / 2.0,
            sin * cxp + cos * cyp + (y0 + y) / 2.0,
        );
        let angle = |ux: f32, uy: f32, vx: f32, vy: f32| (ux * vy - uy * vx).atan2(ux * vx + uy * vy);
        let theta1 = angle(1.0, 0.0, (x1p - cxp) / rx, (y1p - cyp) / ry);
        let mut delta = angle((x1p - cxp) / rx, (y1p - cyp) / ry, (-x1p - cxp) / rx, (-y1p - cyp) / ry);
        if !sweep && delta > 0.0 {
            delta -= std::f32::consts::TAU;
        } else if sweep && delta < 0.0 {
            delta += std::f32::consts::TAU;
        }
        let segments = (delta.abs() / std::f32::consts::FRAC_PI_2).ceil().max(1.0) as usize;
        let step = delta / segments as f32;
        let t = 4.0 / 3.0 * (step / 4.0).tan();
        let mut a = theta1;
        for _ in 0..segments {
            let (c1, s1) = (a.cos(), a.sin());
            let (c2, s2) = ((a + step).cos(), (a + step).sin());
            let point = |ux: f32, uy: f32| (cx + rx * ux * cos - ry * uy * sin, cy + rx * ux * sin + ry * uy * cos);
            let p1 = point(c1 - t * s1, s1 + t * c1);
            let p2 = point(c2 + t * s2, s2 - t * c2);
            let p = point(c2, s2);
            self.curve_to(p1, p2, p);
            a += step;
        }
    }
}

/// Analiza la geometría abreviada de XPS (`F1 M 0,0 L 10,0 10,10 Z`). Devuelve los operadores PDF y si el relleno es
/// par-impar (`F0`; con `F1` o sin prefijo es regla del número de vueltas distinto de cero).
fn parse_path_data(data: &str) -> Option<(String, bool)> {
    let b = data.as_bytes();
    let mut i = 0;
    let mut even_odd = false;
    let mut w = PathWriter::new();
    let skip = |i: &mut usize| {
        while *i < b.len() && (b[*i].is_ascii_whitespace() || b[*i] == b',') {
            *i += 1;
        }
    };
    let read_num = |i: &mut usize| -> Option<f32> {
        while *i < b.len() && (b[*i].is_ascii_whitespace() || b[*i] == b',') {
            *i += 1;
        }
        let s = *i;
        if *i < b.len() && (b[*i] == b'-' || b[*i] == b'+') {
            *i += 1;
        }
        let mut dot = false;
        while *i < b.len() {
            match b[*i] {
                b'0'..=b'9' => *i += 1,
                b'.' if !dot => {
                    dot = true;
                    *i += 1;
                }
                b'e' | b'E' if *i > s => {
                    *i += 1;
                    if *i < b.len() && (b[*i] == b'-' || b[*i] == b'+') {
                        *i += 1;
                    }
                }
                _ => break,
            }
        }
        data[s..*i].parse::<f32>().ok()
    };
    let mut cmd = 0u8;
    skip(&mut i);
    // Prefijo de regla de relleno.
    if i + 1 < b.len() && (b[i] == b'F' || b[i] == b'f') && (b[i + 1] == b'0' || b[i + 1] == b'1') {
        even_odd = b[i + 1] == b'0';
        i += 2;
    }
    let mut guard = 0usize;
    loop {
        skip(&mut i);
        if i >= b.len() {
            break;
        }
        guard += 1;
        if guard > 2_000_000 {
            return None;
        }
        if b[i].is_ascii_alphabetic() {
            cmd = b[i];
            i += 1;
            if matches!(cmd, b'Z' | b'z') {
                w.close();
                w.last_cmd = cmd;
                w.last_ctrl = None;
                continue;
            }
        } else if cmd == 0 {
            return None;
        }
        if !cmd.eq_ignore_ascii_case(&b'M') && w.ops.is_empty() {
            return None;
        }
        let rel = cmd.is_ascii_lowercase();
        let (ox, oy) = if rel { w.cur } else { (0.0, 0.0) };
        match cmd.to_ascii_uppercase() {
            b'M' => {
                let (x, y) = (read_num(&mut i)?, read_num(&mut i)?);
                w.move_to(ox + x, oy + y);
                // Los pares siguientes de un M son líneas.
                cmd = if rel { b'l' } else { b'L' };
                w.last_ctrl = None;
            }
            b'L' => {
                let (x, y) = (read_num(&mut i)?, read_num(&mut i)?);
                w.line_to(ox + x, oy + y);
                w.last_ctrl = None;
            }
            b'H' => {
                let x = read_num(&mut i)?;
                let y = w.cur.1;
                w.line_to(if rel { w.cur.0 + x } else { x }, y);
                w.last_ctrl = None;
            }
            b'V' => {
                let y = read_num(&mut i)?;
                let x = w.cur.0;
                w.line_to(x, if rel { w.cur.1 + y } else { y });
                w.last_ctrl = None;
            }
            b'C' => {
                let v: Vec<f32> = (0..6).map(|_| read_num(&mut i)).collect::<Option<_>>()?;
                w.curve_to((ox + v[0], oy + v[1]), (ox + v[2], oy + v[3]), (ox + v[4], oy + v[5]));
            }
            b'S' => {
                let v: Vec<f32> = (0..4).map(|_| read_num(&mut i)).collect::<Option<_>>()?;
                let cur = w.cur;
                let c1 = match (w.last_ctrl, matches!(w.last_cmd.to_ascii_uppercase(), b'C' | b'S')) {
                    (Some(c), true) => (2.0 * cur.0 - c.0, 2.0 * cur.1 - c.1),
                    _ => cur,
                };
                w.curve_to(c1, (ox + v[0], oy + v[1]), (ox + v[2], oy + v[3]));
            }
            b'Q' => {
                let v: Vec<f32> = (0..4).map(|_| read_num(&mut i)).collect::<Option<_>>()?;
                w.quad_to((ox + v[0], oy + v[1]), (ox + v[2], oy + v[3]));
            }
            b'T' => {
                let v: Vec<f32> = (0..2).map(|_| read_num(&mut i)).collect::<Option<_>>()?;
                let cur = w.cur;
                let c = match (w.last_ctrl, matches!(w.last_cmd.to_ascii_uppercase(), b'Q' | b'T')) {
                    (Some(c), true) => (2.0 * cur.0 - c.0, 2.0 * cur.1 - c.1),
                    _ => cur,
                };
                w.quad_to(c, (ox + v[0], oy + v[1]));
            }
            b'A' => {
                let (rx, ry, rot) = (read_num(&mut i)?, read_num(&mut i)?, read_num(&mut i)?);
                let (large, sweep) = (read_num(&mut i)? != 0.0, read_num(&mut i)? != 0.0);
                let (x, y) = (read_num(&mut i)?, read_num(&mut i)?);
                w.arc_to(rx, ry, rot, large, sweep, ox + x, oy + y);
                w.last_ctrl = None;
            }
            _ => return None,
        }
        w.last_cmd = cmd;
    }
    (!w.ops.is_empty()).then_some((w.ops, even_odd))
}

/// Operadores de una `PathFigure` con segmentos: `PolyLineSegment`, `PolyBezierSegment`, `PolyQuadraticBezierSegment`
/// y `ArcSegment`.
fn figure_ops(fig: Node<'_, '_>) -> String {
    let Some(start) = fig.attribute("StartPoint").map(floats).filter(|p| p.len() == 2) else {
        return String::new();
    };
    let mut w = PathWriter::new();
    w.move_to(start[0], start[1]);
    for seg in fig.children().filter(Node::is_element) {
        let points = seg.attribute("Points").map(floats).unwrap_or_default();
        match seg.tag_name().name() {
            "PolyLineSegment" => {
                for p in points.as_chunks::<2>().0 {
                    w.line_to(p[0], p[1]);
                }
            }
            "PolyBezierSegment" => {
                for p in points.as_chunks::<6>().0 {
                    w.curve_to((p[0], p[1]), (p[2], p[3]), (p[4], p[5]));
                }
            }
            "PolyQuadraticBezierSegment" => {
                for p in points.as_chunks::<4>().0 {
                    w.quad_to((p[0], p[1]), (p[2], p[3]));
                }
            }
            "ArcSegment" => {
                let pt = seg.attribute("Point").map(floats).filter(|p| p.len() == 2);
                let size = seg.attribute("Size").map(floats).filter(|p| p.len() == 2);
                if let (Some(pt), Some(size)) = (pt, size) {
                    let rot = seg
                        .attribute("RotationAngle")
                        .and_then(|v| v.trim().parse().ok())
                        .unwrap_or(0.0);
                    let large = seg
                        .attribute("IsLargeArc")
                        .is_some_and(|v| v.eq_ignore_ascii_case("true"));
                    let clockwise = seg
                        .attribute("SweepDirection")
                        .is_some_and(|v| v.eq_ignore_ascii_case("Clockwise"));
                    w.arc_to(size[0], size[1], rot, large, clockwise, pt[0], pt[1]);
                }
            }
            _ => {}
        }
    }
    if fig
        .attribute("IsClosed")
        .is_some_and(|v| v.eq_ignore_ascii_case("true"))
    {
        w.close();
    }
    w.ops
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors() {
        assert_eq!(
            parse_color("#FF8000"),
            Some(Rgba {
                r: 1.0,
                g: 128.0 / 255.0,
                b: 0.0,
                a: 1.0
            })
        );
        let c = parse_color("#80FF0000").unwrap();
        assert!((c.a - 128.0 / 255.0).abs() < 1e-4 && c.r == 1.0 && c.g == 0.0);
        assert_eq!(
            parse_color("sc#0.5,0.25,1"),
            Some(Rgba {
                r: 0.5,
                g: 0.25,
                b: 1.0,
                a: 1.0
            })
        );
        assert_eq!(parse_color("rojo"), None);
    }

    #[test]
    fn empty_indices_use_the_default_glyph() {
        let bytes = std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../pliego-pdf/fonts/liberation/LiberationSans-Regular.ttf"),
        )
        .unwrap();
        let font = Font::from_bytes("Liberation", &bytes).unwrap();
        let gid = |c: char| font.encode_character(c as i32).unwrap();
        // Lo que escribe Windows: entradas vacías y una lista más corta que el texto.
        let runs = glyph_runs("Pagina 1", ";;;;;,26", &font);
        let gids: Vec<i32> = runs.iter().map(|r| r.gid).collect();
        let want: Vec<i32> = "Pagina 1".chars().map(gid).collect();
        assert_eq!(gids, want);
        assert_eq!(runs[5].advance, Some(26.0));
        assert!(runs[..5].iter().all(|r| r.advance.is_none()));
        // Un `;` sobrante al final no añade glifos.
        assert_eq!(glyph_runs("ab", ";;;", &font).len(), 2);
        // Índices explícitos y vacíos mezclados.
        let runs = glyph_runs("AB", ",300;", &font);
        assert_eq!((runs.len(), runs[0].advance, runs[1].gid), (2, Some(300.0), gid('B')));
    }

    #[test]
    fn abbreviated_geometry() {
        let (ops, even_odd) = parse_path_data("F0 M 10,20 L 30,40 50,60 H 70 v 5 Z").unwrap();
        assert!(even_odd);
        assert_eq!(ops, "10 20 m\n30 40 l\n50 60 l\n70 60 l\n70 65 l\nh\n");
        let (ops, even_odd) = parse_path_data("M0,0 C 1,1 2,2 3,3 S 5,5 6,6").unwrap();
        assert!(!even_odd);
        // La primera curva de `S` usa el reflejo del control anterior.
        assert!(ops.contains("4 4 5 5 6 6 c"), "{ops}");
        // Números pegados con signo.
        let (ops, _) = parse_path_data("M1-2L3.5-4.5").unwrap();
        assert_eq!(ops, "1 -2 m\n3.5 -4.5 l\n");
        assert!(parse_path_data("L 1,2").is_none());
        assert!(parse_path_data("M 1").is_none());
    }

    #[test]
    fn arc_becomes_curves_ending_at_the_target() {
        let (ops, _) = parse_path_data("M 0,0 A 10,10 0 0 1 20,0").unwrap();
        let last = ops.lines().last().unwrap();
        assert!(last.ends_with(" c") && last.contains("20 0 c"), "{ops}");
        assert!(ops.lines().filter(|l| l.ends_with(" c")).count() >= 2);
    }

    #[test]
    fn font_deobfuscation_is_its_own_inverse() {
        let path = "Resources/Fonts/01234567-89AB-CDEF-0123-456789ABCDEF.odttf";
        let original: Vec<u8> = (0..64u8).collect();
        let once = deobfuscate(path, original.clone());
        assert_ne!(once[..32], original[..32]);
        assert_eq!(once[32..], original[32..]);
        assert_eq!(deobfuscate(path, once), original);
        // Un nombre que no es una GUID no se toca.
        assert_eq!(deobfuscate("Fonts/arial.odttf", original.clone()), original);
    }

    #[test]
    fn natural_page_order() {
        let mut v = vec!["Pages/10.fpage", "Pages/2.fpage", "Pages/1.fpage"];
        v.sort_by_key(|n| natural_key(n));
        assert_eq!(v, ["Pages/1.fpage", "Pages/2.fpage", "Pages/10.fpage"]);
    }
}
