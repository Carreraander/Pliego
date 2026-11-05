//! Copia de objetos entre documentos (`graft`) y de páginas (`insert_pdf`, `duplicate_page`).

use std::collections::HashMap;
use std::rc::Rc;

use lopdf::Dictionary;
use lopdf::Object as L;
use lopdf::ObjectId;
use pliego_core::Result;

use super::document::{PdfDocument, catalog_id, dict_of_mut, set_dict_entry};
use super::object::{Object, Root};
use super::{Shared, State, pdf_err, tree};

const MAX_DEPTH: usize = 128;

/// Copia de objetos entre documentos conservando lo compartido (ver [`PdfDocument::new_graft_map`]).
pub struct GraftMap {
    dst: Shared,
    /// Por documento origen (dirección del `Rc`): objeto origen → objeto copiado.
    maps: HashMap<usize, HashMap<ObjectId, ObjectId>>,
}

impl GraftMap {
    pub(crate) fn new(dst: Shared) -> Self {
        Self {
            dst,
            maps: HashMap::new(),
        }
    }

    pub fn graft_object(&mut self, obj: &Object) -> Result<Object> {
        let src = obj
            .st
            .clone()
            .ok_or_else(|| pdf_err("el objeto no pertenece a ningún documento"))?;
        if Rc::ptr_eq(&src, &self.dst) {
            return Err(pdf_err("graft_object copia entre documentos distintos"));
        }
        let key = Rc::as_ptr(&src) as usize;
        let map = self.maps.entry(key).or_default();
        let src_state = src.borrow();
        let mut dst_state = self.dst.borrow_mut();
        let mut copier = Copier {
            src: &src_state,
            dst: &mut dst_state,
            map,
            queue: Vec::new(),
            hook: &|_, _| Hook::Copy,
        };
        // Referencia, o raíz indirecta ya resuelta (un flujo no puede ser directo): se copia como objeto.
        let as_object = match (&obj.root, obj.path.is_empty()) {
            (Root::Obj(id), true) => {
                let is_stream = copier.src.get(*id).is_some_and(|o| matches!(o, L::Stream(_)));
                (obj.indirect || is_stream).then_some(*id)
            }
            _ => None,
        };
        let out = match as_object {
            Some(id) => Ok(L::Reference(copier.graft_id(id))),
            None => obj.read(|v| Ok(copier.rewrite(v, 0))),
        };
        let out = out?;
        copier.run();
        drop(dst_state);
        match out {
            L::Reference(id) => Ok(Object::reference(&self.dst, id)),
            other => Ok(Object::free(Some(self.dst.clone()), other)),
        }
    }
}

pub(crate) enum Hook {
    /// Copiar normalmente.
    Copy,
    /// Sustituir la referencia por este valor.
    Replace(L),
}

/// Copiador de grafos de objetos: reserva el id nuevo al ver un objeto y lo rellena después, sin recursión
/// sobre la cadena de referencias (los árboles largos no desbordan la pila).
pub(crate) struct Copier<'a> {
    pub(crate) src: &'a State,
    pub(crate) dst: &'a mut State,
    pub(crate) map: &'a mut HashMap<ObjectId, ObjectId>,
    queue: Vec<(ObjectId, ObjectId)>,
    pub(crate) hook: &'a dyn Fn(&State, ObjectId) -> Hook,
}

impl<'a> Copier<'a> {
    pub(crate) fn new(
        src: &'a State,
        dst: &'a mut State,
        map: &'a mut HashMap<ObjectId, ObjectId>,
        hook: &'a dyn Fn(&State, ObjectId) -> Hook,
    ) -> Self {
        Self {
            src,
            dst,
            map,
            queue: Vec::new(),
            hook,
        }
    }

    pub(crate) fn graft_id(&mut self, id: ObjectId) -> ObjectId {
        let id = self.src.resolve_id(id);
        if let Some(n) = self.map.get(&id) {
            return *n;
        }
        let new = self.dst.new_id();
        self.dst.doc.objects.insert(new, L::Null);
        self.dst.touch(new);
        self.map.insert(id, new);
        self.queue.push((id, new));
        new
    }

