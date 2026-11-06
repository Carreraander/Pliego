//! Lectura y escritura de `Measure` en el PDF.

use pliego_core::{Error, Lang, Result};
use pliego_pdf::{Object, PdfDocument, PdfPage, Point};
use serde::{Deserialize, Serialize};

use crate::{Measurement, Pt, Scale, Shape, Unit, measure, measure_in};

fn page_of(doc: &PdfDocument, page: u32) -> Result<PdfPage> {
    let n = doc.page_count()?;
    if page as i32 >= n {
        return Err(Error::InvalidInput(format!(
            "la página {} no existe (el documento tiene {n})",
            page + 1
        )));
    }
    doc.load_pdf_page(page as i32)
}

fn number_format(doc: &mut PdfDocument, unit: &str, factor: f64) -> Result<Object> {
    let mut d = doc.new_dict()?;
    d.dict_put("Type", Object::new_name("NumberFormat")?)?;
    d.dict_put("U", Object::new_string(unit)?)?;
    d.dict_put("C", Object::new_real(factor as f32)?)?;
    d.dict_put("F", Object::new_name("D")?)?;
    d.dict_put("D", Object::new_int(100)?)?;
    let mut arr = doc.new_array()?;
    arr.array_push(d)?;
    Ok(arr)
}

/// Diccionario `Measure` de tipo `RL` (medidas rectilíneas) para esta escala.
fn measure_dict(doc: &mut PdfDocument, scale: &Scale) -> Result<Object> {
    let unit = scale.unit.label();
    let k = scale.units_per_point;
    let mut m = doc.new_dict()?;
    m.dict_put("Type", Object::new_name("Measure")?)?;
    m.dict_put("Subtype", Object::new_name("RL")?)?;
    m.dict_put("R", Object::new_string(&scale.ratio_text())?)?;
    let x = number_format(doc, unit, k)?;
    m.dict_put("X", x)?;
    let d = number_format(doc, unit, k)?;
    m.dict_put("D", d)?;
    let a = number_format(doc, &format!("sq {unit}"), k * k)?;
    m.dict_put("A", a)?;
    Ok(m)
}

/// Escala guardada en un diccionario `Measure` (la toma del formato del eje X).
fn scale_of(measure: &Object) -> Result<Option<Scale>> {
    let Some(x) = measure.get_dict("X")? else {
        return Ok(None);
    };
    let Some(nf) = x.get_array(0)? else {
        return Ok(None);
    };
    let (Some(u), Some(c)) = (nf.get_dict("U")?, nf.get_dict("C")?) else {
        return Ok(None);
    };
    let unit: Unit = u.as_string()?.parse()?;
    Ok(Some(Scale {
        unit,
        units_per_point: f64::from(c.as_float()?),
    }))
}

/// Fija la escala de una página (viewport con `Measure` sobre toda la caja de medios). Sustituye los viewports
/// que tuviera la página.
pub fn set_scale(doc: &mut PdfDocument, page: u32, scale: &Scale) -> Result<()> {
    let pg = page_of(doc, page)?;
    let mut obj = pg.object();
    let media = obj
        .get_dict_inheritable("MediaBox")?
        .ok_or_else(|| Error::Pdf("la página no tiene MediaBox".into()))?;
    let mut bbox = doc.new_array()?;
    for i in 0..4 {
        let v = media
            .get_array(i)?
            .ok_or_else(|| Error::Pdf("MediaBox no válida".into()))?
            .as_float()?;
        bbox.array_push(Object::new_real(v)?)?;
    }
    let m = measure_dict(doc, scale)?;
    let mut vp = doc.new_dict()?;
    vp.dict_put("Type", Object::new_name("Viewport")?)?;
    vp.dict_put("BBox", bbox)?;
    vp.dict_put("Name", Object::new_string("Pliego")?)?;
    vp.dict_put("Measure", m)?;
    let mut arr = doc.new_array()?;
    arr.array_push(vp)?;
    obj.dict_put("VP", arr)
}

/// Escala de una página, si tiene una.
pub fn page_scale(doc: &PdfDocument, page: u32) -> Result<Option<Scale>> {
    let pg = page_of(doc, page)?;
    let Some(vps) = pg.object().get_dict("VP")? else {
        return Ok(None);
    };
    for i in 0..vps.len()? {
        if let Some(vp) = vps.get_array(i as i32)?
            && let Some(m) = vp.get_dict("Measure")?
            && let Some(s) = scale_of(&m)?
        {
            return Ok(Some(s));
        }
    }
    Ok(None)
}

