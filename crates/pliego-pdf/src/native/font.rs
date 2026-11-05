//! Fuentes del motor propio: medir, obtener contornos e incrustar (bloque 5 de la ola 5, W39).
//!
//! Misma API que `font.rs` de la raíz (MuPDF): [`Font`], `PdfDocument::add_simple_font`, `add_font` y
//! `PdfPage::insert_font`. Las 14 fuentes estándar de PDF se sirven con Liberation (SIL OFL 1.1, métricamente
//! compatibles con Arial/Helvetica, Times y Courier New; véase `fonts/liberation/`), embebidas en el binario con
//! `include_bytes!`. Cualquier otra fuente llega con [`Font::from_bytes`].
//!
//! Diferencias con la versión de MuPDF (apuntadas en `docs/handoff/flujos/w39.md`):
//! - `Font::new` solo conoce las 14 estándar y sus alias habituales (Arial, Times New Roman, Courier New); no
//!   busca en las fuentes del sistema. Para otra fuente, `from_bytes`.
//! - Los números de glifo son los de Liberation, no los de la fuente de MuPDF; los avances (en em) coinciden.
//! - `add_simple_font` solo admite la codificación `Latin` (WinAnsi); Greek y Cyrillic devuelven
//!   [`Error::Unsupported`](pliego_core::Error::Unsupported). Ningún consumidor del repositorio las usa.
//! - Las fuentes incrustadas lo van completas (sin subconjunto), igual que MuPDF, pero comprimidas con Flate.
//! - Solo se incrustan fuentes TrueType con contornos `glyf` (no CFF ni colecciones `.ttc`).

use std::collections::BTreeMap;
use std::fmt;
use std::io::Write;
use std::sync::Arc;

use lopdf::{Dictionary, Object as L, ObjectId, Stream};
use pliego_core::{Error, Result};
use ttf_parser::{Face, GlyphId, OutlineBuilder};

use super::metrics::winansi_char;
use super::{Object, PdfDocument, PdfPage};
use crate::font::{PathWalker, SimpleFontEncoding};
use crate::geom::Matrix;

/// Bytes de un archivo de fuente: estáticos (los de Liberation) o del usuario.
#[derive(Clone)]
enum Data {
    Static(&'static [u8]),
    Shared(Arc<[u8]>),
}

impl Data {
    fn bytes(&self) -> &[u8] {
        match self {
            Data::Static(b) => b,
            Data::Shared(b) => b,
        }
    }
}

impl fmt::Debug for Data {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Data({} bytes)", self.bytes().len())
    }
}

/// Fuente cargada. Es barata de clonar.
#[derive(Debug, Clone)]
pub struct Font {
    name: String,
    data: Data,
    /// Nombre estándar PDF si se pidió una de las 14 (la fuente se referencia por nombre, sin incrustar).
    standard: Option<&'static str>,
    serif: bool,
    upem: f32,
    ascender: f32,
    descender: f32,
    bold: bool,
    italic: bool,
    mono: bool,
}

/// Las 14 fuentes estándar: nombre canónico, familia y estilo.
const STANDARD: [(&str, &str, bool, bool); 14] = [
    ("Helvetica", "sans", false, false),
    ("Helvetica-Bold", "sans", true, false),
    ("Helvetica-Oblique", "sans", false, true),
    ("Helvetica-BoldOblique", "sans", true, true),
    ("Times-Roman", "serif", false, false),
    ("Times-Bold", "serif", true, false),
    ("Times-Italic", "serif", false, true),
    ("Times-BoldItalic", "serif", true, true),
    ("Courier", "mono", false, false),
    ("Courier-Bold", "mono", true, false),
    ("Courier-Oblique", "mono", false, true),
    ("Courier-BoldOblique", "mono", true, true),
    ("Symbol", "symbol", false, false),
    ("ZapfDingbats", "symbol", false, false),
];

macro_rules! liberation {
    ($file:literal) => {
        include_bytes!(concat!("../../fonts/liberation/", $file))
    };
}

