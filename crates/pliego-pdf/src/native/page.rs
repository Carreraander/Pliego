//! [`PdfPage`]: página de un PDF a nivel de objetos (contenido, recursos, cajas, giro, enlaces).
//!
//! No dibuja ni extrae texto: eso es de PDFium (W37). Las anotaciones están en `annot.rs`; los campos, en `widget.rs`.

use lopdf::Object as L;
use lopdf::{Dictionary, StringFormat};
use pliego_core::Result;

use super::document::PdfDocument;
use super::object::Object;
use super::pdf_err;
use crate::geom::{Matrix, Rect};
use crate::links::{DestinationKind, LinkTarget, NewLink};

/// Página de un PDF.
pub struct PdfPage {
    pub(crate) page: Object,
}

const LETTER: Rect = Rect::new(0.0, 0.0, 612.0, 792.0);

fn rect_of(o: &Object) -> Option<Rect> {
    if !o.is_array().ok()? || o.len().ok()? < 4 {
        return None;
    }
    let mut v = [0f32; 4];
    for (i, slot) in v.iter_mut().enumerate() {
        *slot = o.get_array(i as i32).ok()??.as_float().ok()?;
    }
    Some(Rect::new(
        v[0].min(v[2]),
        v[1].min(v[3]),
        v[0].max(v[2]),
        v[1].max(v[3]),
    ))
}

fn page_boxes(page: &Object) -> Result<(Rect, Rect)> {
    let media = page
        .get_dict_inheritable("MediaBox")?
        .and_then(|o| rect_of(&o))
        .filter(|r| !r.is_empty())
        .unwrap_or(LETTER);
    let crop = page
        .get_dict_inheritable("CropBox")?
        .and_then(|o| rect_of(&o))
        .filter(|r| !r.is_empty())
        .map(|c| media.intersect(&c))
        .filter(|r| !r.is_empty())
        .unwrap_or(media);
    Ok((media, crop))
}

fn page_rotation(page: &Object) -> Result<i32> {
    let r = match page.get_dict_inheritable("Rotate")? {
        Some(o) if o.is_number()? => o.as_int()?,
        _ => 0,
    };
    let r = if r < 0 { 360 - ((-r) % 360) } else { r % 360 };
    let r = 90 * ((r + 45) / 90);
    Ok(if r >= 360 { 0 } else { r })
}

fn user_unit(page: &Object) -> Result<f32> {
    Ok(match page.get_dict("UserUnit")? {
        Some(o) if o.is_real()? => o.as_float()?,
        _ => 1.0,
    })
}

/// `(a, b, c, d)` de rotar `rotate` grados en sentido horario y voltear (espacio de usuario → de página).
fn linear_part(rotate: i32, unit: f32) -> (f32, f32, f32, f32) {
    // fz_rotate(-rotate) seguida de scale(unit, -unit). Los giros son múltiplos de 90: sin error de redondeo.
    let (cos, sin) = match rotate {
        90 => (0.0, -1.0),
        180 => (-1.0, 0.0),
        270 => (0.0, 1.0),
        _ => (1.0, 0.0),
    };
    // rotate(-θ): x' = x cos - y sin; y' = x sin + y cos   (con sin ya del ángulo -θ)
    let (a, b, c, d) = (cos, sin, -sin, cos);
    // scale(unit, -unit) después: x'' = unit·x'; y'' = -unit·y'
    (a * unit + 0.0, -b * unit + 0.0, c * unit + 0.0, -d * unit + 0.0)
}

pub(crate) fn page_ctm(page: &Object) -> Result<Matrix> {
    let (_, crop) = page_boxes(page)?;
    let rotate = page_rotation(page)?;
    let unit = user_unit(page)?;
    let (a, b, c, d) = linear_part(rotate, unit);
    let tx = |x: f32, y: f32| (a * x + c * y, b * x + d * y);
    let corners = [
        tx(crop.x0, crop.y0),
        tx(crop.x1, crop.y0),
        tx(crop.x0, crop.y1),
        tx(crop.x1, crop.y1),
    ];
    let min_x = corners.iter().map(|p| p.0).fold(f32::INFINITY, f32::min);
    let min_y = corners.iter().map(|p| p.1).fold(f32::INFINITY, f32::min);
    Ok(Matrix::new(a, b, c, d, -min_x, -min_y))
}

