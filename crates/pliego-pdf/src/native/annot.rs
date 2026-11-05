//! Anotaciones (`/Annots`) sobre lopdf: crear, leer, modificar, borrar y regenerar su apariencia.
//!
//! Misma API pública que [`crate::PdfAnnotation`] (la de la capa sobre MuPDF), de modo que el cambio de motor
//! sea intercambiar el módulo. Todas las coordenadas que entran y salen están en el **espacio de página** (el de
//! `Page::bounds`, el texto y los enlaces); dentro del archivo se guardan en el espacio de usuario del PDF y la
//! conversión se hace con la matriz de la página.
//!
//! Qué apariencias se regeneran en [`PdfAnnotation::update`]: marcado de texto (resaltado, subrayado, tachado,
//! ondulado), cuadrado, círculo, línea, polígono, polilínea, lápiz, nota, texto libre, cursor de inserción,
//! redacción pendiente y sello sin apariencia. Los demás tipos (enlaces, campos, adjuntos, anotaciones de otros
//! programas) conservan la que traen. Una anotación solo se regenera si algo la ha cambiado desde la última vez
//! (como hace MuPDF), así que abrir y guardar un archivo ajeno no toca sus apariencias.

use lopdf::{Dictionary, Object as L, ObjectId, Stream};
use pliego_core::{Error, Result};

use super::appearance::{self as ap, Ap, Ending, Q, Style};
use super::document::{decode_text, text_string};
use super::object::Object;
use super::page::{PdfPage, page_ctm};
use super::{Shared, State, pdf_err};
use crate::annot::{
    AnnotationBorderStyle, AnnotationColor, AnnotationDefaultAppearance, AnnotationFlags, AnnotationTextAlign,
    LineEndingStyle, PdfAnnotationType, RedactOptions,
};
use crate::geom::{Matrix, Point, Quad, Rect};

/// Anotación de una página. Mientras exista, su página debe seguir viva.
pub struct PdfAnnotation {
    pub(crate) obj: Object,
    pub(crate) page: Object,
}

// --- Nombres de subtipo --------------------------------------------------------------------------------------------

const SUBTYPES: &[(&str, PdfAnnotationType)] = &[
    ("Text", PdfAnnotationType::Text),
    ("Link", PdfAnnotationType::Link),
    ("FreeText", PdfAnnotationType::FreeText),
    ("Line", PdfAnnotationType::Line),
    ("Square", PdfAnnotationType::Square),
    ("Circle", PdfAnnotationType::Circle),
    ("Polygon", PdfAnnotationType::Polygon),
    ("PolyLine", PdfAnnotationType::PolyLine),
    ("Highlight", PdfAnnotationType::Highlight),
    ("Underline", PdfAnnotationType::Underline),
    ("Squiggly", PdfAnnotationType::Squiggly),
    ("StrikeOut", PdfAnnotationType::StrikeOut),
    ("Redact", PdfAnnotationType::Redact),
    ("Stamp", PdfAnnotationType::Stamp),
    ("Caret", PdfAnnotationType::Caret),
    ("Ink", PdfAnnotationType::Ink),
    ("Popup", PdfAnnotationType::Popup),
    ("FileAttachment", PdfAnnotationType::FileAttachment),
    ("Sound", PdfAnnotationType::Sound),
    ("Movie", PdfAnnotationType::Movie),
    ("RichMedia", PdfAnnotationType::RichMedia),
    ("Widget", PdfAnnotationType::Widget),
    ("Screen", PdfAnnotationType::Screen),
    ("PrinterMark", PdfAnnotationType::PrinterMark),
    ("TrapNet", PdfAnnotationType::TrapNet),
    ("Watermark", PdfAnnotationType::Watermark),
    ("3D", PdfAnnotationType::ThreeD),
    ("Projection", PdfAnnotationType::Projection),
];

fn subtype_name(t: PdfAnnotationType) -> &'static str {
    SUBTYPES.iter().find(|(_, v)| *v == t).map_or("Text", |(n, _)| n)
}

fn subtype_of(name: &[u8]) -> PdfAnnotationType {
    SUBTYPES
        .iter()
        .find(|(n, _)| n.as_bytes() == name)
        .map_or(PdfAnnotationType::Unknown, |(_, v)| *v)
}

// --- Lectura de diccionarios ---------------------------------------------------------------------------------------

fn dget<'a>(st: &'a State, d: &'a Dictionary, key: &str) -> Option<&'a L> {
    d.get(key.as_bytes()).ok().and_then(|v| st.deref(v))
}

fn num(st: &State, o: &L) -> Option<f32> {
    st.deref(o).and_then(|v| v.as_float().ok())
}

fn dfloats(st: &State, d: &Dictionary, key: &str) -> Option<Vec<f32>> {
    let arr = dget(st, d, key)?.as_array().ok()?;
    arr.iter().map(|x| num(st, x)).collect()
}

fn dfloat(st: &State, d: &Dictionary, key: &str) -> Option<f32> {
    num(st, d.get(key.as_bytes()).ok()?)
}

fn dname(st: &State, d: &Dictionary, key: &str) -> Option<Vec<u8>> {
    dget(st, d, key)?.as_name().ok().map(<[u8]>::to_vec)
}

fn dtext(st: &State, d: &Dictionary, key: &str) -> Option<String> {
    decode_text(dget(st, d, key)?)
}

fn reals(v: &[f32]) -> L {
    L::Array(v.iter().map(|x| L::Real(*x)).collect())
}

fn rect_array(r: Rect) -> L {
    reals(&[r.x0, r.y0, r.x1, r.y1])
}

fn color_from_array(c: &[f32]) -> Option<AnnotationColor> {
    match c.len() {
        1 => Some(AnnotationColor::Gray(c[0])),
        3 => Some(AnnotationColor::Rgb {
            red: c[0],
            green: c[1],
            blue: c[2],
        }),
        4 => Some(AnnotationColor::Cmyk {
            cyan: c[0],
            magenta: c[1],
            yellow: c[2],
            key: c[3],
        }),
        _ => None,
    }
}