    /// Copia un valor sustituyendo cada referencia por la del documento destino.
    pub(crate) fn rewrite(&mut self, v: &L, depth: usize) -> L {
        if depth > MAX_DEPTH {
            return L::Null;
        }
        match v {
            L::Reference(id) => {
                if !self.src.doc.objects.contains_key(&self.src.resolve_id(*id)) {
                    return L::Null;
                }
                if let Hook::Replace(r) = (self.hook)(self.src, self.src.resolve_id(*id)) {
                    return r;
                }
                L::Reference(self.graft_id(*id))
            }
            L::Array(a) => L::Array(a.iter().map(|x| self.rewrite(x, depth + 1)).collect()),
            L::Dictionary(d) => L::Dictionary(self.rewrite_dict(d, depth)),
            L::Stream(s) => {
                let mut s = s.clone();
                s.dict = self.rewrite_dict(&s.dict, depth);
                L::Stream(s)
            }
            other => other.clone(),
        }
    }

    pub(crate) fn rewrite_dict(&mut self, d: &Dictionary, depth: usize) -> Dictionary {
        let mut out = Dictionary::new();
        for (k, v) in d {
            out.set(k.clone(), self.rewrite(v, depth + 1));
        }
        out
    }

    /// Rellena los objetos reservados (y los que estos reserven a su vez).
    pub(crate) fn run(&mut self) {
        while let Some((from, to)) = self.queue.pop() {
            let Some(obj) = self.src.doc.objects.get(&from) else {
                continue;
            };
            let copy = self.rewrite(obj, 0);
            self.dst.doc.objects.insert(to, copy);
            self.dst.touch(to);
        }
    }
}

fn type_is(st: &State, id: ObjectId, names: &[&[u8]]) -> bool {
    st.doc
        .objects
        .get(&id)
        .and_then(|o| st.dict_of(o))
        .and_then(|d| d.get(b"Type").ok())
        .and_then(|t| t.as_name().ok())
        .is_some_and(|t| names.contains(&t))
}

/// Atributos de página heredados de sus ascendientes, como pares clave/valor ya resueltos en `src`.
fn inherited(src: &State, page: ObjectId) -> Vec<(&'static str, L)> {
    let mut out = Vec::new();
    for key in ["Resources", "MediaBox", "CropBox", "Rotate"] {
        let mut cur = Some(page);
        for _ in 0..64 {
            let Some(id) = cur else { break };
            let d = src.doc.objects.get(&id).and_then(|o| src.dict_of(o));
            if let Some(v) = d.and_then(|d| d.get(key.as_bytes()).ok()) {
                out.push((key, v.clone()));
                break;
            }
            cur = d.and_then(|d| match d.get(b"Parent") {
                Ok(L::Reference(p)) => Some(src.resolve_id(*p)),
                _ => None,
            });
        }
    }
    out
}