/// Nombre estándar PDF canónico para un nombre dado (con alias y variantes de mayúsculas/espacios/guiones).
pub(crate) fn standard_name(name: &str) -> Option<(&'static str, &'static str, bool, bool)> {
    let norm: String = name
        .chars()
        .filter(|c| !matches!(c, ' ' | '-' | '_' | ','))
        .flat_map(char::to_lowercase)
        .collect();
    if norm == "symbol" || norm == "zapfdingbats" {
        return STANDARD.iter().copied().find(|s| s.0.to_lowercase() == norm);
    }
    let (family, rest) = ["helvetica", "arial", "timesnewroman", "times", "couriernew", "courier"]
        .iter()
        .find_map(|f| norm.strip_prefix(f).map(|r| (*f, r)))?;
    let rest = rest.strip_suffix("mt").unwrap_or(rest);
    let (bold, italic) = match rest {
        "" | "regular" | "roman" | "normal" => (false, false),
        "bold" => (true, false),
        "italic" | "oblique" => (false, true),
        "bolditalic" | "boldoblique" => (true, true),
        _ => return None,
    };
    let fam = match family {
        "helvetica" | "arial" => "sans",
        "times" | "timesnewroman" => "serif",
        _ => "mono",
    };
    STANDARD
        .iter()
        .copied()
        .find(|s| s.1 == fam && s.2 == bold && s.3 == italic)
}

fn bundled(family: &str, bold: bool, italic: bool) -> &'static [u8] {
    match (family, bold, italic) {
        ("sans", false, false) => liberation!("LiberationSans-Regular.ttf"),
        ("sans", true, false) => liberation!("LiberationSans-Bold.ttf"),
        ("sans", false, true) => liberation!("LiberationSans-Italic.ttf"),
        ("sans", true, true) => liberation!("LiberationSans-BoldItalic.ttf"),
        ("serif", false, false) => liberation!("LiberationSerif-Regular.ttf"),
        ("serif", true, false) => liberation!("LiberationSerif-Bold.ttf"),
        ("serif", false, true) => liberation!("LiberationSerif-Italic.ttf"),
        ("serif", true, true) => liberation!("LiberationSerif-BoldItalic.ttf"),
        (_, false, false) => liberation!("LiberationMono-Regular.ttf"),
        (_, true, false) => liberation!("LiberationMono-Bold.ttf"),
        (_, false, true) => liberation!("LiberationMono-Italic.ttf"),
        (_, true, true) => liberation!("LiberationMono-BoldItalic.ttf"),
    }
}

fn unsupported(msg: &str) -> Error {
    Error::Unsupported(msg.to_string())
}

fn invalid(msg: String) -> Error {
    Error::InvalidInput(msg)
}

impl Font {
    /// Fuente por nombre: las 14 estándar (`Helvetica`, `Times-Roman`, `Courier-Bold`…) y alias comunes. No busca
    /// fuentes del sistema; para otras, [`Font::from_bytes`]. `Symbol` y `ZapfDingbats` no tienen contornos aquí.
    pub fn new(name: &str) -> Result<Font> {
        let (canonical, family, bold, italic) = standard_name(name)
            .ok_or_else(|| unsupported(&format!("fuente «{name}» desconocida: usa Font::from_bytes")))?;
        if family == "symbol" {
            return Err(unsupported(
                "Symbol y ZapfDingbats no están disponibles en el motor propio (solo se pueden referenciar)",
            ));
        }
        let mut font = Font::build(canonical, Data::Static(bundled(family, bold, italic)))?;
        font.standard = Some(canonical);
        font.serif = family == "serif";
        Ok(font)
    }

    /// Fuente a partir de los bytes de un archivo TrueType u OpenType.
    pub fn from_bytes(name: &str, data: &[u8]) -> Result<Font> {
        let mut font = Font::build(name, Data::Shared(Arc::from(data)))?;
        // Sin dato de la familia en la fuente (ttf-parser no expone el PANOSE), se deduce del nombre.
        let lower = name.to_lowercase();
        font.serif = !lower.contains("sans")
            && [
                "serif", "times", "georgia", "garamond", "palatino", "roman", "minion", "cambria", "book",
            ]
            .iter()
            .any(|k| lower.contains(k));
        Ok(font)
    }