fn color_to_array(c: AnnotationColor) -> Vec<f32> {
    match c {
        AnnotationColor::Gray(g) => vec![g],
        AnnotationColor::Rgb { red, green, blue } => vec![red, green, blue],
        AnnotationColor::Cmyk {
            cyan,
            magenta,
            yellow,
            key,
        } => vec![cyan, magenta, yellow, key],
    }
}

fn pts_of(flat: &[f32]) -> Vec<Point> {
    flat.chunks(2)
        .filter(|c| c.len() == 2)
        .map(|c| Point::new(c[0], c[1]))
        .collect()
}

fn le_name(s: LineEndingStyle) -> &'static str {
    match s {
        LineEndingStyle::None => "None",
        LineEndingStyle::Square => "Square",
        LineEndingStyle::Circle => "Circle",
        LineEndingStyle::Diamond => "Diamond",
        LineEndingStyle::OpenArrow => "OpenArrow",
        LineEndingStyle::ClosedArrow => "ClosedArrow",
        LineEndingStyle::Butt => "Butt",
        LineEndingStyle::ROpenArrow => "ROpenArrow",
        LineEndingStyle::RClosedArrow => "RClosedArrow",
        LineEndingStyle::Slash => "Slash",
    }
}

fn le_from(name: &[u8]) -> LineEndingStyle {
    match name {
        b"Square" => LineEndingStyle::Square,
        b"Circle" => LineEndingStyle::Circle,
        b"Diamond" => LineEndingStyle::Diamond,
        b"OpenArrow" => LineEndingStyle::OpenArrow,
        b"ClosedArrow" => LineEndingStyle::ClosedArrow,
        b"Butt" => LineEndingStyle::Butt,
        b"ROpenArrow" => LineEndingStyle::ROpenArrow,
        b"RClosedArrow" => LineEndingStyle::RClosedArrow,
        b"Slash" => LineEndingStyle::Slash,
        _ => LineEndingStyle::None,
    }
}

fn ending_of(s: LineEndingStyle) -> Ending {
    match s {
        LineEndingStyle::None => Ending::None,
        LineEndingStyle::Square => Ending::Square,
        LineEndingStyle::Circle => Ending::Circle,
        LineEndingStyle::Diamond => Ending::Diamond,
        LineEndingStyle::OpenArrow => Ending::OpenArrow,
        LineEndingStyle::ClosedArrow => Ending::ClosedArrow,
        LineEndingStyle::Butt => Ending::Butt,
        LineEndingStyle::ROpenArrow => Ending::ROpenArrow,
        LineEndingStyle::RClosedArrow => Ending::RClosedArrow,
        LineEndingStyle::Slash => Ending::Slash,
    }
}

impl PdfAnnotation {
    pub(crate) fn new(obj: Object, page: Object) -> Self {
        Self { obj, page }
    }

    fn shared(&self) -> Result<Shared> {
        self.obj
            .st
            .clone()
            .ok_or_else(|| pdf_err("la anotación no pertenece a ningún documento"))
    }

    /// Lee el diccionario de la anotación con acceso al estado (para resolver referencias).
    fn read<R>(&self, f: impl FnOnce(&State, &Dictionary) -> R) -> Result<R> {
        let shared = self.shared()?;
        self.obj.read(|o| {
            let g = shared.borrow();
            match o {
                L::Dictionary(d) => Ok(f(&g, d)),
                L::Stream(s) => Ok(f(&g, &s.dict)),
                _ => Err(pdf_err("la anotación no es un diccionario")),
            }
        })
    }

    /// Escribe una clave y marca la apariencia como pendiente de regenerar.
    fn set(&self, key: &str, v: L) -> Result<()> {
        self.obj.dict_set(key.as_bytes(), v)?;
        self.mark_dirty()
    }

    fn mark_dirty(&self) -> Result<()> {
        if let Some(id) = self.obj.obj_id() {
            self.shared()?.borrow_mut().annot_dirty.insert(id);
        }
        Ok(())
    }

    /// Matriz de espacio de usuario → espacio de página y su inversa.
    fn ctms(&self) -> Result<(Matrix, Matrix)> {
        let m = page_ctm(&self.page)?;
        let inv = m
            .invert()
            .ok_or_else(|| pdf_err("la matriz de la página no es invertible"))?;
        Ok((m, inv))
    }

    fn user_rect(&self) -> Result<Rect> {
        self.read(|st, d| dfloats(st, d, "Rect"))?
            .filter(|v| v.len() >= 4)
            .map(|v| Rect::new(v[0].min(v[2]), v[1].min(v[3]), v[0].max(v[2]), v[1].max(v[3])))
            .ok_or_else(|| pdf_err("la anotación no tiene /Rect"))
    }

    /// Escribe `/Rect` sin marcar la anotación como pendiente (lo usan los ajustes que hace `update`).
    fn set_user_rect(&self, r: Rect) -> Result<()> {
        self.obj.dict_set(b"Rect", rect_array(r))
    }

    // --- Propiedades básicas -------------------------------------------------------------------------------------