/// Mide y añade una anotación de medida: `Line` (2 puntos), `PolyLine` (más, abierta) o `Polygon` (`closed`).
/// Devuelve la medida y el número de objeto (xref) de la anotación.
pub fn add_measurement(
    doc: &mut PdfDocument,
    page: u32,
    scale: &Scale,
    points: &[Pt],
    closed: bool,
) -> Result<(Measurement, i32)> {
    add_measurement_in(doc, page, scale, points, closed, Lang::Es)
}

/// Como [`add_measurement`], con el asunto y el texto de la anotación en `lang` (el idioma de la interfaz).
pub fn add_measurement_in(
    doc: &mut PdfDocument,
    page: u32,
    scale: &Scale,
    points: &[Pt],
    closed: bool,
    lang: Lang,
) -> Result<(Measurement, i32)> {
    let m = measure_in(scale, points, closed, lang)?;
    let mut pg = page_of(doc, page)?;
    let pts: Vec<Point> = points.iter().map(|p| Point::new(p[0], p[1])).collect();
    let (mut annot, it, subj) = match m.shape {
        Shape::Line => (
            pg.add_line_annotation(pts[0], pts[1])?,
            "LineDimension",
            lang.pick("Distancia", "Distance"),
        ),
        Shape::Polyline => (
            pg.add_polyline_annotation(&pts)?,
            "PolyLineDimension",
            lang.pick("Longitud", "Length"),
        ),
        Shape::Polygon => (
            pg.add_polygon_annotation(&pts)?,
            "PolygonDimension",
            lang.pick("Área", "Area"),
        ),
    };
    let mut obj = annot.object();
    let md = measure_dict(doc, scale)?;
    obj.dict_put("Measure", md)?;
    obj.dict_put("IT", Object::new_name(it)?)?;
    obj.dict_put("Subj", Object::new_string(subj)?)?;
    annot.set_contents(&m.text)?;
    annot.update()?;
    let xref = annot.xref()?;
    Ok((m, xref))
}

/// Una anotación de medida ya existente en el documento.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeasuredAnnot {
    pub page: u32,
    pub xref: i32,
    pub measurement: Measurement,
}

fn numbers(arr: &Object) -> Result<Vec<f32>> {
    let mut v = Vec::new();
    for i in 0..arr.len()? {
        if let Some(x) = arr.get_array(i as i32)? {
            v.push(x.as_float()?);
        }
    }
    Ok(v)
}

/// Lista las anotaciones de medida de una página (`Some`) o de todo el documento, medidas con la escala de
/// cada anotación (no la de la página).
pub fn list_measurements(doc: &PdfDocument, page: Option<u32>) -> Result<Vec<MeasuredAnnot>> {
    let n = doc.page_count()?.max(0) as u32;
    let range = match page {
        Some(p) => p..p + 1,
        None => 0..n,
    };
    let mut out = Vec::new();
    for p in range {
        let pg = page_of(doc, p)?;
        for a in pg.annotations() {
            let obj = a.object();
            let Some(md) = obj.get_dict("Measure")? else {
                continue;
            };
            let Some(scale) = scale_of(&md)? else { continue };
            let subtype = obj
                .get_dict("Subtype")?
                .and_then(|s| s.as_name().ok())
                .map(|b| String::from_utf8_lossy(&b).into_owned())
                .unwrap_or_default();
            let key = if subtype == "Line" { "L" } else { "Vertices" };
            let Some(arr) = obj.get_dict(key)? else {
                continue;
            };
            let flat = numbers(&arr)?;
            let mut pts: Vec<Pt> = Vec::with_capacity(flat.len() / 2);
            let mut it = flat.into_iter();
            while let (Some(x), Some(y)) = (it.next(), it.next()) {
                pts.push([x, y]);
            }
            let measurement = measure(&scale, &pts, subtype == "Polygon")?;
            out.push(MeasuredAnnot {
                page: p,
                xref: a.xref()?,
                measurement,
            });
        }
    }
    Ok(out)
}
