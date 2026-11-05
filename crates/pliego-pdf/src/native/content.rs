//! Recorrido del contenido de una página (bloque 5 de la ola 5, W39).
//!
//! Intérprete mínimo de operadores gráficos: solo lleva la matriz de transformación (`q`, `Q`, `cm`) y baja a los
//! formularios (`Do` de un XObject `Form`, con su `/Matrix` y `/Resources`) y a las imágenes (`Do` de un XObject
//! `Image` e imágenes en línea `BI … EI`). No dibuja ni mide texto: eso es de PDFium (W37). De momento sirve a
//! [`PdfPage::image_placements`]; el mismo recorrido valdrá para cualquier otra consulta sobre el contenido.

use std::collections::HashMap;

use lopdf::content::Content;
use lopdf::{Dictionary, Object as L, ObjectId};
use pliego_core::Result;

use super::image::Image;
use super::object::decode_stream;
use super::page::page_ctm;
use super::{PdfPage, State, pdf_err};
use crate::geom::Matrix;

/// Anidamiento máximo de formularios (los reales no pasan de 4 o 5).
const MAX_DEPTH: usize = 12;
/// Operadores que se interpretan como máximo por página: defensa contra contenido hostil.
const MAX_OPS: usize = 20_000_000;

/// `a` y después `b` (la transformación `a` se aplica primero).
fn then(a: Matrix, b: Matrix) -> Matrix {
    Matrix::new(
        a.a * b.a + a.b * b.c,
        a.a * b.b + a.b * b.d,
        a.c * b.a + a.d * b.c,
        a.c * b.b + a.d * b.d,
        a.e * b.a + a.f * b.c + b.e,
        a.e * b.b + a.f * b.d + b.f,
    )
}

fn number(o: &L) -> Option<f32> {
    match o {
        L::Integer(i) => Some(*i as f32),
        L::Real(r) => Some(*r),
        _ => None,
    }
}

fn matrix_from(st: &State, o: Option<&L>) -> Option<Matrix> {
    let L::Array(a) = st.deref(o?)? else { return None };
    let v: Vec<f32> = a.iter().filter_map(|x| st.deref(x).and_then(number)).collect();
    (v.len() == 6).then(|| Matrix::new(v[0], v[1], v[2], v[3], v[4], v[5]))
}

struct Walker<'a> {
    st: &'a State,
    ops: usize,
    /// Imágenes de XObject ya decodificadas (la misma imagen se dibuja a menudo muchas veces).
    cache: HashMap<ObjectId, Option<Image>>,
    /// Formularios en curso, contra ciclos.
    open: Vec<ObjectId>,
    out: Vec<(Image, Matrix)>,
}

impl Walker<'_> {
    fn run(&mut self, content: &[u8], resources: Option<&Dictionary>, start: Matrix, depth: usize) {
        let Ok(content) = Content::decode(content) else { return };
        let mut ctm = start;
        let mut stack: Vec<Matrix> = Vec::new();
        for op in &content.operations {
            self.ops += 1;
            if self.ops > MAX_OPS {
                return;
            }
            match op.operator.as_str() {
                "q" => stack.push(ctm),
                "Q" => {
                    if let Some(m) = stack.pop() {
                        ctm = m;
                    }
                }
                "cm" => {
                    let v: Vec<f32> = op.operands.iter().filter_map(number).collect();
                    if v.len() == 6 {
                        ctm = then(Matrix::new(v[0], v[1], v[2], v[3], v[4], v[5]), ctm);
                    }
                }
                "BI" => {
                    if let Some(L::Stream(s)) = op.operands.first()
                        && let Ok(img) = Image::from_stream(self.st, s, true)
                    {
                        self.out.push((img, ctm));
                    }
                }
                "Do" => {
                    let Some(L::Name(name)) = op.operands.first() else {
                        continue;
                    };
                    self.xobject(name, resources, ctm, depth);
                }
                _ => {}
            }
        }
    }

    fn xobject(&mut self, name: &[u8], resources: Option<&Dictionary>, ctm: Matrix, depth: usize) {
        let st = self.st;
        let Some(entry) = resources
            .and_then(|r| r.get(b"XObject").ok())
            .and_then(|x| st.dict_of(x))
            .and_then(|x| x.get(name).ok())
        else {
            return;
        };
        let id = match entry {
            L::Reference(id) => Some(st.resolve_id(*id)),
            _ => None,
        };
        let Some(L::Stream(stream)) = st.deref(entry) else {
            return;
        };
        let subtype = stream
            .dict
            .get(b"Subtype")
            .ok()
            .and_then(|s| st.deref(s))
            .and_then(|s| match s {
                L::Name(n) => Some(n.as_slice()),
                _ => None,
            });
        match subtype {
            Some(b"Image") => {
                let img = match id {
                    Some(id) => self
                        .cache
                        .entry(id)
                        .or_insert_with(|| Image::from_stream(st, stream, false).ok())
                        .clone(),
                    None => Image::from_stream(st, stream, false).ok(),
                };
                if let Some(img) = img {
                    self.out.push((img, ctm));
                }
            }
            Some(b"Form") => {
                if depth >= MAX_DEPTH || id.is_some_and(|i| self.open.contains(&i)) {
                    return;
                }
                let Ok(body) = decode_stream(stream) else { return };
                let m = matrix_from(st, stream.dict.get(b"Matrix").ok()).unwrap_or(Matrix::IDENTITY);
                let own = stream.dict.get(b"Resources").ok().and_then(|r| st.dict_of(r));
                if let Some(i) = id {
                    self.open.push(i);
                }
                self.run(&body, own.or(resources), then(m, ctm), depth + 1);
                if id.is_some() {
                    self.open.pop();
                }
            }
            _ => {}
        }
    }
}