    pub fn r#type(&self) -> Result<PdfAnnotationType> {
        self.read(|st, d| dname(st, d, "Subtype"))?
            .map(|n| subtype_of(&n))
            .ok_or_else(|| pdf_err("la anotación no tiene /Subtype"))
    }

    /// Diccionario de la anotación.
    pub fn object(&self) -> Object {
        self.obj.clone()
    }

    pub fn xref(&self) -> Result<i32> {
        Ok(self.obj.obj_id().map_or(0, |id| id.0 as i32))
    }

    pub fn bounds(&self) -> Result<Rect> {
        self.rect()
    }

    /// Rectángulo en el espacio de página.
    pub fn rect(&self) -> Result<Rect> {
        let (m, _) = self.ctms()?;
        Ok(self.user_rect()?.transform(&m))
    }

    pub fn set_rect(&mut self, rect: Rect) -> Result<()> {
        let (_, inv) = self.ctms()?;
        self.set_user_rect(rect.transform(&inv))?;
        self.mark_dirty()
    }

    pub fn flags(&self) -> Result<AnnotationFlags> {
        Ok(AnnotationFlags(
            self.read(|st, d| dfloat(st, d, "F"))?.map_or(0, |f| f as i32),
        ))
    }

    pub fn set_flags(&mut self, flags: AnnotationFlags) -> Result<()> {
        self.set("F", L::Integer(i64::from(flags.bits())))
    }

    pub fn color(&self) -> Result<Option<AnnotationColor>> {
        Ok(self
            .read(|st, d| dfloats(st, d, "C"))?
            .and_then(|c| color_from_array(&c)))
    }

    pub fn set_color(&mut self, color: AnnotationColor) -> Result<()> {
        self.set("C", reals(&color_to_array(color)))
    }

    pub fn interior_color(&self) -> Result<Option<AnnotationColor>> {
        Ok(self
            .read(|st, d| dfloats(st, d, "IC"))?
            .and_then(|c| color_from_array(&c)))
    }

    pub fn set_interior_color(&mut self, color: AnnotationColor) -> Result<()> {
        self.set("IC", reals(&color_to_array(color)))
    }

    pub fn opacity(&self) -> Result<f32> {
        Ok(self.read(|st, d| dfloat(st, d, "CA"))?.unwrap_or(1.0).clamp(0.0, 1.0))
    }

    pub fn set_opacity(&mut self, opacity: f32) -> Result<()> {
        self.set("CA", L::Real(opacity.clamp(0.0, 1.0)))
    }

    pub fn border_width(&self) -> Result<f32> {
        self.read(|st, d| {
            if let Some(L::Dictionary(bs)) = dget(st, d, "BS")
                && let Some(w) = dfloat(st, bs, "W")
            {
                return w;
            }
            dfloats(st, d, "Border").and_then(|b| b.get(2).copied()).unwrap_or(1.0)
        })
    }

    pub fn set_border_width(&mut self, width: f32) -> Result<()> {
        self.update_bs(|bs| bs.set("W", L::Real(width.max(0.0))))
    }

    fn update_bs(&self, f: impl FnOnce(&mut Dictionary)) -> Result<()> {
        let mut bs = self.read(|st, d| match dget(st, d, "BS") {
            Some(L::Dictionary(b)) => b.clone(),
            _ => Dictionary::new(),
        })?;
        if !bs.has(b"Type") {
            bs.set("Type", L::Name(b"Border".to_vec()));
        }
        f(&mut bs);
        self.set("BS", L::Dictionary(bs))
    }

    pub fn border_style(&self) -> Result<AnnotationBorderStyle> {
        Ok(self
            .read(|st, d| match dget(st, d, "BS") {
                Some(L::Dictionary(bs)) => dname(st, bs, "S"),
                _ => None,
            })?
            .map_or(AnnotationBorderStyle::Solid, |s| match s.as_slice() {
                b"D" => AnnotationBorderStyle::Dashed,
                b"B" => AnnotationBorderStyle::Beveled,
                b"I" => AnnotationBorderStyle::Inset,
                b"U" => AnnotationBorderStyle::Underline,
                _ => AnnotationBorderStyle::Solid,
            }))
    }

    pub fn set_border_style(&mut self, style: AnnotationBorderStyle) -> Result<()> {
        let s: &[u8] = match style {
            AnnotationBorderStyle::Solid => b"S",
            AnnotationBorderStyle::Dashed => b"D",
            AnnotationBorderStyle::Beveled => b"B",
            AnnotationBorderStyle::Inset => b"I",
            AnnotationBorderStyle::Underline => b"U",
        };
        self.update_bs(|bs| bs.set("S", L::Name(s.to_vec())))
    }

    pub fn border_dash_pattern(&self) -> Result<Vec<f32>> {
        Ok(self
            .read(|st, d| match dget(st, d, "BS") {
                Some(L::Dictionary(bs)) => dfloats(st, bs, "D"),
                _ => None,
            })?
            .unwrap_or_default())
    }

    pub fn set_border_dash_pattern(&mut self, lengths: &[f32]) -> Result<()> {
        self.update_bs(|bs| bs.set("D", reals(lengths)))
    }

    pub fn author(&self) -> Result<Option<String>> {
        Ok(Some(self.read(|st, d| dtext(st, d, "T"))?.unwrap_or_default()))
    }

    pub fn set_author(&mut self, author: &str) -> Result<()> {
        self.set("T", text_string(author))
    }

    pub fn contents(&self) -> Result<Option<String>> {
        Ok(Some(self.read(|st, d| dtext(st, d, "Contents"))?.unwrap_or_default()))
    }

    pub fn set_contents(&mut self, contents: &str) -> Result<()> {
        self.set("Contents", text_string(contents))
    }

    pub fn icon_name(&self) -> Result<Option<String>> {
        Ok(self
            .read(|st, d| dname(st, d, "Name"))?
            .map(|n| String::from_utf8_lossy(&n).into_owned()))
    }

    pub fn set_icon_name(&mut self, name: &str) -> Result<()> {
        self.set("Name", L::Name(name.as_bytes().to_vec()))
    }

    fn popup(&self) -> Result<Option<Object>> {
        self.obj.get_dict("Popup")
    }

    pub fn is_open(&self) -> Result<bool> {
        if let Some(p) = self.popup()?
            && let Some(o) = p.get_dict("Open")?
            && o.is_bool()?
        {
            return o.as_bool();
        }
        match self.obj.get_dict("Open")? {
            Some(o) if o.is_bool()? => o.as_bool(),
            _ => Ok(false),
        }
    }

    pub fn set_open(&mut self, open: bool) -> Result<()> {
        if let Some(p) = self.popup()? {
            p.dict_set(b"Open", L::Boolean(open))?;
        }
        self.obj.dict_set(b"Open", L::Boolean(open))
    }

    /// Crea (o recoloca) la ventana emergente de la anotación. `rect` en el espacio de página.
    pub fn set_popup(&mut self, rect: Rect) -> Result<()> {
        let (_, inv) = self.ctms()?;
        let user = rect.transform(&inv);
        if let Some(p) = self.popup()? {
            return p.dict_set(b"Rect", rect_array(user));
        }
        let shared = self.shared()?;
        let parent = self
            .obj
            .obj_id()
            .ok_or_else(|| pdf_err("anotación sin número de objeto"))?;
        let mut d = Dictionary::new();
        d.set("Type", L::Name(b"Annot".to_vec()));
        d.set("Subtype", L::Name(b"Popup".to_vec()));
        d.set("Parent", L::Reference(parent));
        d.set("Rect", rect_array(user));
        d.set("Open", L::Boolean(false));
        let id = shared.borrow_mut().add(L::Dictionary(d));
        push_annot(&self.page, id)?;
        self.obj.dict_set(b"Popup", L::Reference(id))
    }

    pub fn quadding(&self) -> Result<AnnotationTextAlign> {
        Ok(match self.read(|st, d| dfloat(st, d, "Q"))?.map(|q| q as i32) {
            Some(1) => AnnotationTextAlign::Center,
            Some(2) => AnnotationTextAlign::Right,
            _ => AnnotationTextAlign::Left,
        })
    }

    pub fn set_quadding(&mut self, align: AnnotationTextAlign) -> Result<()> {
        let q = match align {
            AnnotationTextAlign::Left => 0,
            AnnotationTextAlign::Center => 1,
            AnnotationTextAlign::Right => 2,
        };
        self.set("Q", L::Integer(q))
    }

    // --- Geometría -----------------------------------------------------------------------------------------------

    fn user_quads(&self) -> Result<Vec<Q>> {
        let flat = self.read(|st, d| dfloats(st, d, "QuadPoints"))?.unwrap_or_default();
        Ok(flat
            .chunks_exact(8)
            .map(|c| {
                [
                    Point::new(c[0], c[1]),
                    Point::new(c[2], c[3]),
                    Point::new(c[4], c[5]),
                    Point::new(c[6], c[7]),
                ]
            })
            .collect())
    }

    pub fn quad_points(&self) -> Result<Vec<Quad>> {
        let (m, _) = self.ctms()?;
        Ok(self
            .user_quads()?
            .into_iter()
            .map(|q| {
                Quad::new(
                    q[0].transform(&m),
                    q[1].transform(&m),
                    q[2].transform(&m),
                    q[3].transform(&m),
                )
            })
            .collect())
    }

    pub fn set_quad_points(&mut self, quads: &[Quad]) -> Result<()> {
        let (_, inv) = self.ctms()?;
        let user: Vec<Q> = quads
            .iter()
            .map(|q| {
                [
                    q.ul.transform(&inv),
                    q.ur.transform(&inv),
                    q.ll.transform(&inv),
                    q.lr.transform(&inv),
                ]
            })
            .collect();
        let flat: Vec<f32> = user.iter().flatten().flat_map(|p| [p.x, p.y]).collect();
        self.set("QuadPoints", reals(&flat))?;
        if !user.is_empty() {
            self.set_user_rect(ap::quads_bbox(&user))?;
        }
        Ok(())
    }

    fn user_line(&self) -> Result<(Point, Point)> {
        let l = self.read(|st, d| dfloats(st, d, "L"))?.unwrap_or_default();
        if l.len() < 4 {
            return Err(pdf_err("la línea no tiene /L"));
        }
        Ok((Point::new(l[0], l[1]), Point::new(l[2], l[3])))
    }

    pub fn line(&self) -> Result<(Point, Point)> {
        let (m, _) = self.ctms()?;
        let (a, b) = self.user_line()?;
        Ok((a.transform(&m), b.transform(&m)))
    }

    pub fn set_line(&mut self, start: Point, end: Point) -> Result<()> {
        let (_, inv) = self.ctms()?;
        let (a, b) = (start.transform(&inv), end.transform(&inv));
        self.set("L", reals(&[a.x, a.y, b.x, b.y]))?;
        self.set_user_rect(ap::poly_bbox(&[a, b]))
    }

    pub fn line_ending_styles(&self) -> Result<(LineEndingStyle, LineEndingStyle)> {
        let names = self.read(|st, d| {
            dget(st, d, "LE").and_then(|o| o.as_array().ok()).map(|a| {
                a.iter()
                    .map(|x| st.deref(x).and_then(|x| x.as_name().ok()).map(<[u8]>::to_vec))
                    .collect::<Vec<_>>()
            })
        })?;
        let get = |i: usize| {
            names
                .as_ref()
                .and_then(|n| n.get(i))
                .and_then(|n| n.as_deref())
                .map_or(LineEndingStyle::None, le_from)
        };
        Ok((get(0), get(1)))
    }

    pub fn set_line_ending_styles(&mut self, start: LineEndingStyle, end: LineEndingStyle) -> Result<()> {
        self.set(
            "LE",
            L::Array(vec![
                L::Name(le_name(start).as_bytes().to_vec()),
                L::Name(le_name(end).as_bytes().to_vec()),
            ]),
        )
    }

    fn user_vertices(&self) -> Result<Vec<Point>> {
        Ok(pts_of(
            &self.read(|st, d| dfloats(st, d, "Vertices"))?.unwrap_or_default(),
        ))
    }

    pub fn vertices(&self) -> Result<Vec<Point>> {
        let (m, _) = self.ctms()?;
        Ok(self.user_vertices()?.into_iter().map(|p| p.transform(&m)).collect())
    }

    pub fn set_vertices(&mut self, points: impl IntoIterator<Item = Point>) -> Result<()> {
        let (_, inv) = self.ctms()?;
        let user: Vec<Point> = points.into_iter().map(|p| p.transform(&inv)).collect();
        let flat: Vec<f32> = user.iter().flat_map(|p| [p.x, p.y]).collect();
        self.set("Vertices", reals(&flat))?;
        if !user.is_empty() {
            self.set_user_rect(ap::poly_bbox(&user))?;
        }
        Ok(())
    }

    fn user_ink(&self) -> Result<Vec<Vec<Point>>> {
        Ok(self
            .read(|st, d| {
                dget(st, d, "InkList").and_then(|o| o.as_array().ok()).map(|strokes| {
                    strokes
                        .iter()
                        .filter_map(|s| {
                            let a = st.deref(s)?.as_array().ok()?;
                            let flat: Option<Vec<f32>> = a.iter().map(|x| num(st, x)).collect();
                            Some(pts_of(&flat?))
                        })
                        .collect::<Vec<_>>()
                })
            })?
            .unwrap_or_default())
    }

    pub fn ink_list(&self) -> Result<Vec<Vec<Point>>> {
        let (m, _) = self.ctms()?;
        Ok(self
            .user_ink()?
            .into_iter()
            .map(|s| s.into_iter().map(|p| p.transform(&m)).collect())
            .collect())
    }

    pub fn set_ink_list<I, S>(&mut self, strokes: I) -> Result<()>
    where
        I: IntoIterator<Item = S>,
        S: IntoIterator<Item = Point>,
    {
        let (_, inv) = self.ctms()?;
        let user: Vec<Vec<Point>> = strokes
            .into_iter()
            .map(|s| s.into_iter().map(|p| p.transform(&inv)).collect())
            .collect();
        let arr = L::Array(
            user.iter()
                .map(|s| reals(&s.iter().flat_map(|p| [p.x, p.y]).collect::<Vec<_>>()))
                .collect(),
        );
        self.set("InkList", arr)?;
        let all: Vec<Point> = user.iter().flatten().copied().collect();
        if !all.is_empty() {
            self.set_user_rect(ap::poly_bbox(&all))?;
        }
        Ok(())
    }

    // --- Texto libre (/DA) -----------------------------------------------------------------------------------------

    pub fn default_appearance(&self) -> Result<Option<AnnotationDefaultAppearance>> {
        let da = self.read(|st, d| dtext(st, d, "DA"))?;
        Ok(da.and_then(|s| parse_da(&s)))
    }

    pub fn set_default_appearance(&mut self, font_name: &str, size: f32, color: Option<AnnotationColor>) -> Result<()> {
        let mut s = format!("/{font_name} {} Tf", ap::n(size));
        if let Some(c) = color {
            let ops = ap::color_op(&color_to_array(c), false);
            s.push(' ');
            s.push_str(ops.trim_end());
        }
        self.set("DA", L::String(s.into_bytes(), lopdf::StringFormat::Literal))
    }

    /// Aplicar una redacción es trabajo de W38 (reescritura de flujos de contenido): aquí no hay nada que aplicar.
    pub fn apply_redaction(self, _options: RedactOptions) -> Result<bool> {
        Err(Error::Unsupported(
            "aplicar redacciones con el modelo nativo llega con W38".into(),
        ))
    }

    // --- Apariencia --------------------------------------------------------------------------------------------------

    fn style(&self) -> Result<Style> {
        let width = self.border_width()?;
        let dash = if self.border_style()? == AnnotationBorderStyle::Dashed {
            let d = self.border_dash_pattern()?;
            Some(if d.is_empty() { vec![3.0] } else { d })
        } else {
            None
        };
        Ok(Style {
            color: self.color()?.map(color_to_array),
            interior: self.interior_color()?.map(color_to_array),
            width,
            dash,
            opacity: self.opacity()?,
        })
    }

    /// Regenera la apariencia si algo cambió desde la última vez. `true` si se ha escrito una apariencia nueva.
    pub fn update(&mut self) -> Result<bool> {
        let shared = self.shared()?;
        if let Some(id) = self.obj.obj_id()
            && !shared.borrow_mut().annot_dirty.remove(&id)
        {
            return Ok(false);
        }
        let Some(built) = self.build_appearance()? else {
            return Ok(false);
        };
        self.install(&shared, built)?;
        Ok(true)
    }

    fn has_appearance(&self) -> Result<bool> {
        Ok(match self.obj.get_dict("AP")? {
            Some(ap) => ap.get_dict("N")?.is_some(),
            None => false,
        })
    }

    fn build_appearance(&self) -> Result<Option<Ap>> {
        let ty = self.r#type()?;
        let style = self.style()?;
        let color = style.color.clone().unwrap_or_default();
        Ok(match ty {
            PdfAnnotationType::Highlight
            | PdfAnnotationType::Underline
            | PdfAnnotationType::StrikeOut
            | PdfAnnotationType::Squiggly => {
                let quads = self.user_quads()?;
                if quads.is_empty() {
                    return Ok(None);
                }
                let built = match ty {
                    PdfAnnotationType::Highlight => ap::highlight(&quads, &color, style.opacity),
                    PdfAnnotationType::Underline => {
                        ap::line_mark(ap::LineMark::Underline, &quads, &color, style.opacity)
                    }
                    PdfAnnotationType::StrikeOut => {
                        ap::line_mark(ap::LineMark::StrikeOut, &quads, &color, style.opacity)
                    }
                    _ => ap::line_mark(ap::LineMark::Squiggly, &quads, &color, style.opacity),
                };
                self.set_user_rect(built.bbox)?;
                Some(built)
            }
            PdfAnnotationType::Square | PdfAnnotationType::Circle => {
                let rect = self.user_rect()?;
                let old_rd = self
                    .read(|st, d| dfloats(st, d, "RD"))?
                    .filter(|v| v.len() == 4)
                    .unwrap_or_else(|| vec![0.0; 4]);
                let inner = Rect::new(
                    rect.x0 + old_rd[0],
                    rect.y0 + old_rd[1],
                    rect.x1 - old_rd[2],
                    rect.y1 - old_rd[3],
                );
                let rd = (style.width / 2.0).ceil().max(0.0);
                let outer = Rect::new(inner.x0 - rd, inner.y0 - rd, inner.x1 + rd, inner.y1 + rd);
                self.obj.dict_set(b"RD", reals(&[rd; 4]))?;
                self.set_user_rect(outer)?;
                Some(if ty == PdfAnnotationType::Square {
                    ap::square(inner, &style, outer)
                } else {
                    ap::circle(inner, &style, outer)
                })
            }
            PdfAnnotationType::Line => {
                let (a, b) = self.user_line()?;
                let (s, e) = self.line_ending_styles()?;
                let built = ap::line(a, b, (ending_of(s), ending_of(e)), &style);
                self.set_user_rect(built.bbox)?;
                Some(built)
            }
            PdfAnnotationType::Polygon | PdfAnnotationType::PolyLine => {
                let pts = self.user_vertices()?;
                if pts.len() < 2 {
                    return Ok(None);
                }
                let bbox = ap::expand(ap::poly_bbox(&pts), style.width.max(1.0));
                self.set_user_rect(bbox)?;
                Some(ap::poly(&pts, ty == PdfAnnotationType::Polygon, &style, bbox))
            }
            PdfAnnotationType::Ink => {
                let strokes = self.user_ink()?;
                if strokes.iter().all(Vec::is_empty) {
                    return Ok(None);
                }
                let built = ap::ink(&strokes, &style);
                self.set_user_rect(built.bbox)?;
                Some(built)
            }
            PdfAnnotationType::Text => {
                let name = self.icon_name()?.unwrap_or_else(|| "Note".into());
                Some(ap::note_icon(&name, &color, style.opacity))
            }
            PdfAnnotationType::Caret => Some(ap::caret(&color, style.opacity)),
            PdfAnnotationType::Redact => Some(ap::redact_mark(self.user_rect()?)),
            PdfAnnotationType::Stamp => {
                if self.has_appearance()? {
                    // Una apariencia propia (sello personalizado) se conserva: el visor la ajusta a /Rect.
                    return Ok(None);
                }
                let name = self.icon_name()?.unwrap_or_else(|| "Draft".into());
                Some(ap::stamp(self.user_rect()?, &name, &color, style.opacity))
            }
            PdfAnnotationType::FreeText => {
                let da = self.default_appearance()?;
                let (font_res, size, tcolor) = match da {
                    Some(d) => (d.font_name, d.size, d.color.map(color_to_array)),
                    None => ("Helv".to_string(), 12.0, None),
                };
                let base = super::metrics::base_font_for(&font_res);
                let text = self.contents()?.unwrap_or_default();
                let quadding = match self.quadding()? {
                    AnnotationTextAlign::Left => 0,
                    AnnotationTextAlign::Center => 1,
                    AnnotationTextAlign::Right => 2,
                };
                let mut fstyle = style.clone();
                fstyle.width = self
                    .read(|st, d| match dget(st, d, "BS") {
                        Some(L::Dictionary(bs)) => dfloat(st, bs, "W"),
                        _ => None,
                    })?
                    .unwrap_or(0.0);
                Some(ap::free_text(&ap::FreeText {
                    rect: self.user_rect()?,
                    text: &text,
                    font_res: &font_res,
                    base_font: base,
                    size,
                    text_color: tcolor.unwrap_or_else(|| vec![0.0]),
                    quadding,
                    style: &fstyle,
                }))
            }
            _ => None,
        })
    }

    /// Escribe `built` como XObject de formulario en `/AP /N` (reutiliza el flujo anterior si lo hay).
    fn install(&self, shared: &Shared, built: Ap) -> Result<()> {
        let mut dict = Dictionary::new();
        dict.set("Type", L::Name(b"XObject".to_vec()));
        dict.set("Subtype", L::Name(b"Form".to_vec()));
        dict.set("BBox", rect_array(built.bbox));
        let mut res = Dictionary::new();
        if let Some(gs) = built.gs {
            let mut g = Dictionary::new();
            g.set("Type", L::Name(b"ExtGState".to_vec()));
            if (gs.stroke - 1.0).abs() > 1e-4 {
                g.set("CA", L::Real(gs.stroke));
            }
            if (gs.fill - 1.0).abs() > 1e-4 {
                g.set("ca", L::Real(gs.fill));
            }
            if gs.multiply {
                g.set("BM", L::Name(b"Multiply".to_vec()));
            }
            let mut ext = Dictionary::new();
            ext.set("H", L::Dictionary(g));
            res.set("ExtGState", L::Dictionary(ext));
        }
        if !built.fonts.is_empty() {
            let mut fonts = Dictionary::new();
            for (name, base) in &built.fonts {
                let id = font_object(shared, base);
                fonts.set(name.as_str(), L::Reference(id));
            }
            res.set("Font", L::Dictionary(fonts));
        }
        if !res.is_empty() {
            dict.set("Resources", L::Dictionary(res));
        }
        let stream = Stream::new(dict, built.content.into_bytes());
        // ¿Ya hay un flujo en /AP /N? Se reutiliza su número de objeto.
        let existing = match self.obj.get_dict("AP")? {
            Some(a) => a
                .get_dict("N")?
                .and_then(|n| n.obj_id().or_else(|| n.as_indirect().ok().map(|i| (i as u32, 0)))),
            None => None,
        };
        let id = {
            let mut g = shared.borrow_mut();
            match existing
                .map(|id| g.resolve_id(id))
                .filter(|id| matches!(g.doc.objects.get(id), Some(L::Stream(_))))
            {
                Some(id) => {
                    g.doc.objects.insert(id, L::Stream(stream));
                    g.touch(id);
                    id
                }
                None => g.add(L::Stream(stream)),
            }
        };
        let mut ap_dict = Dictionary::new();
        ap_dict.set("N", L::Reference(id));
        self.obj.dict_set(b"AP", L::Dictionary(ap_dict))
    }
}