/// Copia las páginas `idx` (índices de `src`) a `dst` a partir de la posición `at`.
pub(crate) fn insert_pages(
    dst_doc: &mut PdfDocument,
    src_doc: &PdfDocument,
    idx: &[usize],
    at: usize,
    rotate: Option<i32>,
) -> Result<()> {
    if Rc::ptr_eq(dst_doc.state(), src_doc.state()) {
        return Err(pdf_err("insert_pdf copia entre documentos distintos"));
    }
    let src_pages = src_doc.page_ids();
    let src_state = src_doc.state().borrow();
    let mut dst_state = dst_doc.state().borrow_mut();

    // Primera copia de cada página seleccionada: los enlaces internos entre ellas se conservan.
    let selected: Vec<ObjectId> = idx.iter().map(|&i| src_pages[i]).collect();
    let mut page_new: HashMap<ObjectId, ObjectId> = HashMap::new();
    let mut new_ids = Vec::with_capacity(idx.len());
    for &sp in &selected {
        let new = dst_state.new_id();
        dst_state.doc.objects.insert(new, L::Null);
        dst_state.touch(new);
        page_new.entry(sp).or_insert(new);
        new_ids.push((sp, new));
    }

    let mut map: HashMap<ObjectId, ObjectId> = HashMap::new();
    let page_new_ref = page_new.clone();
    let hook = move |st: &State, id: ObjectId| -> Hook {
        if let Some(n) = page_new_ref.get(&id) {
            return Hook::Replace(L::Reference(*n));
        }
        // Una página o un nodo del árbol que no se copia: arrastraría el documento entero.
        if type_is(st, id, &[b"Page", b"Pages"]) {
            return Hook::Replace(L::Null);
        }
        Hook::Copy
    };
    let mut widgets: Vec<ObjectId> = Vec::new();
    {
        let mut copier = Copier::new(&src_state, &mut dst_state, &mut map, &hook);
        for (sp, new) in &new_ids {
            let Some(L::Dictionary(d)) = src_state.doc.objects.get(sp) else {
                return Err(pdf_err("la página de origen no es un diccionario"));
            };
            let mut d = d.clone();
            d.remove(b"Parent");
            d.remove(b"StructParents");
            for (k, v) in inherited(&src_state, *sp) {
                if !d.has(k.as_bytes()) {
                    d.set(k, v);
                }
            }
            let mut copy = copier.rewrite_dict(&d, 0);
            if let Some(r) = rotate {
                let cur = copy.get(b"Rotate").and_then(L::as_i64).unwrap_or(0);
                copy.set("Rotate", (cur + i64::from(r)).rem_euclid(360));
            }
            copier.dst.doc.objects.insert(*new, L::Dictionary(copy));
            copier.dst.touch(*new);
        }
        copier.run();
    }
    // Tras `run` las anotaciones copiadas ya tienen contenido: se localizan los widgets.
    for (_, new) in &new_ids {
        let annots = match dst_state
            .doc
            .objects
            .get(new)
            .and_then(|o| dst_state.dict_of(o))
            .and_then(|d| d.get(b"Annots").ok())
        {
            Some(L::Array(a)) => a.clone(),
            _ => continue,
        };
        for a in annots {
            if let L::Reference(id) = a
                && let Some(L::Dictionary(ad)) = dst_state.doc.objects.get(&id)
                && matches!(ad.get(b"Subtype").and_then(L::as_name), Ok(b"Widget"))
            {
                widgets.push(id);
            }
        }
    }

    // Campos de formulario: el campo raíz de cada widget debe colgar de /AcroForm /Fields del destino.
    if !widgets.is_empty() {
        let src_form = catalog_dict_of(&src_state).and_then(|c| c.get(b"AcroForm").ok().cloned());
        register_fields(&mut dst_state, &src_state, &mut map, &widgets, src_form)?;
    }

    for (k, (_, new)) in new_ids.iter().enumerate() {
        tree::insert_page(&mut dst_state, at + k, *new)?;
    }
    Ok(())
}

fn catalog_dict_of(st: &State) -> Option<&Dictionary> {
    super::document::catalog_dict(st)
}