/// `/Resources` de la página, heredados de sus ancestros si hace falta.
fn page_resources(st: &State, page: ObjectId) -> Option<&Dictionary> {
    let mut id = page;
    for _ in 0..32 {
        let L::Dictionary(d) = st.get(id)? else { return None };
        if let Some(r) = d.get(b"Resources").ok().and_then(|r| st.dict_of(r)) {
            return Some(r);
        }
        id = match d.get(b"Parent").ok()? {
            L::Reference(p) => *p,
            _ => return None,
        };
    }
    None
}

/// Contenido de la página: todos los flujos de `/Contents` unidos con un salto de línea.
fn page_content(st: &State, page: ObjectId) -> Vec<u8> {
    let Some(L::Dictionary(d)) = st.get(page) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut add = |o: &L| {
        if let Some(L::Stream(s)) = st.deref(o)
            && let Ok(b) = decode_stream(s)
        {
            out.extend_from_slice(&b);
            out.push(b'\n');
        }
    };
    match d.get(b"Contents").ok().and_then(|c| st.deref(c)) {
        Some(L::Array(a)) => a.iter().for_each(&mut add),
        Some(s @ L::Stream(_)) => add(s),
        _ => {}
    }
    out
}

impl PdfPage {
    /// Cada imagen que dibuja el contenido de la página (también dentro de formularios y en línea), con la matriz
    /// con la que se coloca en el espacio de página (origen arriba a la izquierda, `y` hacia abajo, giro aplicado) y
    /// con la imagen apoyada en su esquina superior izquierda, como MuPDF. Sirve para la resolución efectiva.
    pub fn image_placements(&self) -> Result<Vec<(Image, Matrix)>> {
        let shared = self
            .page
            .st
            .clone()
            .ok_or_else(|| pdf_err("la página no pertenece a ningún documento"))?;
        let id = self
            .page
            .obj_id()
            .ok_or_else(|| pdf_err("la página no es un objeto indirecto"))?;
        let to_page = page_ctm(&self.page)?;
        let st = shared.borrow();
        let id = st.resolve_id(id);
        let content = page_content(&st, id);
        let mut walker = Walker {
            st: &st,
            ops: 0,
            cache: HashMap::new(),
            open: Vec::new(),
            out: Vec::new(),
        };
        walker.run(&content, page_resources(&st, id), Matrix::IDENTITY, 0);
        // El cuadrado unidad de la imagen tiene el origen arriba a la izquierda: se voltea antes de pasar a página.
        let flip = Matrix::new(1.0, 0.0, 0.0, -1.0, 0.0, 1.0);
        Ok(walker
            .out
            .into_iter()
            .map(|(img, m)| (img, then(then(flip, m), to_page)))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrix_order_applies_the_first_one_first() {
        // Escalar ×2 y luego trasladar (10, 0): el punto (1, 0) acaba en (12, 0).
        let m = then(Matrix::new_scale(2.0, 2.0), Matrix::new_translate(10.0, 0.0));
        assert_eq!(m.transform_xy(1.0, 0.0), (12.0, 0.0));
    }
}