impl PdfPage {
    pub(crate) fn new(page: Object) -> Self {
        Self { page }
    }

    /// Diccionario `/Page`.
    pub fn object(&self) -> Object {
        self.page.clone()
    }

    /// Valor de `/Contents` tal cual (un flujo, una matriz de flujos o `None`).
    pub fn contents(&self) -> Result<Option<Object>> {
        self.page.get_dict("Contents")
    }

    /// `/Resources` propio de la página; si solo los hereda, los copia a la página antes de devolverlos.
    pub fn resources(&self) -> Result<Object> {
        if let Some(r) = self.page.get_dict("Resources")?
            && r.is_dict()?
        {
            return Ok(r);
        }
        let mut page = self.page.clone();
        let inherited = match self.page.get_dict_inheritable("Resources")? {
            Some(r) if r.is_dict()? => r.copy_dict()?,
            _ => Object::free(self.page.st.clone(), L::Dictionary(Dictionary::new())),
        };
        page.dict_put("Resources", inherited)?;
        page.get_dict("Resources")?
            .ok_or_else(|| pdf_err("no se pudo crear /Resources"))
    }

    /// Giro de la página tal como está en el archivo (heredado si hace falta; sin normalizar, como MuPDF).
    pub fn rotation(&self) -> Result<i32> {
        match self.page.get_dict_inheritable("Rotate")? {
            Some(o) => o.as_int(),
            None => Ok(0),
        }
    }

    pub fn set_rotation(&mut self, rotate: i32) -> Result<()> {
        if rotate % 90 != 0 {
            return Err(pdf_err("el giro debe ser múltiplo de 90"));
        }
        self.page.dict_put_int("Rotate", rotate)
    }

    /// `/MediaBox` en el espacio de usuario, sin girar.
    pub fn media_box(&self) -> Result<Rect> {
        Ok(page_boxes(&self.page)?.0)
    }

    /// Recorte en el espacio «arriba a la izquierda» de la caja de medios sin girar (el de PyMuPDF/mupdf-rs: es
    /// el que entienden [`PdfPage::set_crop_box`] y los crates que ya lo usan).
    pub fn crop_box(&self) -> Result<Rect> {
        let bounds = self.bounds()?;
        let (x, y) = match self.page.get_dict_inheritable("CropBox")?.and_then(|o| rect_of(&o)) {
            Some(c) => (c.x0, c.y0),
            None => (0.0, 0.0),
        };
        let media_h = self.media_box()?.height();
        let y0 = media_h - y - bounds.height();
        Ok(Rect::new(x, y0, x + bounds.width(), y0 + bounds.height()))
    }

    /// Fija el recorte; `crop_box` está en el espacio de [`PdfPage::crop_box`] (origen arriba a la izquierda).
    pub fn set_crop_box(&mut self, crop_box: Rect) -> Result<()> {
        let media = self.media_box()?;
        let arr = L::Array(vec![
            L::Real(crop_box.x0),
            L::Real(media.y1 - crop_box.y1),
            L::Real(crop_box.x1),
            L::Real(media.y1 - crop_box.y0),
        ]);
        self.page.dict_set(b"CropBox", arr)
    }

    /// Matriz del espacio de usuario de la página al espacio de página (con giro y recorte aplicados).
    pub fn ctm(&self) -> Result<Matrix> {
        page_ctm(&self.page)
    }

    /// Límites de la página en el espacio de página (recorte y giro aplicados, origen en 0,0).
    pub fn bounds(&self) -> Result<Rect> {
        let (_, crop) = page_boxes(&self.page)?;
        Ok(crop.transform(&page_ctm(&self.page)?))
    }

    /// Añade un flujo de contenido a `/Contents`: al final (`overlay`) o al principio. Devuelve su número de objeto.
    pub fn insert_contents(&mut self, doc: &mut PdfDocument, bytes: &[u8], overlay: bool) -> Result<i32> {
        let stream = doc.add_stream(bytes, None, false)?;
        let new_ref = stream.embed_value(&self.page.st)?;
        let mut items: Vec<L> = match self.page.get_dict("Contents")? {
            None => Vec::new(),
            Some(c) => {
                if c.is_array()? {
                    c.read(|o| Ok(o.as_array().cloned().unwrap_or_default()))?
                } else if c.is_stream()? {
                    vec![c.embed_value(&self.page.st)?]
                } else {
                    Vec::new()
                }
            }
        };
        if overlay {
            items.push(new_ref);
        } else {
            items.insert(0, new_ref);
        }
        // Siempre una matriz directa propia de la página: la matriz anterior podía estar compartida.
        self.page.dict_set(b"Contents", L::Array(items))?;
        stream.as_indirect()
    }