    fn build(name: &str, data: Data) -> Result<Font> {
        let face = Face::parse(data.bytes(), 0).map_err(|e| invalid(format!("fuente no válida: {e}")))?;
        let upem = f32::from(face.units_per_em()).max(1.0);
        Ok(Font {
            name: name.to_string(),
            standard: None,
            serif: false,
            upem,
            ascender: f32::from(face.ascender()) / upem,
            descender: f32::from(face.descender()) / upem,
            bold: face.is_bold(),
            italic: face.is_italic() || face.is_oblique(),
            mono: face.is_monospaced(),
            data,
        })
    }

    fn face(&self) -> Result<Face<'_>> {
        Face::parse(self.data.bytes(), 0).map_err(|e| invalid(format!("fuente no válida: {e}")))
    }

    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn is_bold(&self) -> bool {
        self.bold
    }
    pub fn is_italic(&self) -> bool {
        self.italic
    }
    pub fn is_serif(&self) -> bool {
        self.serif
    }
    pub fn is_monospaced(&self) -> bool {
        self.mono
    }
    /// Ascendente en unidades de tamaño (1.0 = el tamaño de la fuente).
    pub fn ascender(&self) -> f32 {
        self.ascender
    }
    pub fn descender(&self) -> f32 {
        self.descender
    }

    /// Glifo de un carácter Unicode (0 = `.notdef`).
    pub fn encode_character(&self, unicode: i32) -> Result<i32> {
        let face = self.face()?;
        Ok(u32::try_from(unicode)
            .ok()
            .and_then(char::from_u32)
            .and_then(|c| face.glyph_index(c))
            .map_or(0, |g| i32::from(g.0)))
    }

    /// Avance horizontal del glifo en unidades de tamaño.
    pub fn advance_glyph(&self, glyph: i32) -> Result<f32> {
        let face = self.face()?;
        let gid = u16::try_from(glyph)
            .ok()
            .filter(|g| *g < face.number_of_glyphs())
            .ok_or_else(|| invalid(format!("glifo {glyph} fuera de rango")))?;
        Ok(face
            .glyph_hor_advance(GlyphId(gid))
            .map_or(0.0, |a| f32::from(a) / self.upem))
    }

    /// Recorre el contorno del glifo (en unidades de tamaño, y hacia arriba) transformado por `ctm`. Devuelve
    /// `false` si el glifo no tiene contorno (un espacio, por ejemplo).
    pub fn outline_glyph(&self, glyph: i32, ctm: &Matrix, walker: &mut dyn PathWalker) -> Result<bool> {
        let face = self.face()?;
        let gid = u16::try_from(glyph)
            .ok()
            .filter(|g| *g < face.number_of_glyphs())
            .ok_or_else(|| invalid(format!("glifo {glyph} fuera de rango")))?;
        let mut b = Outline {
            walker,
            m: *ctm,
            scale: 1.0 / self.upem,
            cur: (0.0, 0.0),
        };
        Ok(face.outline_glyph(GlyphId(gid), &mut b).is_some())
    }
}

/// Adapta el contorno de `ttf-parser` (cuadráticas) a [`PathWalker`] (cúbicas), escalando a em y aplicando `ctm`.
struct Outline<'a> {
    walker: &'a mut dyn PathWalker,
    m: Matrix,
    scale: f32,
    cur: (f32, f32),
}

impl Outline<'_> {
    fn map(&self, x: f32, y: f32) -> (f32, f32) {
        let (x, y) = (x * self.scale, y * self.scale);
        (
            self.m.a * x + self.m.c * y + self.m.e,
            self.m.b * x + self.m.d * y + self.m.f,
        )
    }
}