/// Fuente estándar sin incrustar (`/Type1 /WinAnsiEncoding`) para las apariencias.
fn font_object(shared: &Shared, base: &str) -> ObjectId {
    let mut f = Dictionary::new();
    f.set("Type", L::Name(b"Font".to_vec()));
    f.set("Subtype", L::Name(b"Type1".to_vec()));
    f.set("BaseFont", L::Name(base.as_bytes().to_vec()));
    if !base.starts_with("Symbol") && !base.starts_with("ZapfDingbats") {
        f.set("Encoding", L::Name(b"WinAnsiEncoding".to_vec()));
    }
    shared.borrow_mut().add(L::Dictionary(f))
}

/// `/Helv 12 Tf 0 g` → fuente, cuerpo y color.
pub(crate) fn parse_da(da: &str) -> Option<AnnotationDefaultAppearance> {
    let toks: Vec<&str> = da.split_whitespace().collect();
    let tf = toks.iter().position(|t| *t == "Tf")?;
    if tf < 2 {
        return None;
    }
    let font_name = toks[tf - 2].trim_start_matches('/').to_string();
    let size: f32 = toks[tf - 1].parse().ok()?;
    let mut color = None;
    for (i, t) in toks.iter().enumerate() {
        let take = |n: usize| -> Option<Vec<f32>> {
            if i < n {
                return None;
            }
            toks[i - n..i].iter().map(|x| x.parse().ok()).collect()
        };
        match *t {
            "g" => color = take(1).and_then(|c| color_from_array(&c)),
            "rg" => color = take(3).and_then(|c| color_from_array(&c)),
            "k" => color = take(4).and_then(|c| color_from_array(&c)),
            _ => {}
        }
    }
    Some(AnnotationDefaultAppearance { font_name, size, color })
}

