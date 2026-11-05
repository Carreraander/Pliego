//! Imágenes: listar, insertar, reemplazar, mover/redimensionar/girar, recortar y borrar.
//!
//! Las imágenes de una página se identifican por su orden de aparición (`id`) entre los operadores `Do` del
//! flujo de contenido. El `id` solo vale mientras no cambie el contenido de la página.

use pliego_core::{Error, Report, Result};
use pliego_document::Document;
use pliego_pdf::{Image, ImageSource, InsertImageOptions, Matrix, Object, PdfDocument, PdfPage, Rect};
use serde::{Deserialize, Serialize};

use crate::content::{Placement, parse, placements};
use crate::load_page;
use crate::types::RectPts;

/// Imagen colocada en una página.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageObject {
    pub page: usize,
    pub id: usize,
    /// Nombre del recurso en la página.
    pub name: String,
    /// Número de objeto PDF de la imagen (0 si no se pudo resolver).
    pub xref: i32,
    /// Recuadro en el espacio MuPDF (origen arriba a la izquierda).
    pub bbox: RectPts,
}

/// Transformación de un objeto, en el espacio MuPDF. El giro es en sentido horario y se aplica
/// alrededor del centro del objeto; el orden es escalar, girar y mover.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Transform {
    pub dx: f32,
    pub dy: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub rotate_degrees: f32,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            dx: 0.0,
            dy: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            rotate_degrees: 0.0,
        }
    }
}

pub(crate) fn num(v: f32) -> String {
    let s = format!("{v:.5}");
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

pub(crate) fn matrix_str(m: &Matrix) -> String {
    format!(
        "{} {} {} {} {} {}",
        num(m.a),
        num(m.b),
        num(m.c),
        num(m.d),
        num(m.e),
        num(m.f)
    )
}

pub(crate) fn mul(a: &Matrix, b: &Matrix) -> Matrix {
    let mut m = *a;
    m.concat(*b);
    m
}

pub(crate) fn inv(m: &Matrix) -> Result<Matrix> {
    m.invert()
        .ok_or_else(|| Error::Pdf("la imagen tiene una matriz no invertible (tamaño cero)".into()))
}

/// Matriz `M` para insertar (`M cm`) justo antes de dibujar un objeto con matriz vigente `c`, de modo que
/// el objeto sufra la transformación `t` (dada en el espacio MuPDF; `p` lleva usuario -> MuPDF). `center`
/// es el centro del objeto en el espacio MuPDF.
pub(crate) fn local_transform(c: &Matrix, p: &Matrix, t: &Transform, center: (f32, f32)) -> Result<Matrix> {
    let (cx, cy) = center;
    // T en espacio MuPDF: alrededor del centro, escalar, girar y mover.
    let mut tf = Matrix::new_translate(-cx, -cy);
    tf.concat(Matrix::new_scale(t.scale_x, t.scale_y));
    tf.concat(Matrix::new_rotate(t.rotate_degrees));
    tf.concat(Matrix::new_translate(cx + t.dx, cy + t.dy));
    // Al espacio de usuario (P·T·P⁻¹) y de ahí al espacio local del objeto (C·T·C⁻¹).
    let t_user = mul(&mul(p, &tf), &inv(p)?);
    Ok(mul(&mul(c, &t_user), &inv(c)?))
}

pub(crate) fn valid_transform(t: &Transform) -> Result<()> {
    if t.scale_x == 0.0 || t.scale_y == 0.0 || !(t.dx + t.dy + t.scale_x + t.scale_y + t.rotate_degrees).is_finite() {
        return Err(Error::InvalidInput("la transformación no es válida".into()));
    }
    Ok(())
}

pub(crate) fn bbox_of(m: &Matrix) -> RectPts {
    let pts = [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)].map(|(x, y)| m.transform_xy(x, y));
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for (x, y) in pts {
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    }
    [x0, y0, x1, y1]
}

/// Recursos `XObject` visibles desde la página (con herencia).
fn xobject(page: &PdfPage) -> Option<Object> {
    let res = page.object().get_dict_inheritable("Resources").ok()??;
    let x = res.get_dict("XObject").ok()??;
    x.is_dict().ok()?.then_some(x)
}

/// Contenido de la página (todos los flujos, decodificados y unidos).
pub(crate) fn page_bytes(page: &PdfPage) -> Result<Vec<u8>> {
    let Some(c) = page.contents()? else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    if c.is_array()? {
        for i in 0..c.len()? {
            if let Some(s) = c.get_array(i as i32)? {
                out.extend(s.read_stream()?);
                out.push(b'\n');
            }
        }
    } else if c.is_stream()? {
        out = c.read_stream()?;
    }
    Ok(out)
}