impl OutlineBuilder for Outline<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.cur = (x, y);
        let (x, y) = self.map(x, y);
        self.walker.move_to(x, y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.cur = (x, y);
        let (x, y) = self.map(x, y);
        self.walker.line_to(x, y);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (x0, y0) = self.cur;
        let (c1x, c1y) = (x0 + 2.0 / 3.0 * (x1 - x0), y0 + 2.0 / 3.0 * (y1 - y0));
        let (c2x, c2y) = (x + 2.0 / 3.0 * (x1 - x), y + 2.0 / 3.0 * (y1 - y));
        self.cur = (x, y);
        let (a, b) = self.map(c1x, c1y);
        let (c, d) = self.map(c2x, c2y);
        let (e, f) = self.map(x, y);
        self.walker.curve_to(a, b, c, d, e, f);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.cur = (x, y);
        let (a, b) = self.map(x1, y1);
        let (c, d) = self.map(x2, y2);
        let (e, f) = self.map(x, y);
        self.walker.curve_to(a, b, c, d, e, f);
    }
    fn close(&mut self) {
        self.walker.close();
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Objetos PDF
// ---------------------------------------------------------------------------------------------------------------

fn name(s: &str) -> L {
    L::Name(s.as_bytes().to_vec())
}

/// Comprime con zlib (`/FlateDecode`).
pub(crate) fn deflate(data: &[u8]) -> Vec<u8> {
    let mut enc =
        flate2::write::ZlibEncoder::new(Vec::with_capacity(data.len() / 2 + 16), flate2::Compression::default());
    // Escribir en un `Vec` no falla.
    let _ = enc.write_all(data);
    enc.finish().unwrap_or_default()
}

/// Flujo con el contenido comprimido con Flate.
pub(crate) fn flate_stream(mut dict: Dictionary, data: &[u8]) -> Stream {
    dict.set("Filter", name("FlateDecode"));
    Stream::new(dict, deflate(data))
}

/// Nombre PostScript sin espacios ni caracteres problemáticos.
fn ps_name(s: &str) -> String {
    let n: String = s
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '+' | '.'))
        .collect();
    if n.is_empty() { "Font".into() } else { n }
}

fn thousandths(v: f32) -> L {
    L::Integer((v * 1000.0).round() as i64)
}

fn embeddable(font: &Font) -> Result<()> {
    let data = font.data.bytes();
    if data.starts_with(b"ttcf") {
        return Err(unsupported("las colecciones de fuentes (.ttc) no se pueden incrustar"));
    }
    if font.face()?.tables().glyf.is_none() {
        return Err(unsupported(
            "solo se incrustan fuentes TrueType con contornos glyf (no CFF/OpenType PostScript)",
        ));
    }
    Ok(())
}

/// `FontDescriptor` con el archivo incrustado.
fn descriptor(st: &mut super::State, font: &Font) -> Result<ObjectId> {
    let face = font.face()?;
    let data = font.data.bytes();
    let mut file = Dictionary::new();
    file.set("Length1", L::Integer(data.len() as i64));
    let file_id = st.add(L::Stream(flate_stream(file, data)));

    let bb = face.global_bounding_box();
    let k = 1000.0 / font.upem;
    let mut flags = 32i64;
    if font.mono {
        flags |= 1;
    }
    if font.serif {
        flags |= 2;
    }
    if font.italic {
        flags |= 64;
    }
    if font.bold {
        flags |= 1 << 18;
    }
    let cap = face
        .capital_height()
        .map_or(font.ascender * 1000.0, |c| f32::from(c) * k);
    let mut d = Dictionary::new();
    d.set("Type", name("FontDescriptor"));
    d.set("FontName", name(&ps_name(&font.name)));
    d.set("Flags", L::Integer(flags));
    d.set(
        "FontBBox",
        L::Array(
            [bb.x_min, bb.y_min, bb.x_max, bb.y_max]
                .iter()
                .map(|v| L::Integer((f32::from(*v) * k).round() as i64))
                .collect(),
        ),
    );
    d.set("ItalicAngle", L::Real(face.italic_angle()));
    d.set("Ascent", thousandths(font.ascender));
    d.set("Descent", thousandths(font.descender));
    d.set("CapHeight", L::Integer(cap.round() as i64));
    d.set("StemV", L::Integer(if font.bold { 140 } else { 80 }));
    d.set("FontFile2", L::Reference(file_id));
    Ok(st.add(L::Dictionary(d)))
}