// --- Página ----------------------------------------------------------------------------------------------------------

/// Añade una referencia a `/Annots` de la página (crea la matriz si falta).
fn push_annot(page: &Object, id: ObjectId) -> Result<()> {
    match page.get_dict("Annots")? {
        Some(arr) if arr.is_array()? => arr.modify(|o| {
            if let L::Array(a) = o {
                a.push(L::Reference(id));
            }
            Ok(())
        }),
        _ => page.dict_set(b"Annots", L::Array(vec![L::Reference(id)])),
    }
}

fn remove_annot(page: &Object, id: ObjectId) -> Result<()> {
    if let Some(arr) = page.get_dict("Annots")?
        && arr.is_array()?
    {
        arr.modify(|o| {
            if let L::Array(a) = o {
                a.retain(|x| *x != L::Reference(id));
            }
            Ok(())
        })?;
    }
    Ok(())
}

impl PdfPage {
    fn shared(&self) -> Result<Shared> {
        self.page
            .st
            .clone()
            .ok_or_else(|| pdf_err("la página no pertenece a ningún documento"))
    }

    /// Crea una anotación vacía del tipo dado (sin rectángulo ni contenido).
    pub fn create_annotation(&mut self, subtype: PdfAnnotationType) -> Result<PdfAnnotation> {
        let shared = self.shared()?;
        let mut d = Dictionary::new();
        d.set("Type", L::Name(b"Annot".to_vec()));
        d.set("Subtype", L::Name(subtype_name(subtype).as_bytes().to_vec()));
        if let Some(pid) = self.page.obj_id() {
            d.set("P", L::Reference(pid));
        }
        let (flags, color): (i64, Option<Vec<f32>>) = match subtype {
            PdfAnnotationType::Highlight => (4, Some(vec![1.0, 1.0, 0.0])),
            PdfAnnotationType::Underline => (4, Some(vec![0.0, 1.0, 0.0])),
            PdfAnnotationType::StrikeOut => (4, Some(vec![1.0, 0.0, 0.0])),
            PdfAnnotationType::Squiggly => (4, Some(vec![1.0, 0.0, 1.0])),
            PdfAnnotationType::Square
            | PdfAnnotationType::Circle
            | PdfAnnotationType::Line
            | PdfAnnotationType::Polygon
            | PdfAnnotationType::PolyLine
            | PdfAnnotationType::Ink
            | PdfAnnotationType::Stamp => (4, Some(vec![1.0, 0.0, 0.0])),
            PdfAnnotationType::Text => (28, Some(vec![1.0, 1.0, 0.0])),
            PdfAnnotationType::Caret => (4, Some(vec![0.0, 0.0, 1.0])),
            _ => (4, None),
        };
        d.set("F", L::Integer(flags));
        if let Some(c) = color {
            d.set("C", reals(&c));
        }
        let border = match subtype {
            PdfAnnotationType::Square
            | PdfAnnotationType::Circle
            | PdfAnnotationType::Line
            | PdfAnnotationType::Polygon
            | PdfAnnotationType::PolyLine
            | PdfAnnotationType::Ink => Some(1.0),
            PdfAnnotationType::FreeText => Some(0.0),
            _ => None,
        };
        if let Some(w) = border {
            let mut bs = Dictionary::new();
            bs.set("Type", L::Name(b"Border".to_vec()));
            bs.set("W", L::Real(w));
            d.set("BS", L::Dictionary(bs));
        }
        if subtype == PdfAnnotationType::FreeText {
            d.set(
                "DA",
                L::String(b"/Helv 12 Tf 0 g".to_vec(), lopdf::StringFormat::Literal),
            );
        }
        let id = {
            let mut g = shared.borrow_mut();
            let id = g.add(L::Dictionary(d));
            g.annot_dirty.insert(id);
            id
        };
        push_annot(&self.page, id)?;
        Ok(PdfAnnotation::new(Object::reference(&shared, id), self.page.clone()))
    }