    /// Envuelve el contenido existente en `q … Q` para que lo que se añada después parta de un estado gráfico limpio.
    pub fn wrap_contents(&mut self, doc: &mut PdfDocument) -> Result<()> {
        if self.page.get_dict("Contents")?.is_none() {
            return Ok(());
        }
        self.insert_contents(doc, b"q\n", false)?;
        self.insert_contents(doc, b"\nQ\n", true)?;
        Ok(())
    }

    /// Registra (o reutiliza) un `ExtGState` con opacidades y devuelve su nombre de recurso con barra
    /// (`"/GS0"`); `None` si ambas son `None`.
    pub fn register_ext_gstate(
        &mut self,
        doc: &mut PdfDocument,
        stroke_opacity: Option<f32>,
        fill_opacity: Option<f32>,
    ) -> Result<Option<String>> {
        if stroke_opacity.is_none() && fill_opacity.is_none() {
            return Ok(None);
        }
        let res = self.resources()?;
        let ext = match res.get_dict("ExtGState")? {
            Some(e) if e.is_dict()? => e,
            _ => {
                let res = res.clone();
                res.dict_set(b"ExtGState", L::Dictionary(Dictionary::new()))?;
                res.get_dict("ExtGState")?
                    .ok_or_else(|| pdf_err("no se pudo crear /ExtGState"))?
            }
        };
        let close = |a: Option<f32>, b: Option<f32>| match (a, b) {
            (None, None) => true,
            (Some(x), Some(y)) => (x - y).abs() < 1e-4,
            _ => false,
        };
        let mut next = 0usize;
        for (name, g) in ext.dict_iter()? {
            if let Some(n) = name.strip_prefix("GS").and_then(|n| n.parse::<usize>().ok()) {
                next = next.max(n + 1);
            }
            if !g.is_dict()? {
                continue;
            }
            let get = |k: &str| -> Result<Option<f32>> {
                Ok(match g.get_dict(k)? {
                    Some(v) if v.is_number()? => Some(v.as_float()?),
                    _ => None,
                })
            };
            if close(get("CA")?, stroke_opacity) && close(get("ca")?, fill_opacity) {
                return Ok(Some(format!("/{name}")));
            }
        }
        let mut gs = Dictionary::new();
        gs.set("Type", L::Name(b"ExtGState".to_vec()));
        if let Some(s) = stroke_opacity {
            gs.set("CA", L::Real(s));
        }
        if let Some(f) = fill_opacity {
            gs.set("ca", L::Real(f));
        }
        let id = doc.state().borrow_mut().add(L::Dictionary(gs));
        let name = format!("GS{next}");
        ext.dict_set(name.as_bytes(), L::Reference(id))?;
        Ok(Some(format!("/{name}")))
    }

    /// Regenera las apariencias de las anotaciones modificadas. `true` si alguna cambió.
    pub fn update(&mut self) -> Result<bool> {
        let mut changed = false;
        for mut a in self.annotations() {
            changed |= a.update()?;
        }
        Ok(changed)
    }