/// CMap `ToUnicode` que invierte la tabla `cmap` de la fuente (glifo → primer carácter Unicode que lo usa).
fn to_unicode(face: &Face<'_>) -> Vec<u8> {
    let mut map: BTreeMap<u16, u32> = BTreeMap::new();
    if let Some(cmap) = face.tables().cmap {
        for sub in cmap.subtables.into_iter().filter(|s| s.is_unicode()) {
            sub.codepoints(|cp| {
                if let Some(g) = sub.glyph_index(cp).filter(|g| g.0 != 0) {
                    map.entry(g.0).and_modify(|c| *c = (*c).min(cp)).or_insert(cp);
                }
            });
        }
    }
    let mut out = String::from(
        "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n\
         /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n\
         /CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n\
         1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
    );
    let entries: Vec<(u16, u32)> = map.into_iter().collect();
    for chunk in entries.chunks(100) {
        out.push_str(&format!("{} beginbfchar\n", chunk.len()));
        for (gid, cp) in chunk {
            let mut buf = [0u16; 2];
            let units = char::from_u32(*cp).map_or(&[0xfffd_u16][..], |c| &*c.encode_utf16(&mut buf));
            let hex: String = units.iter().map(|u| format!("{u:04X}")).collect();
            out.push_str(&format!("<{gid:04X}> <{hex}>\n"));
        }
        out.push_str("endbfchar\n");
    }
    out.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    out.into_bytes()
}

/// Diccionario `Type1` de una fuente estándar sin incrustar. `Symbol` y `ZapfDingbats` no llevan `Encoding`.
pub(crate) fn standard_font_dict(canonical: &str) -> Dictionary {
    let mut f = Dictionary::new();
    f.set("Type", name("Font"));
    f.set("Subtype", name("Type1"));
    f.set("BaseFont", name(canonical));
    if canonical != "Symbol" && canonical != "ZapfDingbats" {
        f.set("Encoding", name("WinAnsiEncoding"));
    }
    f
}

impl PdfDocument {
    /// Incorpora `font` como fuente simple (un byte por carácter, WinAnsi) y devuelve su diccionario (referencia
    /// indirecta), listo para `/Resources /Font`. Las 14 estándar se referencian por nombre sin incrustar; el resto
    /// se incrusta como TrueType.
    pub fn add_simple_font(&mut self, font: &Font, encoding: SimpleFontEncoding) -> Result<Object> {
        self.guard()?;
        if encoding != SimpleFontEncoding::Latin {
            return Err(unsupported(
                "el motor propio solo admite la codificación Latin (WinAnsi) en fuentes simples",
            ));
        }
        if let Some(canonical) = font.standard {
            let id = self
                .state()
                .borrow_mut()
                .add(L::Dictionary(standard_font_dict(canonical)));
            return Ok(Object::reference(self.state(), id));
        }
        embeddable(font)?;
        let face = font.face()?;
        let widths: Vec<L> = (32u8..=255)
            .map(|code| {
                let w = winansi_char(code)
                    .and_then(|c| face.glyph_index(c))
                    .and_then(|g| face.glyph_hor_advance(g))
                    .map_or(0.0, |a| f32::from(a) / font.upem);
                thousandths(w)
            })
            .collect();
        let mut st = self.state().borrow_mut();
        let desc = descriptor(&mut st, font)?;
        let mut d = Dictionary::new();
        d.set("Type", name("Font"));
        d.set("Subtype", name("TrueType"));
        d.set("BaseFont", name(&ps_name(&font.name)));
        d.set("Encoding", name("WinAnsiEncoding"));
        d.set("FirstChar", L::Integer(32));
        d.set("LastChar", L::Integer(255));
        d.set("Widths", L::Array(widths));
        d.set("FontDescriptor", L::Reference(desc));
        let id = st.add(L::Dictionary(d));
        drop(st);
        Ok(Object::reference(self.state(), id))
    }

