//! Aplanar anotaciones y campos (`PdfDocument::bake`): su apariencia pasa a ser contenido fijo de la página.
//!
//! Para cada anotación se coloca su `/AP /N` en el `/Rect` con el algoritmo de ISO 32000-1 §12.5.5 (el rectángulo
//! de la caja transformada por `/Matrix` se ajusta al `/Rect`) y se saca de `/Annots`. Con campos, además se
//! elimina `/AcroForm` del catálogo.

use std::fmt::Write as _;

use lopdf::{Dictionary, Object as L, ObjectId};
use pliego_core::Result;

use super::appearance::n;
use super::document::PdfDocument;
use super::formjs::{self, JsDocEvent, JsMessage};
use super::pdf_err;
use super::{State, tree};

struct Plan {
    id: ObjectId,
    /// Formulario a dibujar y la matriz `a b c d e f` que lo coloca; `None` si no se dibuja.
    draw: Option<(ObjectId, [f32; 6])>,
}

fn floats(st: &State, o: Option<&L>) -> Vec<f32> {
    match o.and_then(|o| st.deref(o)) {
        Some(L::Array(a)) => a.iter().filter_map(|x| st.deref(x)?.as_float().ok()).collect(),
        _ => Vec::new(),
    }
}

/// Matriz que lleva el formulario `form` al rectángulo `rect` (§12.5.5, pasos 1-3).
fn placement(st: &State, form: ObjectId, rect: [f32; 4]) -> Option<[f32; 6]> {
    let fd = st.dict_of(st.get(form)?)?;
    let bbox = floats(st, fd.get(b"BBox").ok());
    if bbox.len() < 4 {
        return None;
    }
    let m = {
        let m = floats(st, fd.get(b"Matrix").ok());
        if m.len() == 6 {
            [m[0], m[1], m[2], m[3], m[4], m[5]]
        } else {
            [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]
        }
    };
    let corners = [
        (bbox[0], bbox[1]),
        (bbox[2], bbox[1]),
        (bbox[0], bbox[3]),
        (bbox[2], bbox[3]),
    ];
    let pts: Vec<(f32, f32)> = corners
        .iter()
        .map(|&(x, y)| (x * m[0] + y * m[2] + m[4], x * m[1] + y * m[3] + m[5]))
        .collect();
    let (x0, x1) = pts
        .iter()
        .fold((f32::INFINITY, f32::NEG_INFINITY), |a, p| (a.0.min(p.0), a.1.max(p.0)));
    let (y0, y1) = pts
        .iter()
        .fold((f32::INFINITY, f32::NEG_INFINITY), |a, p| (a.0.min(p.1), a.1.max(p.1)));
    let (rx0, rx1) = (rect[0].min(rect[2]), rect[0].max(rect[2]));
    let (ry0, ry1) = (rect[1].min(rect[3]), rect[1].max(rect[3]));
    let sx = if x1 - x0 > 1e-6 { (rx1 - rx0) / (x1 - x0) } else { 1.0 };
    let sy = if y1 - y0 > 1e-6 { (ry1 - ry0) / (y1 - y0) } else { 1.0 };
    Some([sx, 0.0, 0.0, sy, rx0 - x0 * sx, ry0 - y0 * sy])
}

fn plan_page(st: &State, page: ObjectId, annots: bool, widgets: bool) -> Vec<Plan> {
    let Some(pd) = st.dict_of(match st.get(page) {
        Some(p) => p,
        None => return Vec::new(),
    }) else {
        return Vec::new();
    };
    let list: Vec<ObjectId> = match pd.get(b"Annots").ok().and_then(|a| st.deref(a)) {
        Some(L::Array(a)) => a
            .iter()
            .filter_map(|x| {
                if let L::Reference(r) = x {
                    Some(st.resolve_id(*r))
                } else {
                    None
                }
            })
            .collect(),
        _ => Vec::new(),
    };
    let mut out = Vec::new();
    for id in list {
        let Some(d) = st.dict_of(match st.get(id) {
            Some(o) => o,
            None => continue,
        }) else {
            continue;
        };
        let sub = d.get(b"Subtype").ok().and_then(|s| s.as_name().ok()).unwrap_or(b"");
        let take = match sub {
            b"Widget" => widgets,
            b"Link" => false,
            b"Popup" => annots,
            _ => annots,
        };
        if !take {
            continue;
        }
        let flags = d.get(b"F").ok().and_then(|f| f.as_float().ok()).map_or(0, |f| f as i32);
        let hidden = flags & (2 | 32) != 0 || sub == b"Popup";
        let rect = floats(st, d.get(b"Rect").ok());
        let draw = (|| {
            if hidden || rect.len() < 4 {
                return None;
            }
            let ap = match d.get(b"AP").ok().and_then(|a| st.deref(a))? {
                L::Dictionary(a) => a,
                _ => return None,
            };
            let n = ap.get(b"N").ok()?;
            let pick = |states: &Dictionary| -> Option<ObjectId> {
                let state = d.get(b"AS").ok().and_then(|s| s.as_name().ok())?;
                match states.get(state).ok()? {
                    L::Reference(r) => Some(st.resolve_id(*r)),
                    _ => None,
                }
            };
            let form = match n {
                L::Reference(r) => {
                    let r = st.resolve_id(*r);
                    match st.doc.objects.get(&r)? {
                        L::Stream(_) => r,
                        // Diccionario de estados indirecto: se elige el de /AS.
                        L::Dictionary(states) => pick(states)?,
                        _ => return None,
                    }
                }
                // Diccionario de estados directo (casillas y radios).
                L::Dictionary(states) => pick(states)?,
                _ => return None,
            };
            Some((form, placement(st, form, [rect[0], rect[1], rect[2], rect[3]])?))
        })();
        out.push(Plan { id, draw });
    }
    out
}