    fn create_with_rect(&mut self, ty: PdfAnnotationType, rect: Rect) -> Result<PdfAnnotation> {
        let mut a = self.create_annotation(ty)?;
        a.set_rect(rect)?;
        Ok(a)
    }

    pub fn add_text_annotation(&mut self, rect: Rect, contents: &str) -> Result<PdfAnnotation> {
        let mut a = self.create_with_rect(PdfAnnotationType::Text, rect)?;
        a.set_contents(contents)?;
        // Ventana emergente a la derecha de la nota, como la crean los lectores habituales.
        a.set_popup(Rect::new(rect.x1, rect.y0, rect.x1 + 200.0, rect.y0 + 100.0))?;
        Ok(a)
    }

    pub fn add_free_text_annotation(&mut self, rect: Rect, contents: &str) -> Result<PdfAnnotation> {
        let mut a = self.create_with_rect(PdfAnnotationType::FreeText, rect)?;
        a.set_contents(contents)?;
        Ok(a)
    }

    pub fn add_caret_annotation(&mut self, rect: Rect) -> Result<PdfAnnotation> {
        self.create_with_rect(PdfAnnotationType::Caret, rect)
    }

    pub fn add_line_annotation(&mut self, start: Point, end: Point) -> Result<PdfAnnotation> {
        let mut a = self.create_annotation(PdfAnnotationType::Line)?;
        a.set_line(start, end)?;
        Ok(a)
    }