    /// Incorpora `font` como fuente Unicode (Type0, Identity-H, CIDFontType2): el texto se escribe con los números
    /// de glifo de 2 bytes (véase [`Font::encode_character`]). Siempre se incrusta, también las estándar.
    pub fn add_font(&mut self, font: &Font) -> Result<Object> {
        self.guard()?;
        embeddable(font)?;
        let face = font.face()?;
        let widths: Vec<L> = (0..face.number_of_glyphs())
            .map(|g| {
                thousandths(
                    face.glyph_hor_advance(GlyphId(g))
                        .map_or(0.0, |a| f32::from(a) / font.upem),
                )
            })
            .collect();
        let tounicode = to_unicode(&face);
        let mut st = self.state().borrow_mut();
        let desc = descriptor(&mut st, font)?;
        let w_id = st.add(L::Array(vec![L::Integer(0), L::Array(widths)]));
        let tu_id = st.add(L::Stream(flate_stream(Dictionary::new(), &tounicode)));

        let mut info = Dictionary::new();
        info.set("Registry", L::string_literal("Adobe"));
        info.set("Ordering", L::string_literal("Identity"));
        info.set("Supplement", L::Integer(0));
        let mut cid = Dictionary::new();
        cid.set("Type", name("Font"));
        cid.set("Subtype", name("CIDFontType2"));
        cid.set("BaseFont", name(&ps_name(&font.name)));
        cid.set("CIDSystemInfo", L::Dictionary(info));
        cid.set("FontDescriptor", L::Reference(desc));
        cid.set("CIDToGIDMap", name("Identity"));
        cid.set("DW", L::Integer(1000));
        cid.set("W", L::Reference(w_id));
        let cid_id = st.add(L::Dictionary(cid));

        let mut d = Dictionary::new();
        d.set("Type", name("Font"));
        d.set("Subtype", name("Type0"));
        d.set("BaseFont", name(&ps_name(&font.name)));
        d.set("Encoding", name("Identity-H"));
        d.set("DescendantFonts", L::Array(vec![L::Reference(cid_id)]));
        d.set("ToUnicode", L::Reference(tu_id));
        let id = st.add(L::Dictionary(d));
        drop(st);
        Ok(Object::reference(self.state(), id))
    }
}