impl PdfDocument {
    /// Aplana las anotaciones (`bake_annots`) y/o los campos (`bake_widgets`): dibuja su apariencia en el contenido
    /// de la página y las quita. Los enlaces se conservan.
    pub fn bake(&mut self, bake_annots: bool, bake_widgets: bool) -> Result<()> {
        self.guard()?;
        if !bake_annots && !bake_widgets {
            return Ok(());
        }
        // Un campo sin apariencia (valor puesto con `set_value` en un archivo antiguo) se dibuja antes de aplanar.
        if bake_widgets {
            for p in 0..self.page_count()? {
                let page = self.load_pdf_page(p)?;
                for mut w in page.widgets() {
                    if !w.annotation().object().get_dict("AP")?.is_some() {
                        w.update()?;
                    }
                }
            }
        }
        for p in 0..self.page_count()? {
            let mut page = self.load_pdf_page(p)?;
            let page_id = page
                .page
                .obj_id()
                .ok_or_else(|| pdf_err("página sin número de objeto"))?;
            let plans = plan_page(&self.st.borrow(), page_id, bake_annots, bake_widgets);
            if plans.is_empty() {
                continue;
            }
            let mut content = String::new();
            let mut names: Vec<(String, ObjectId)> = Vec::new();
            for (i, plan) in plans.iter().enumerate() {
                if let Some((form, m)) = plan.draw {
                    let name = format!("PliegoFlat{i}");
                    let _ = writeln!(
                        content,
                        "q\n{} {} {} {} {} {} cm\n/{name} Do\nQ",
                        n(m[0]),
                        n(m[1]),
                        n(m[2]),
                        n(m[3]),
                        n(m[4]),
                        n(m[5])
                    );
                    names.push((name, form));
                }
            }
            if !names.is_empty() {
                let res = page.resources()?;
                let xo = match res.get_dict("XObject")? {
                    Some(x) if x.is_dict()? => x,
                    _ => {
                        res.dict_set(b"XObject", L::Dictionary(Dictionary::new()))?;
                        res.get_dict("XObject")?
                            .ok_or_else(|| pdf_err("no se pudo crear /XObject"))?
                    }
                };
                for (name, form) in &names {
                    xo.dict_set(name.as_bytes(), L::Reference(*form))?;
                }
                page.wrap_contents(self)?;
                page.insert_contents(self, content.as_bytes(), true)?;
            }
            // Fuera de /Annots.
            let gone: Vec<ObjectId> = plans.iter().map(|p| p.id).collect();
            if let Some(arr) = page.page.get_dict("Annots")?
                && arr.is_array()?
            {
                arr.modify(|o| {
                    if let L::Array(a) = o {
                        a.retain(|x| !matches!(x, L::Reference(r) if gone.contains(r)));
                    }
                    Ok(())
                })?;
            }
        }
        if bake_widgets {
            let mut st = self.st.borrow_mut();
            if let Some(root) = tree::catalog_id(&st) {
                if let Some(L::Dictionary(cd)) = st.doc.objects.get_mut(&root) {
                    cd.remove(b"AcroForm");
                }
                st.touch(root);
            }
        }
        Ok(())
    }

    /// Habilita el JavaScript de formularios (motor `pliego-formjs`): `PdfWidget::set_value` y `calculate_form` pasan a
    /// ejecutar tecleo, validación, cálculo y formato. Ejecuta también los scripts de documento (`/Names /JavaScript`).
    pub fn enable_js(&mut self) -> Result<()> {
        self.guard()?;
        formjs::enable(&self.st)
    }

    pub fn disable_js(&mut self) -> Result<()> {
        formjs::disable(&self.st);
        Ok(())
    }

    pub fn is_js_supported(&self) -> Result<bool> {
        Ok(true)
    }

    /// Recalcula los campos con acción de cálculo, en el orden de `/CO`, y da formato a los que cambian. Sin
    /// `enable_js` no hace nada.
    pub fn calculate_form(&mut self) -> Result<()> {
        self.guard()?;
        formjs::calculate(&self.st)
    }

    /// Ejecuta el script de un evento de documento o de página (`/OpenAction`, `/AA` del catálogo o de una página).
    /// Devuelve `false` si el JavaScript no está habilitado o el evento no tiene script. La aplicación decide cuándo
    /// llamarlo: al abrir, antes y después de guardar o imprimir, al mostrar una página.
    pub fn run_document_event(&mut self, event: JsDocEvent) -> Result<bool> {
        self.guard()?;
        formjs::run_document_event(&self.st, event)
    }

    /// Avisos de los scripts desde la última llamada: `app.alert`, `console.println`, acciones bloqueadas (enviar el
    /// formulario, abrir una URL...) y errores de script. La interfaz decide qué mostrar.
    pub fn take_js_messages(&mut self) -> Vec<JsMessage> {
        std::mem::take(&mut self.st.borrow_mut().js.log)
    }
}