    pub fn add_square_annotation(&mut self, rect: Rect) -> Result<PdfAnnotation> {
        self.create_with_rect(PdfAnnotationType::Square, rect)
    }

    pub fn add_circle_annotation(&mut self, rect: Rect) -> Result<PdfAnnotation> {
        self.create_with_rect(PdfAnnotationType::Circle, rect)
    }

    pub fn add_polygon_annotation(&mut self, points: &[Point]) -> Result<PdfAnnotation> {
        let mut a = self.create_annotation(PdfAnnotationType::Polygon)?;
        a.set_vertices(points.iter().copied())?;
        Ok(a)
    }

    pub fn add_polyline_annotation(&mut self, points: &[Point]) -> Result<PdfAnnotation> {
        let mut a = self.create_annotation(PdfAnnotationType::PolyLine)?;
        a.set_vertices(points.iter().copied())?;
        Ok(a)
    }

    pub fn add_ink_annotation(&mut self, strokes: &[Vec<Point>]) -> Result<PdfAnnotation> {
        let mut a = self.create_annotation(PdfAnnotationType::Ink)?;
        a.set_ink_list(strokes.iter().map(|s| s.iter().copied()))?;
        Ok(a)
    }

    fn add_markup(&mut self, ty: PdfAnnotationType, quads: &[Quad]) -> Result<PdfAnnotation> {
        let mut a = self.create_annotation(ty)?;
        a.set_quad_points(quads)?;
        Ok(a)
    }

