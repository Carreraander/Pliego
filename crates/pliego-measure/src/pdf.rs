//! Lectura y escritura de `Measure` en el PDF.

use mupdf::Point;
use mupdf::pdf::{PdfDocument, PdfObject, PdfPage};
use pliego_core::{Error, Result};
use serde::{Deserialize, Serialize};

use crate::{Measurement, Pt, Scale, Shape, Unit, measure};

fn pe(e: mupdf::Error) -> Error {
    Error::Pdf(e.to_string())
}

fn page_of(doc: &PdfDocument, page: u32) -> Result<PdfPage> {
    let n = doc.page_count().map_err(pe)?;
    if page as i32 >= n {
        return Err(Error::InvalidInput(format!(
            "la página {} no existe (el documento tiene {n})",
            page + 1
        )));
    }
    doc.load_pdf_page(page as i32).map_err(pe)
}

fn number_format(doc: &mut PdfDocument, unit: &str, factor: f64) -> Result<PdfObject> {
    let mut d = doc.new_dict().map_err(pe)?;
    d.dict_put("Type", PdfObject::new_name("NumberFormat").map_err(pe)?)
        .map_err(pe)?;
    d.dict_put("U", PdfObject::new_string(unit).map_err(pe)?).map_err(pe)?;
    d.dict_put("C", PdfObject::new_real(factor as f32).map_err(pe)?)
        .map_err(pe)?;
    d.dict_put("F", PdfObject::new_name("D").map_err(pe)?).map_err(pe)?;
    d.dict_put("D", PdfObject::new_int(100).map_err(pe)?).map_err(pe)?;
    let mut arr = doc.new_array().map_err(pe)?;
    arr.array_push(d).map_err(pe)?;
    Ok(arr)
}

/// Diccionario `Measure` de tipo `RL` (medidas rectilíneas) para esta escala.
fn measure_dict(doc: &mut PdfDocument, scale: &Scale) -> Result<PdfObject> {
    let unit = scale.unit.label();
    let k = scale.units_per_point;
    let mut m = doc.new_dict().map_err(pe)?;
    m.dict_put("Type", PdfObject::new_name("Measure").map_err(pe)?)
        .map_err(pe)?;
    m.dict_put("Subtype", PdfObject::new_name("RL").map_err(pe)?)
        .map_err(pe)?;
    m.dict_put("R", PdfObject::new_string(&scale.ratio_text()).map_err(pe)?)
        .map_err(pe)?;
    let x = number_format(doc, unit, k)?;
    m.dict_put("X", x).map_err(pe)?;
    let d = number_format(doc, unit, k)?;
    m.dict_put("D", d).map_err(pe)?;
    let a = number_format(doc, &format!("sq {unit}"), k * k)?;
    m.dict_put("A", a).map_err(pe)?;
    Ok(m)
}

/// Escala guardada en un diccionario `Measure` (la toma del formato del eje X).
fn scale_of(measure: &PdfObject) -> Result<Option<Scale>> {
    let Some(x) = measure.get_dict("X").map_err(pe)? else {
        return Ok(None);
    };
    let Some(nf) = x.get_array(0).map_err(pe)? else {
        return Ok(None);
    };
    let (Some(u), Some(c)) = (nf.get_dict("U").map_err(pe)?, nf.get_dict("C").map_err(pe)?) else {
        return Ok(None);
    };
    let unit: Unit = u.as_string().map_err(pe)?.parse()?;
    Ok(Some(Scale {
        unit,
        units_per_point: f64::from(c.as_float().map_err(pe)?),
    }))
}

/// Fija la escala de una página (viewport con `Measure` sobre toda la caja de medios). Sustituye los viewports
/// que tuviera la página.
pub fn set_scale(doc: &mut PdfDocument, page: u32, scale: &Scale) -> Result<()> {
    let pg = page_of(doc, page)?;
    let mut obj = pg.object();
    let media = obj
        .get_dict_inheritable("MediaBox")
        .map_err(pe)?
        .ok_or_else(|| Error::Pdf("la página no tiene MediaBox".into()))?;
    let mut bbox = doc.new_array().map_err(pe)?;
    for i in 0..4 {
        let v = media
            .get_array(i)
            .map_err(pe)?
            .ok_or_else(|| Error::Pdf("MediaBox no válida".into()))?
            .as_float()
            .map_err(pe)?;
        bbox.array_push(PdfObject::new_real(v).map_err(pe)?).map_err(pe)?;
    }
    let m = measure_dict(doc, scale)?;
    let mut vp = doc.new_dict().map_err(pe)?;
    vp.dict_put("Type", PdfObject::new_name("Viewport").map_err(pe)?)
        .map_err(pe)?;
    vp.dict_put("BBox", bbox).map_err(pe)?;
    vp.dict_put("Name", PdfObject::new_string("Pliego").map_err(pe)?)
        .map_err(pe)?;
    vp.dict_put("Measure", m).map_err(pe)?;
    let mut arr = doc.new_array().map_err(pe)?;
    arr.array_push(vp).map_err(pe)?;
    obj.dict_put("VP", arr).map_err(pe)
}