    /// Crea enlaces en la página (`bounds` en el espacio de página, como los que devuelve `Page::links`).
    pub fn add_links(&mut self, doc: &mut PdfDocument, links: &[NewLink]) -> Result<()> {
        let inv = self
            .ctm()?
            .invert()
            .ok_or_else(|| pdf_err("la matriz de la página no es invertible"))?;
        let page_ids = doc.page_ids();
        let mut refs = Vec::new();
        for link in links {
            let r = link.bounds.transform(&inv);
            let mut d = Dictionary::new();
            d.set("Type", L::Name(b"Annot".to_vec()));
            d.set("Subtype", L::Name(b"Link".to_vec()));
            d.set(
                "Rect",
                L::Array(vec![L::Real(r.x0), L::Real(r.y0), L::Real(r.x1), L::Real(r.y1)]),
            );
            d.set("Border", L::Array(vec![L::Integer(0), L::Integer(0), L::Integer(0)]));
            let mut action = Dictionary::new();
            match &link.target {
                LinkTarget::Uri(u) => {
                    action.set("S", L::Name(b"URI".to_vec()));
                    action.set("URI", L::String(u.as_bytes().to_vec(), StringFormat::Literal));
                }
                LinkTarget::Page { page, kind } => {
                    let pid = *page_ids
                        .get(*page as usize)
                        .ok_or_else(|| pdf_err(format!("la página {} no existe", page + 1)))?;
                    action.set("S", L::Name(b"GoTo".to_vec()));
                    action.set("D", L::Array(dest_array(pid, *kind)));
                }
            }
            d.set("A", L::Dictionary(action));
            if let Some(pid) = self.page.obj_id() {
                d.set("P", L::Reference(pid));
            }
            let id = doc.state().borrow_mut().add(L::Dictionary(d));
            refs.push(L::Reference(id));
        }
        let mut annots: Vec<L> = match self.page.get_dict("Annots")? {
            Some(a) if a.is_array()? => a.read(|o| Ok(o.as_array().cloned().unwrap_or_default()))?,
            _ => Vec::new(),
        };
        annots.extend(refs);
        // Si /Annots era una referencia a una matriz, se modifica esa matriz en su sitio.
        match self.page.get_dict("Annots")? {
            Some(a) if a.indirect => a.modify(|o| {
                *o = L::Array(annots);
                Ok(())
            }),
            _ => self.page.dict_set(b"Annots", L::Array(annots)),
        }
    }
}

/// Destino explícito `[página /Vista …]`.
pub(crate) fn dest_array(page: lopdf::ObjectId, kind: DestinationKind) -> Vec<L> {
    let num = |v: Option<f32>| v.map_or(L::Null, L::Real);
    let mut a = vec![L::Reference(page)];
    match kind {
        DestinationKind::Fit => a.push(L::Name(b"Fit".to_vec())),
        DestinationKind::FitH { top } => {
            a.push(L::Name(b"FitH".to_vec()));
            a.push(num(top));
        }
        DestinationKind::FitV { left } => {
            a.push(L::Name(b"FitV".to_vec()));
            a.push(num(left));
        }
        DestinationKind::XYZ { left, top, zoom } => {
            a.push(L::Name(b"XYZ".to_vec()));
            a.push(num(left));
            a.push(num(top));
            a.push(num(zoom));
        }
        DestinationKind::FitR {
            left,
            bottom,
            right,
            top,
        } => {
            a.push(L::Name(b"FitR".to_vec()));
            a.extend([L::Real(left), L::Real(bottom), L::Real(right), L::Real(top)]);
        }
        DestinationKind::FitB => a.push(L::Name(b"FitB".to_vec())),
        DestinationKind::FitBH { top } => {
            a.push(L::Name(b"FitBH".to_vec()));
            a.push(num(top));
        }
        DestinationKind::FitBV { left } => {
            a.push(L::Name(b"FitBV".to_vec()));
            a.push(num(left));
        }
    }
    a
}

/// Vista de un destino explícito (el resto del array tras la página).
pub(crate) fn dest_kind(items: &[L]) -> DestinationKind {
    let num = |i: usize| items.get(i).and_then(|o| o.as_float().ok());
    match items.get(1).and_then(|o| o.as_name().ok()) {
        Some(b"FitH") => DestinationKind::FitH { top: num(2) },
        Some(b"FitV") => DestinationKind::FitV { left: num(2) },
        Some(b"XYZ") => DestinationKind::XYZ {
            left: num(2),
            top: num(3),
            zoom: num(4),
        },
        Some(b"FitR") => DestinationKind::FitR {
            left: num(2).unwrap_or(0.0),
            bottom: num(3).unwrap_or(0.0),
            right: num(4).unwrap_or(0.0),
            top: num(5).unwrap_or(0.0),
        },
        Some(b"FitB") => DestinationKind::FitB,
        Some(b"FitBH") => DestinationKind::FitBH { top: num(2) },
        Some(b"FitBV") => DestinationKind::FitBV { left: num(2) },
        _ => DestinationKind::Fit,
    }
}