    pub fn add_highlight_annotation(&mut self, quads: &[Quad]) -> Result<PdfAnnotation> {
        self.add_markup(PdfAnnotationType::Highlight, quads)
    }

    pub fn add_underline_annotation(&mut self, quads: &[Quad]) -> Result<PdfAnnotation> {
        self.add_markup(PdfAnnotationType::Underline, quads)
    }

    pub fn add_squiggly_annotation(&mut self, quads: &[Quad]) -> Result<PdfAnnotation> {
        self.add_markup(PdfAnnotationType::Squiggly, quads)
    }

    pub fn add_strikeout_annotation(&mut self, quads: &[Quad]) -> Result<PdfAnnotation> {
        self.add_markup(PdfAnnotationType::StrikeOut, quads)
    }

    pub fn add_stamp_annotation(&mut self, rect: Rect, icon_name: &str) -> Result<PdfAnnotation> {
        let mut a = self.create_with_rect(PdfAnnotationType::Stamp, rect)?;
        a.set_icon_name(icon_name)?;
        Ok(a)
    }

    /// Marca un área para redactar (se aplica con [`PdfPage::redact`]).
    pub fn add_redact_annotation(&mut self, rect: Rect) -> Result<PdfAnnotation> {
        self.create_with_rect(PdfAnnotationType::Redact, rect)
    }

    pub fn add_redact_annotation_quads(&mut self, quads: &[Quad]) -> Result<PdfAnnotation> {
        self.add_markup(PdfAnnotationType::Redact, quads)
    }

    pub fn delete_annotation(&mut self, annot: PdfAnnotation) -> Result<()> {
        let id = annot
            .obj
            .obj_id()
            .ok_or_else(|| pdf_err("la anotación no es un objeto indirecto"))?;
        if let Some(p) = annot.popup()?
            && let Some(pid) = p.obj_id().or_else(|| p.as_indirect().ok().map(|i| (i as u32, 0)))
        {
            remove_annot(&self.page, pid)?;
        }
        remove_annot(&self.page, id)
    }

    /// Anotaciones de la página, sin enlaces, campos ni ventanas emergentes.
    pub fn annotations(&self) -> std::vec::IntoIter<PdfAnnotation> {
        let items = match self.page.get_dict("Annots") {
            Ok(Some(arr)) if arr.is_array().unwrap_or(false) => arr.array_iter().unwrap_or_default(),
            _ => Vec::new(),
        };
        items
            .into_iter()
            .map(|o| PdfAnnotation::new(o, self.page.clone()))
            .filter(|a| {
                !matches!(
                    a.r#type(),
                    Ok(PdfAnnotationType::Widget | PdfAnnotationType::Link | PdfAnnotationType::Popup) | Err(_)
                )
            })
            .collect::<Vec<_>>()
            .into_iter()
    }
}