impl PdfPage {
    /// Añade a los recursos de la página una fuente estándar por nombre (si no estaba ya) y devuelve `(nombre de
    /// recurso con barra, número de objeto)`. Se reutiliza una fuente `Type1` del mismo `BaseFont` que ya exista.
    pub fn insert_font(&mut self, doc: &mut PdfDocument, name: &str) -> Result<(String, i32)> {
        doc.guard()?;
        let (canonical, ..) =
            standard_name(name).ok_or_else(|| unsupported(&format!("fuente estándar «{name}» desconocida")))?;

        // Recursos propios de la página (si los heredaba, se copian a la página para no tocar a las demás).
        let mut res = self.resources()?;
        let mut fonts = match res.get_dict("Font")? {
            Some(f) if f.is_dict()? => f,
            _ => {
                res.dict_put("Font", doc.new_dict()?)?;
                res.get_dict("Font")?
                    .ok_or_else(|| invalid("no se pudo crear /Font".into()))?
            }
        };

        let mut used = Vec::new();
        for (key, val) in fonts.dict_iter()? {
            used.push(key.clone());
            let Some(target) = val.resolve()? else { continue };
            let same = target.is_dict()?
                && target.dict_get_name("BaseFont")?.as_deref() == Some(canonical)
                && target.dict_get_name("Subtype")?.as_deref() == Some("Type1");
            if same && val.is_indirect()? {
                return Ok((format!("/{key}"), val.as_indirect()?));
            }
        }
        let n = (0..).find(|n| !used.contains(&format!("F{n}"))).unwrap_or(0);
        let key = format!("F{n}");
        let id = doc
            .state()
            .borrow_mut()
            .add(L::Dictionary(standard_font_dict(canonical)));
        fonts.dict_put(&key, Object::reference(doc.state(), id))?;
        Ok((format!("/{key}"), i32::try_from(id.0).unwrap_or(0)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::Size;

    struct Collect(Vec<String>);
    impl PathWalker for Collect {
        fn move_to(&mut self, x: f32, y: f32) {
            self.0.push(format!("M{x:.3},{y:.3}"));
        }
        fn line_to(&mut self, x: f32, y: f32) {
            self.0.push(format!("L{x:.3},{y:.3}"));
        }
        fn curve_to(&mut self, _: f32, _: f32, _: f32, _: f32, x: f32, y: f32) {
            self.0.push(format!("C{x:.3},{y:.3}"));
        }
        fn close(&mut self) {
            self.0.push("Z".into());
        }
    }

    fn width(font: &Font, text: &str) -> f32 {
        text.chars()
            .map(|c| {
                let g = font.encode_character(c as i32).unwrap();
                font.advance_glyph(g).unwrap()
            })
            .sum()
    }

    #[test]
    fn standard_names_and_aliases() {
        for (asked, canonical) in [
            ("Helvetica", "Helvetica"),
            ("helvetica-bold", "Helvetica-Bold"),
            ("Arial", "Helvetica"),
            ("Arial,Bold", "Helvetica-Bold"),
            ("Times New Roman", "Times-Roman"),
            ("Times-Italic", "Times-Italic"),
            ("TimesNewRomanPS-BoldItalicMT", ""),
            ("Courier New", "Courier"),
            ("Courier-BoldOblique", "Courier-BoldOblique"),
        ] {
            let got = standard_name(asked).map_or("", |s| s.0);
            assert_eq!(got, canonical, "{asked}");
        }
        assert!(Font::new("Comic Sans").is_err());
        assert!(Font::new("Symbol").is_err());
        assert!(standard_name("ZapfDingbats").is_some());
    }

    #[test]
    fn metrics_match_the_standard_widths() {
        // Mismas anchuras AFM que `metrics.rs` (y que MuPDF): Liberation es métricamente compatible.
        let h = Font::new("Helvetica").unwrap();
        assert_eq!(h.name(), "Helvetica");
        assert!(!h.is_bold() && !h.is_italic() && !h.is_serif() && !h.is_monospaced());
        assert!((width(&h, "Hello") - 2.278).abs() < 2e-3, "{}", width(&h, "Hello"));
        let t = Font::new("Times-Bold").unwrap();
        assert!(t.is_bold() && t.is_serif());
        assert!((width(&t, "A") - 0.722).abs() < 2e-3);
        let c = Font::new("Courier-Oblique").unwrap();
        assert!(c.is_monospaced() && c.is_italic());
        assert!((width(&c, "abcd") - 2.4).abs() < 1e-3);
        assert!(h.ascender() > 0.7 && h.descender() < 0.0);
        // Un carácter inexistente va a .notdef; fuera de rango es error.
        assert_eq!(h.encode_character(0x65e5).unwrap(), 0);
        assert_eq!(h.encode_character(-5).unwrap(), 0);
        assert!(h.advance_glyph(99_999).is_err());
    }

    #[test]
    fn outlines_scale_and_transform() {
        let f = Font::new("Helvetica").unwrap();
        let g = f.encode_character('I' as i32).unwrap();
        let mut a = Collect(Vec::new());
        assert!(f.outline_glyph(g, &Matrix::IDENTITY, &mut a).unwrap());
        assert!(a.0.first().is_some_and(|s| s.starts_with('M')) && a.0.last().is_some_and(|s| s == "Z"));
        // La «I» de Liberation Sans mide 0,688 em de alto: con ctm de escala 10 y desplazamiento (5, 7) cambian las coordenadas.
        let ctm = Matrix {
            a: 10.0,
            b: 0.0,
            c: 0.0,
            d: 10.0,
            e: 5.0,
            f: 7.0,
        };
        let mut b = Collect(Vec::new());
        f.outline_glyph(g, &ctm, &mut b).unwrap();
        let ys: Vec<f32> =
            b.0.iter()
                .filter_map(|s| s[1..].split(',').nth(1)?.parse().ok())
                .collect();
        let top = ys.iter().copied().fold(f32::MIN, f32::max);
        assert!((top - (7.0 + 0.688 * 10.0)).abs() < 0.2, "{top}");
        // El espacio no tiene contorno.
        let sp = f.encode_character(32).unwrap();
        assert!(
            !f.outline_glyph(sp, &Matrix::IDENTITY, &mut Collect(Vec::new()))
                .unwrap()
        );
    }

    #[test]
    fn standard_simple_font_is_not_embedded() {
        let mut doc = PdfDocument::new();
        let f = Font::new("Times-Roman").unwrap();
        let o = doc.add_simple_font(&f, SimpleFontEncoding::Latin).unwrap();
        assert!(o.is_indirect().unwrap());
        let d = o.resolve().unwrap().unwrap();
        assert_eq!(d.dict_get_name("BaseFont").unwrap().as_deref(), Some("Times-Roman"));
        assert_eq!(d.dict_get_name("Subtype").unwrap().as_deref(), Some("Type1"));
        assert_eq!(d.dict_get_name("Encoding").unwrap().as_deref(), Some("WinAnsiEncoding"));
        assert!(d.get_dict("FontDescriptor").unwrap().is_none());
        assert!(doc.add_simple_font(&f, SimpleFontEncoding::Greek).is_err());
    }

    #[test]
    fn embedded_fonts_have_descriptor_widths_and_tounicode() {
        let data = include_bytes!("../../fonts/liberation/LiberationSerif-Italic.ttf");
        let f = Font::from_bytes("Mi Serif Cursiva", data).unwrap();
        assert!(f.is_italic() && f.is_serif());
        let mut doc = PdfDocument::new();

        let simple = doc
            .add_simple_font(&f, SimpleFontEncoding::Latin)
            .unwrap()
            .resolve()
            .unwrap()
            .unwrap();
        assert_eq!(simple.dict_get_name("Subtype").unwrap().as_deref(), Some("TrueType"));
        assert_eq!(
            simple.dict_get_name("BaseFont").unwrap().as_deref(),
            Some("MiSerifCursiva")
        );
        assert_eq!(simple.get_dict("Widths").unwrap().unwrap().len().unwrap(), 224);
        let fd = simple.get_dict("FontDescriptor").unwrap().unwrap();
        let file = fd.get_dict("FontFile2").unwrap().unwrap();
        assert_eq!(file.read_stream().unwrap(), data.to_vec());
        assert!(file.read_raw_stream().unwrap().len() < data.len());
        // «A» (65) mide 0,611 em en Times Italic.
        let widths = simple.get_dict("Widths").unwrap().unwrap();
        assert_eq!(widths.get_array(65 - 32).unwrap().unwrap().as_int().unwrap(), 611);

        let uni = doc.add_font(&f).unwrap().resolve().unwrap().unwrap();
        assert_eq!(uni.dict_get_name("Subtype").unwrap().as_deref(), Some("Type0"));
        assert_eq!(uni.dict_get_name("Encoding").unwrap().as_deref(), Some("Identity-H"));
        let cid = uni
            .get_dict("DescendantFonts")
            .unwrap()
            .unwrap()
            .get_array(0)
            .unwrap()
            .unwrap();
        let cid = cid.resolve().unwrap().unwrap();
        assert_eq!(cid.dict_get_name("Subtype").unwrap().as_deref(), Some("CIDFontType2"));
        let tu = String::from_utf8(uni.get_dict("ToUnicode").unwrap().unwrap().read_stream().unwrap()).unwrap();
        let gid = f.encode_character('ñ' as i32).unwrap();
        assert!(
            tu.contains(&format!("<{gid:04X}> <00F1>")),
            "{}",
            &tu[..tu.len().min(400)]
        );

        // Una fuente que no es TrueType glyf no se incrusta.
        assert!(Font::from_bytes("x", b"no es una fuente").is_err());
    }

    #[test]
    fn insert_font_reuses_and_numbers_resources() {
        let mut doc = PdfDocument::new();
        let mut page = doc
            .new_page(Size {
                width: 200.0,
                height: 200.0,
            })
            .unwrap();
        let (a, xa) = page.insert_font(&mut doc, "Helvetica").unwrap();
        let (b, xb) = page.insert_font(&mut doc, "Arial").unwrap();
        assert_eq!((a.as_str(), xa), ("/F0", xb));
        assert_eq!(b, "/F0");
        let (c, xc) = page.insert_font(&mut doc, "Times-Italic").unwrap();
        assert_eq!(c, "/F1");
        assert_ne!(xc, xa);
        assert!(page.insert_font(&mut doc, "Nope").is_err());
        let fonts = page
            .object()
            .get_dict("Resources")
            .unwrap()
            .unwrap()
            .get_dict("Font")
            .unwrap()
            .unwrap();
        assert_eq!(fonts.dict_len().unwrap(), 2);
    }
}