fn register_fields(
    dst: &mut State,
    src: &State,
    map: &mut HashMap<ObjectId, ObjectId>,
    widgets: &[ObjectId],
    src_form: Option<L>,
) -> Result<()> {
    let cat = catalog_id(dst)?;
    // AcroForm del destino (se crea copiando los ajustes del de origen si no existe).
    let existing = catalog_dict_of(dst).and_then(|c| c.get(b"AcroForm").ok().cloned());
    let form_id = match existing {
        Some(L::Reference(id)) => dst.resolve_id(id),
        Some(L::Dictionary(d)) => {
            let id = dst.add(L::Dictionary(d));
            set_dict_entry(dst, cat, "AcroForm", L::Reference(id))?;
            id
        }
        _ => {
            let hook = |_: &State, _: ObjectId| Hook::Copy;
            let mut form = Dictionary::new();
            if let Some(sf) = src_form.as_ref().and_then(|f| src.dict_of(f)) {
                let mut copier = Copier::new(src, dst, map, &hook);
                for key in ["DA", "DR", "Q", "NeedAppearances"] {
                    if let Ok(v) = sf.get(key.as_bytes()) {
                        form.set(key, copier.rewrite(v, 0));
                    }
                }
                copier.run();
            }
            form.set("Fields", L::Array(Vec::new()));
            let id = dst.add(L::Dictionary(form));
            set_dict_entry(dst, cat, "AcroForm", L::Reference(id))?;
            id
        }
    };
    let mut fields = match dict_of_mut(dst, form_id).and_then(|d| d.get(b"Fields").ok().cloned()) {
        Some(L::Array(a)) => a,
        _ => Vec::new(),
    };
    for w in widgets {
        let mut top = *w;
        for _ in 0..64 {
            match dst
                .doc
                .objects
                .get(&top)
                .and_then(|o| dst.dict_of(o))
                .and_then(|d| d.get(b"Parent").ok())
            {
                Some(L::Reference(p)) => top = dst.resolve_id(*p),
                _ => break,
            }
        }
        if !fields.iter().any(|f| *f == L::Reference(top)) {
            fields.push(L::Reference(top));
        }
    }
    if let Some(d) = dict_of_mut(dst, form_id) {
        d.set("Fields", L::Array(fields));
        dst.touch(form_id);
    }
    Ok(())
}

/// Tras copiar una página dentro del mismo documento, da a la copia sus propias anotaciones.
pub(crate) fn clone_page_annotations(st: &mut State, page: ObjectId) -> Result<()> {
    let annots: Vec<L> = match st
        .doc
        .objects
        .get(&page)
        .and_then(|o| st.dict_of(o))
        .and_then(|d| d.get(b"Annots").ok())
    {
        Some(L::Array(a)) => a.clone(),
        Some(L::Reference(r)) => match st.get(*r) {
            Some(L::Array(a)) => a.clone(),
            _ => return Ok(()),
        },
        _ => return Ok(()),
    };
    let mut out = Vec::with_capacity(annots.len());
    let mut form_adds: Vec<ObjectId> = Vec::new();
    for a in annots {
        let L::Reference(id) = a else {
            out.push(a);
            continue;
        };
        let Some(L::Dictionary(d)) = st.get(id).cloned() else {
            out.push(L::Reference(id));
            continue;
        };
        let mut d = d;
        d.set("P", L::Reference(page));
        let is_widget = matches!(d.get(b"Subtype").and_then(L::as_name), Ok(b"Widget"));
        let parent = match d.get(b"Parent") {
            Ok(L::Reference(p)) => Some(st.resolve_id(*p)),
            _ => None,
        };
        let new = st.add(L::Dictionary(d));
        if is_widget {
            match parent {
                Some(p) => {
                    if let Some(pd) = dict_of_mut(st, p)
                        && let Ok(L::Array(kids)) = pd.get_mut(b"Kids")
                    {
                        kids.push(L::Reference(new));
                        st.touch(p);
                    }
                }
                None => form_adds.push(new),
            }
        }
        out.push(L::Reference(new));
    }
    if let Some(d) = dict_of_mut(st, page) {
        d.set("Annots", L::Array(out));
        st.touch(page);
    }
    if !form_adds.is_empty() {
        let cat = catalog_id(st)?;
        let form = catalog_dict_of(st).and_then(|c| c.get(b"AcroForm").ok().cloned());
        if let Some(L::Reference(fid)) = form {
            let fid = st.resolve_id(fid);
            if let Some(d) = dict_of_mut(st, fid)
                && let Ok(L::Array(fields)) = d.get_mut(b"Fields")
            {
                fields.extend(form_adds.iter().map(|n| L::Reference(*n)));
                st.touch(fid);
            }
        } else if let Some(L::Dictionary(mut fd)) = form {
            let mut fields = match fd.get(b"Fields") {
                Ok(L::Array(a)) => a.clone(),
                _ => Vec::new(),
            };
            fields.extend(form_adds.iter().map(|n| L::Reference(*n)));
            fd.set("Fields", L::Array(fields));
            set_dict_entry(st, cat, "AcroForm", L::Dictionary(fd))?;
        }
    }
    Ok(())
}