struct Loaded {
    page: PdfPage,
    bytes: Vec<u8>,
    list: Vec<Placement>,
    /// Matriz espacio de usuario -> espacio MuPDF.
    p: Matrix,
}

fn load(pdf: &PdfDocument, page: usize) -> Result<Loaded> {
    let pg = load_page(pdf, page)?;
    let bytes = page_bytes(&pg)?;
    let xo = xobject(&pg);
    let is_image = |name: &str| -> bool {
        xo.as_ref()
            .and_then(|x| x.get_dict(name).ok().flatten())
            .and_then(|o| o.get_dict("Subtype").ok().flatten())
            .and_then(|s| s.as_name().ok())
            .is_some_and(|n| n == b"Image")
    };
    let list = placements(&parse(&bytes), is_image);
    let p = pg.object().page_ctm()?;
    Ok(Loaded {
        page: pg,
        bytes,
        list,
        p,
    })
}

fn xref_of(page: &PdfPage, name: &str) -> i32 {
    xobject(page)
        .and_then(|x| x.get_dict(name).ok().flatten())
        .and_then(|o| o.as_indirect().ok())
        .unwrap_or(0)
}

/// Cajas de las imágenes de la página (espacio MuPDF), para la recolocación del texto.
pub(crate) fn image_boxes(pdf: &PdfDocument, page: usize) -> Result<Vec<RectPts>> {
    let l = load(pdf, page)?;
    Ok(l.list.iter().map(|pl| bbox_of(&mul(&pl.ctm, &l.p))).collect())
}

/// Imágenes colocadas en la página (desde 0), en orden de dibujo.
pub fn images(doc: &Document, page: usize) -> Result<Vec<ImageObject>> {
    let l = load(doc.pdf(), page)?;
    Ok(l.list
        .iter()
        .enumerate()
        .map(|(id, pl)| ImageObject {
            page,
            id,
            name: pl.name.clone(),
            xref: xref_of(&l.page, &pl.name),
            bbox: bbox_of(&mul(&pl.ctm, &l.p)),
        })
        .collect())
}

/// Inserta una imagen (PNG, JPEG, ...) ocupando `rect` (espacio MuPDF) encima del contenido.
pub fn insert_image(doc: &mut Document, page: usize, rect: RectPts, data: &[u8]) -> Result<Report> {
    if rect[2] <= rect[0] || rect[3] <= rect[1] {
        return Err(Error::InvalidInput("el recuadro de la imagen está vacío".into()));
    }
    Image::from_bytes(data).map_err(|_| Error::InvalidInput("el archivo no es una imagen que se pueda leer".into()))?;
    let mut report = Report::default();
    doc.with_pdf_mut(|pdf| {
        let mut pg = load_page(pdf, page)?;
        let p = pg.object().page_ctm()?;
        let ip = inv(&p)?;
        // Esquinas del recuadro al espacio de usuario; se normaliza para que la imagen salga derecha.
        let (ax, ay) = ip.transform_xy(rect[0], rect[1]);
        let (bx, by) = ip.transform_xy(rect[2], rect[3]);
        let r = Rect::new(ax.min(bx), ay.min(by), ax.max(bx), ay.max(by));
        if pg.rotation()? != 0 {
            report.warnings.push(
                "la página está girada: la imagen se coloca según el espacio del PDF y puede verse girada".into(),
            );
        }
        pg.insert_image(
            pdf,
            r,
            ImageSource::Bytes {
                data,
                format_hint: None,
            },
            InsertImageOptions::default(),
        )?;
        Ok(())
    })?;
    report.summary = format!("Imagen insertada en la página {}", page + 1);
    Ok(report)
}

/// Aplica `f` a la imagen `id` y sustituye su `/Nombre Do` por el texto que devuelva.
fn edit_placement(
    doc: &mut Document,
    page: usize,
    id: usize,
    f: impl FnOnce(&Placement, &Matrix, &mut PdfDocument, &mut PdfPage) -> Result<String>,
) -> Result<()> {
    doc.with_pdf_mut(|pdf| {
        let mut l = load(pdf, page)?;
        let pl = l
            .list
            .get(id)
            .cloned()
            .ok_or_else(|| Error::InvalidInput(format!("la página {} no tiene la imagen {id}", page + 1)))?;
        let replacement = f(&pl, &l.p, pdf, &mut l.page)?;
        let mut out = Vec::with_capacity(l.bytes.len() + replacement.len());
        out.extend_from_slice(&l.bytes[..pl.range.0]);
        out.extend_from_slice(replacement.as_bytes());
        out.extend_from_slice(&l.bytes[pl.range.1..]);
        let stream = pdf.add_stream(&out, None, false)?;
        l.page.object().dict_put("Contents", stream)
    })
}