/// Escala de una página, si tiene una.
pub fn page_scale(doc: &PdfDocument, page: u32) -> Result<Option<Scale>> {
    let pg = page_of(doc, page)?;
    let Some(vps) = pg.object().get_dict("VP").map_err(pe)? else {
        return Ok(None);
    };
    for i in 0..vps.len().map_err(pe)? {
        if let Some(vp) = vps.get_array(i as i32).map_err(pe)?
            && let Some(m) = vp.get_dict("Measure").map_err(pe)?
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
    let m = measure(scale, points, closed)?;
    let mut pg = page_of(doc, page)?;
    let pts: Vec<Point> = points.iter().map(|p| Point::new(p[0], p[1])).collect();
    let (mut annot, it, subj) = match m.shape {
        Shape::Line => (
            pg.add_line_annotation(pts[0], pts[1]).map_err(pe)?,
            "LineDimension",
            "Distancia",
        ),
        Shape::Polyline => (
            pg.add_polyline_annotation(pts).map_err(pe)?,
            "PolyLineDimension",
            "Longitud",
        ),
        Shape::Polygon => (pg.add_polygon_annotation(pts).map_err(pe)?, "PolygonDimension", "Área"),
    };
    let mut obj = annot.object();
    let md = measure_dict(doc, scale)?;
    obj.dict_put("Measure", md).map_err(pe)?;
    obj.dict_put("IT", PdfObject::new_name(it).map_err(pe)?).map_err(pe)?;
    obj.dict_put("Subj", PdfObject::new_string(subj).map_err(pe)?)
        .map_err(pe)?;
    annot.set_contents(&m.text).map_err(pe)?;
    annot.update().map_err(pe)?;
    let xref = annot.xref().map_err(pe)?;
    Ok((m, xref))
}

/// Una anotación de medida ya existente en el documento.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeasuredAnnot {
    pub page: u32,
    pub xref: i32,
    pub measurement: Measurement,
}

fn numbers(arr: &PdfObject) -> Result<Vec<f32>> {
    let mut v = Vec::new();
    for i in 0..arr.len().map_err(pe)? {
        if let Some(x) = arr.get_array(i as i32).map_err(pe)? {
            v.push(x.as_float().map_err(pe)?);
        }
    }
    Ok(v)
}

/// Lista las anotaciones de medida de una página (`Some`) o de todo el documento, medidas con la escala de
/// cada anotación (no la de la página).
pub fn list_measurements(doc: &PdfDocument, page: Option<u32>) -> Result<Vec<MeasuredAnnot>> {
    let n = doc.page_count().map_err(pe)?.max(0) as u32;
    let range = match page {
        Some(p) => p..p + 1,
        None => 0..n,
    };
    let mut out = Vec::new();
    for p in range {
        let pg = page_of(doc, p)?;
        for a in pg.annotations() {
            let obj = a.object();
            let Some(md) = obj.get_dict("Measure").map_err(pe)? else {
                continue;
            };
            let Some(scale) = scale_of(&md)? else { continue };
            let subtype = obj
                .get_dict("Subtype")
                .map_err(pe)?
                .and_then(|s| s.as_name().ok())
                .map(|b| String::from_utf8_lossy(&b).into_owned())
                .unwrap_or_default();
            let key = if subtype == "Line" { "L" } else { "Vertices" };
            let Some(arr) = obj.get_dict(key).map_err(pe)? else {
                continue;
            };
            let flat = numbers(&arr)?;
            let pts: Vec<Pt> = flat.chunks_exact(2).map(|c| [c[0], c[1]]).collect();
            let measurement = measure(&scale, &pts, subtype == "Polygon")?;
            out.push(MeasuredAnnot {
                page: p,
                xref: a.xref().map_err(pe)?,
                measurement,
            });
        }
    }
    Ok(out)
}