/// Borra la imagen `id` de la página. Los recursos sin uso se limpian al guardar con `garbage`.
pub fn delete_object(doc: &mut Document, page: usize, id: usize) -> Result<Report> {
    edit_placement(doc, page, id, |_, _, _, _| Ok(String::new()))?;
    Ok(Report {
        summary: format!("Imagen {id} borrada de la página {}", page + 1),
        ..Report::default()
    })
}

/// Mueve, redimensiona y/o gira la imagen `id`.
pub fn transform_object(doc: &mut Document, page: usize, id: usize, t: &Transform) -> Result<Report> {
    valid_transform(t)?;
    let t = *t;
    edit_placement(doc, page, id, move |pl, p, _, _| {
        let b = bbox_of(&mul(&pl.ctm, p));
        let m = local_transform(&pl.ctm, p, &t, ((b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0))?;
        Ok(format!("q {} cm /{} Do Q", matrix_str(&m), pl.name))
    })?;
    Ok(Report {
        summary: format!("Imagen {id} transformada en la página {}", page + 1),
        ..Report::default()
    })
}

/// Recorta la imagen `id` a `rect` (espacio MuPDF); lo que queda fuera deja de verse, sin perder píxeles.
pub fn crop_image(doc: &mut Document, page: usize, id: usize, rect: RectPts) -> Result<Report> {
    if rect[2] <= rect[0] || rect[3] <= rect[1] {
        return Err(Error::InvalidInput("el recuadro de recorte está vacío".into()));
    }
    edit_placement(doc, page, id, move |pl, p, _, _| {
        // Esquinas del recorte en el espacio local de la imagen: fitz -> usuario -> local.
        let back = mul(&inv(p)?, &inv(&pl.ctm)?);
        let pt = |x: f32, y: f32| back.transform_xy(x, y);
        let (a, b, c, d) = (
            pt(rect[0], rect[1]),
            pt(rect[2], rect[1]),
            pt(rect[2], rect[3]),
            pt(rect[0], rect[3]),
        );
        Ok(format!(
            "q {} {} m {} {} l {} {} l {} {} l h W n /{} Do Q",
            num(a.0),
            num(a.1),
            num(b.0),
            num(b.1),
            num(c.0),
            num(c.1),
            num(d.0),
            num(d.1),
            pl.name
        ))
    })?;
    Ok(Report {
        summary: format!("Imagen {id} recortada en la página {}", page + 1),
        ..Report::default()
    })
}

/// Sustituye la imagen `id` por otra conservando su posición y tamaño.
pub fn replace_image(doc: &mut Document, page: usize, id: usize, data: &[u8]) -> Result<Report> {
    let img = Image::from_bytes(data)
        .map_err(|_| Error::InvalidInput("el archivo no es una imagen que se pueda leer".into()))?;
    let (w, h) = (img.width() as f32, img.height() as f32);
    let mut report = Report::default();
    edit_placement(doc, page, id, |pl, p, pdf, pg| {
        let b = bbox_of(&mul(&pl.ctm, p));
        let (bw, bh) = (b[2] - b[0], b[3] - b[1]);
        if w > 0.0 && h > 0.0 && bh > 0.0 && ((w / h) / (bw / bh) - 1.0).abs() > 0.02 {
            report
                .warnings
                .push("la imagen nueva tiene otra proporción y se deformará al ajustarse al hueco".into());
        }
        let obj = pdf.add_image(&img)?;
        let mut res = pg.resources()?;
        let mut xo = match res.get_dict("XObject")? {
            Some(x) if x.is_dict()? => x,
            _ => {
                let d = pdf.new_dict()?;
                res.dict_put("XObject", d.clone())?;
                d
            }
        };
        let mut n = 0;
        let name = loop {
            let cand = format!("PlImg{n}");
            if xo.get_dict(cand.as_str())?.is_none() {
                break cand;
            }
            n += 1;
        };
        xo.dict_put(name.as_str(), obj)?;
        Ok(format!("/{name} Do"))
    })?;
    report.summary = format!("Imagen {id} reemplazada en la página {}", page + 1);
    Ok(report)
}
