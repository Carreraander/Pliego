//! [`Object`]: asa a un objeto PDF del documento (una *ruta*, véase el módulo padre).

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use lopdf::Object as L;
use lopdf::{Dictionary, ObjectId, Stream, StringFormat};
use pliego_core::{Error, Result};

use super::parse::{object_to_string, parse_object};
use super::{NULL, Shared, err, pdf_err};
use crate::geom::Matrix;
use crate::object::Value;

#[derive(Clone)]
pub(crate) enum Root {
    /// Objeto indirecto número `id`.
    Obj(ObjectId),
    /// Diccionario `trailer`.
    Trailer,
    /// Valor directo que aún no cuelga de nada (recién construido).
    Free(Rc<RefCell<L>>),
}

#[derive(Clone, Debug)]
pub(crate) enum Step {
    Key(Vec<u8>),
    Idx(usize),
}

/// Asa a un objeto PDF. Ver el contrato en [`crate::Object`] y el diseño en el módulo `native`.
#[derive(Clone)]
pub struct Object {
    pub(crate) st: Option<Shared>,
    pub(crate) root: Root,
    pub(crate) path: Vec<Step>,
    /// Es la referencia `n g R` (y no el objeto ya resuelto).
    pub(crate) indirect: bool,
}

/// Qué hay en una posición: otra referencia o un valor directo.
enum Kid {
    Ref(ObjectId),
    Direct,
}

fn kid_of(v: &L) -> Kid {
    match v {
        L::Reference(id) => Kid::Ref(*id),
        _ => Kid::Direct,
    }
}

fn walk<'a>(mut cur: &'a L, path: &[Step]) -> &'a L {
    for s in path {
        cur = match (s, cur) {
            (Step::Key(k), L::Dictionary(d)) => d.get(k).unwrap_or(&NULL),
            (Step::Key(k), L::Stream(st)) => st.dict.get(k).unwrap_or(&NULL),
            (Step::Idx(i), L::Array(a)) => a.get(*i).unwrap_or(&NULL),
            _ => &NULL,
        };
    }
    cur
}

fn walk_mut<'a>(mut cur: &'a mut L, path: &[Step]) -> Option<&'a mut L> {
    for s in path {
        cur = match (s, cur) {
            (Step::Key(k), L::Dictionary(d)) => d.get_mut(k).ok()?,
            (Step::Key(k), L::Stream(st)) => st.dict.get_mut(k).ok()?,
            (Step::Idx(i), L::Array(a)) => a.get_mut(*i)?,
            _ => return None,
        };
    }
    Some(cur)
}

/// Filtros que lopdf sabe aplicar; el resto (DCT, JPX, CCITT, JBIG2…) son de imagen y se dejan puestos.
const DECODABLE: &[&[u8]] = &[
    b"FlateDecode",
    b"LZWDecode",
    b"ASCII85Decode",
    b"ASCIIHexDecode",
    b"RunLengthDecode",
    b"BrotliDecode",
];
/// Límite de tamaño al descomprimir un flujo (defensa contra bombas de descompresión).
const MAX_DECODED: usize = 1 << 30;

/// Clave que marca, en el diccionario de un flujo, que su contenido no se cargó (documentos abiertos con
/// `PdfDocument::open_for_viewing` y `PdfDocument::open_lean`). Nunca se escribe en un archivo.
pub(crate) const OMITTED_KEY: &[u8] = b"PliegoOmitido";

std::thread_local! {
    /// Alguien intentó leer un flujo omitido en este hilo desde el último `PdfDocument::open_lean`: lo que se haya
    /// calculado con ese contenido vacío no es fiable, así que el guardado ligero se niega (y se usa el completo).
    pub(crate) static OMITTED_READ: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// `true` si el contenido de `s` no se cargó; en ese caso anota la lectura en [`OMITTED_READ`].
pub(crate) fn omitted(s: &Stream) -> bool {
    let om = s.dict.has(OMITTED_KEY);
    if om {
        OMITTED_READ.with(|f| f.set(true));
    }
    om
}

fn omitted_error() -> pliego_core::Error {
    pliego_core::Error::Unsupported("el flujo es muy grande y no está cargado en este modo".into())
}

/// Contenido del flujo con los filtros genéricos aplicados (se detiene ante un filtro de imagen).
pub(crate) fn decode_stream(s: &Stream) -> Result<Vec<u8>> {
    if omitted(s) {
        return Err(omitted_error());
    }
    let filters: Vec<Vec<u8>> = match s.filters() {
        Ok(f) => f.into_iter().map(<[u8]>::to_vec).collect(),
        Err(_) => return Ok(s.content.clone()),
    };
    let ok = filters.iter().take_while(|f| DECODABLE.contains(&f.as_slice())).count();
    if ok == filters.len() {
        return s.decompressed_content_with_limit(MAX_DECODED).map_err(err);
    }
    if ok == 0 {
        return Ok(s.content.clone());
    }
    let mut tmp = s.clone();
    tmp.dict.set(
        "Filter",
        L::Array(filters[..ok].iter().map(|f| L::Name(f.clone())).collect()),
    );
    if let Ok(L::Array(parms)) = s.dict.get(b"DecodeParms") {
        tmp.dict
            .set("DecodeParms", L::Array(parms.iter().take(ok).cloned().collect()));
    }
    tmp.decompressed_content_with_limit(MAX_DECODED).map_err(err)
}

impl Object {
    pub(crate) fn free(st: Option<Shared>, value: L) -> Object {
        Object {
            st,
            root: Root::Free(Rc::new(RefCell::new(value))),
            path: Vec::new(),
            indirect: false,
        }
    }

    pub(crate) fn reference(st: &Shared, id: ObjectId) -> Object {
        Object {
            st: Some(st.clone()),
            root: Root::Obj(id),
            path: Vec::new(),
            indirect: true,
        }
    }

    /// El objeto indirecto `id` ya resuelto (como lo devuelven `find_page`, `xref_object`…).
    pub(crate) fn resolved(st: &Shared, id: ObjectId) -> Object {
        Object {
            st: Some(st.clone()),
            root: Root::Obj(id),
            path: Vec::new(),
            indirect: false,
        }
    }

    pub(crate) fn trailer(st: &Shared) -> Object {
        Object {
            st: Some(st.clone()),
            root: Root::Trailer,
            path: Vec::new(),
            indirect: false,
        }
    }

    pub(crate) fn state(&self) -> Result<&Shared> {
        self.st
            .as_ref()
            .ok_or_else(|| pdf_err("el objeto no pertenece a ningún documento"))
    }

    /// Número de objeto si es una raíz indirecta sin ruta.
    pub(crate) fn obj_id(&self) -> Option<ObjectId> {
        match (&self.root, self.path.is_empty()) {
            (Root::Obj(id), true) => Some(*id),
            _ => None,
        }
    }

    pub(crate) fn same_doc(&self, other: &Object) -> bool {
        match (&self.st, &other.st) {
            (Some(a), Some(b)) => Rc::ptr_eq(a, b),
            _ => true,
        }
    }

    /// Lee el valor al que apunta el asa (resolviendo referencias).
    pub(crate) fn read<R>(&self, f: impl FnOnce(&L) -> Result<R>) -> Result<R> {
        match &self.root {
            Root::Free(rc) => {
                let g = rc.borrow();
                f(walk(&g, &self.path))
            }
            Root::Trailer => {
                let st = self.state()?.borrow();
                let tmp = L::Dictionary(st.doc.trailer.clone());
                f(walk(&tmp, &self.path))
            }
            Root::Obj(id) => {
                let st = self.state()?.borrow();
                let base = st.get(*id).unwrap_or(&NULL);
                f(walk(base, &self.path))
            }
        }
    }

    /// Modifica el valor al que apunta el asa y marca el objeto raíz como cambiado.
    pub(crate) fn modify<R>(&self, f: impl FnOnce(&mut L) -> Result<R>) -> Result<R> {
        match &self.root {
            Root::Free(rc) => {
                let mut g = rc.borrow_mut();
                let target = walk_mut(&mut g, &self.path).ok_or_else(|| pdf_err("la ruta del objeto ya no existe"))?;
                f(target)
            }
            Root::Trailer => {
                let mut st = self.state()?.borrow_mut();
                let mut tmp = L::Dictionary(std::mem::take(&mut st.doc.trailer));
                let r = match walk_mut(&mut tmp, &self.path) {
                    Some(t) => f(t),
                    None => Err(pdf_err("la ruta del objeto ya no existe")),
                };
                if let L::Dictionary(d) = tmp {
                    st.doc.trailer = d;
                }
                st.trailer_dirty = true;
                st.mark_changed();
                st.invalidate_pages();
                r
            }
            Root::Obj(id) => {
                let mut st = self.state()?.borrow_mut();
                let id = st.resolve_id(*id);
                let r = {
                    let base = st
                        .doc
                        .objects
                        .get_mut(&id)
                        .ok_or_else(|| pdf_err(format!("el objeto {} no existe", id.0)))?;
                    match walk_mut(base, &self.path) {
                        Some(t) => f(t),
                        None => Err(pdf_err("la ruta del objeto ya no existe")),
                    }
                };
                if r.is_ok() {
                    st.touch(id);
                    // Un cambio en un nodo del árbol de páginas, o en el catálogo, invalida la lista.
                    if super::tree::is_tree_node(st.doc.objects.get(&id)) {
                        st.invalidate_pages();
                    }
                }
                r
            }
        }
    }

    fn child(&self, step: Step, kid: Kid) -> Object {
        match kid {
            Kid::Ref(id) => match &self.st {
                Some(st) => Object::reference(st, id),
                // Un valor suelto sin documento no puede contener referencias útiles.
                None => Object::free(None, L::Reference(id)),
            },
            Kid::Direct => {
                let mut path = self.path.clone();
                path.push(step);
                Object {
                    st: self.st.clone(),
                    root: match &self.root {
                        Root::Obj(id) => {
                            let resolved = match &self.st {
                                Some(st) => st.borrow().resolve_id(*id),
                                None => *id,
                            };
                            Root::Obj(resolved)
                        }
                        other => other.clone(),
                    },
                    path,
                    indirect: false,
                }
            }
        }
    }

    /// Valor para guardar dentro de otro objeto: referencia si es indirecto, copia si es directo.
    pub(crate) fn embed_value(&self, target: &Option<Shared>) -> Result<L> {
        if let (Some(a), Some(b)) = (&self.st, target)
            && !Rc::ptr_eq(a, b)
            && self.obj_id().is_some()
        {
            return Err(Error::InvalidInput(
                "el objeto es de otro documento; cópialo antes con graft_object".into(),
            ));
        }
        // Un objeto indirecto ya resuelto (raíz sin ruta) se inserta como referencia.
        if let Some(id) = self.obj_id() {
            return Ok(L::Reference(id));
        }
        self.read(|o| match o {
            L::Stream(_) => Err(pdf_err("un flujo debe ser un objeto indirecto")),
            other => Ok(other.clone()),
        })
    }

    /// Copia profunda del contenido actual (flujos incluidos), sin seguir referencias.
    pub(crate) fn clone_value(&self) -> Result<L> {
        self.read(|o| Ok(o.clone()))
    }

    // --- Constructores sin documento (valores directos) ---

    pub fn new_null() -> Object {
        Object::free(None, L::Null)
    }

    pub fn new_bool(b: bool) -> Object {
        Object::free(None, L::Boolean(b))
    }

    pub fn new_int(i: i32) -> Result<Object> {
        Ok(Object::free(None, L::Integer(i64::from(i))))
    }

    pub fn new_real(f: f32) -> Result<Object> {
        Ok(Object::free(None, L::Real(f)))
    }

    /// Cadena PDF (literal) con los bytes de `s` en UTF-8.
    pub fn new_string(s: &str) -> Result<Object> {
        Ok(Object::free(
            None,
            L::String(s.as_bytes().to_vec(), StringFormat::Literal),
        ))
    }

    pub fn new_name(name: &str) -> Result<Object> {
        Ok(Object::free(None, L::Name(name.as_bytes().to_vec())))
    }

    /// Otra asa al mismo objeto.
    pub fn try_clone(&self) -> Result<Object> {
        Ok(self.clone())
    }

    // --- Tipos ---

    pub fn is_indirect(&self) -> Result<bool> {
        Ok(self.indirect)
    }
    pub fn is_null(&self) -> Result<bool> {
        self.read(|o| Ok(matches!(o, L::Null)))
    }
    pub fn is_bool(&self) -> Result<bool> {
        self.read(|o| Ok(matches!(o, L::Boolean(_))))
    }
    pub fn is_int(&self) -> Result<bool> {
        self.read(|o| Ok(matches!(o, L::Integer(_))))
    }
    pub fn is_real(&self) -> Result<bool> {
        self.read(|o| Ok(matches!(o, L::Real(_))))
    }
    /// Entero o real.
    pub fn is_number(&self) -> Result<bool> {
        self.read(|o| Ok(matches!(o, L::Integer(_) | L::Real(_))))
    }
    pub fn is_string(&self) -> Result<bool> {
        self.read(|o| Ok(matches!(o, L::String(..))))
    }
    pub fn is_name(&self) -> Result<bool> {
        self.read(|o| Ok(matches!(o, L::Name(_))))
    }
    pub fn is_array(&self) -> Result<bool> {
        self.read(|o| Ok(matches!(o, L::Array(_))))
    }
    /// Diccionario o flujo (un flujo también tiene diccionario).
    pub fn is_dict(&self) -> Result<bool> {
        self.read(|o| Ok(matches!(o, L::Dictionary(_) | L::Stream(_))))
    }
    pub fn is_stream(&self) -> Result<bool> {
        self.read(|o| Ok(matches!(o, L::Stream(_))))
    }

    // --- Valores ---

    pub fn as_bool(&self) -> Result<bool> {
        self.read(|o| match o {
            L::Boolean(b) => Ok(*b),
            _ => Err(pdf_err("se esperaba un booleano")),
        })
    }
    pub fn as_int(&self) -> Result<i32> {
        self.read(|o| match o {
            L::Integer(i) => Ok((*i).clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32),
            L::Real(f) => Ok((*f + 0.5) as i32), // como pdf_to_int de MuPDF
            _ => Err(pdf_err("se esperaba un entero")),
        })
    }
    /// Entero o real como `f32`.
    pub fn as_float(&self) -> Result<f32> {
        self.read(|o| match o {
            L::Integer(i) => Ok(*i as f32),
            L::Real(f) => Ok(*f),
            _ => Err(pdf_err("se esperaba un número")),
        })
    }
    /// Número de objeto de una referencia indirecta (o de un objeto indirecto ya resuelto).
    pub fn as_indirect(&self) -> Result<i32> {
        // Como MuPDF: un objeto ya resuelto (o directo) devuelve 0.
        match (self.indirect, self.obj_id()) {
            (true, Some(id)) => Ok(id.0 as i32),
            _ => Ok(0),
        }
    }
    /// Bytes del nombre, sin la barra.
    pub fn as_name(&self) -> Result<Vec<u8>> {
        self.read(|o| match o {
            L::Name(n) => Ok(n.clone()),
            _ => Err(pdf_err("se esperaba un nombre")),
        })
    }
    /// Cadena como UTF-8 (falla si los bytes no lo son; usa [`Object::as_bytes`] entonces).
    pub fn as_string(&self) -> Result<String> {
        self.read(|o| match o {
            // UTF-8 tal cual; si no lo es (UTF-16 con BOM, PDFDocEncoding), se decodifica como texto PDF.
            L::String(bytes, _) => Ok(String::from_utf8(bytes.clone())
                .ok()
                .or_else(|| super::document::decode_text(o))
                .unwrap_or_default()),
            _ => Err(pdf_err("se esperaba una cadena")),
        })
    }
    /// Bytes crudos de una cadena PDF.
    pub fn as_bytes(&self) -> Result<Vec<u8>> {
        self.read(|o| match o {
            L::String(b, _) => Ok(b.clone()),
            _ => Err(pdf_err("se esperaba una cadena")),
        })
    }

    /// Sigue una referencia indirecta hasta el objeto real (`None` si apunta a la nada).
    pub fn resolve(&self) -> Result<Option<Object>> {
        match (&self.root, self.path.is_empty()) {
            (Root::Obj(id), true) => {
                let st = self.state()?;
                let exists = st.borrow().get(*id).is_some();
                Ok(exists.then(|| Object::resolved(st, *id)))
            }
            _ => Ok(Some(self.clone())),
        }
    }

    // --- Flujos ---

    /// Contenido del flujo, ya descomprimido.
    pub fn read_stream(&self) -> Result<Vec<u8>> {
        self.read(|o| match o {
            L::Stream(s) => decode_stream(s),
            _ => Err(pdf_err("el objeto no es un flujo")),
        })
    }
    /// Contenido del flujo tal como está en el archivo (con sus filtros).
    pub fn read_raw_stream(&self) -> Result<Vec<u8>> {
        self.read(|o| match o {
            L::Stream(s) if omitted(s) => Err(omitted_error()),
            L::Stream(s) => Ok(s.content.clone()),
            _ => Err(pdf_err("el objeto no es un flujo")),
        })
    }
    /// Sustituye el flujo por `data` **sin comprimir** (quita los filtros del diccionario; el guardado
    /// comprime si se pide).
    pub fn write_stream(&mut self, data: &[u8]) -> Result<()> {
        self.set_stream(data, true)
    }
    /// Sustituye el flujo por `data` tal cual, conservando los filtros del diccionario (`data` ya está codificado).
    pub fn write_raw_stream(&mut self, data: &[u8]) -> Result<()> {
        self.set_stream(data, false)
    }

    fn set_stream(&self, data: &[u8], plain: bool) -> Result<()> {
        if !self.path.is_empty() {
            return Err(pdf_err("solo un objeto indirecto puede tener flujo"));
        }
        self.modify(|o| {
            if let L::Dictionary(d) = o {
                let dict = std::mem::take(d);
                *o = L::Stream(Stream::new(dict, Vec::new()));
            }
            match o {
                L::Stream(s) => {
                    if plain {
                        s.set_plain_content(data.to_vec());
                    } else {
                        s.set_content(data.to_vec());
                    }
                    Ok(())
                }
                _ => Err(pdf_err("el objeto no es un flujo ni un diccionario")),
            }
        })
    }

    // --- Matrices ---

    /// Elemento `index` (desde 0) de una matriz PDF; `None` si no existe.
    pub fn get_array(&self, index: i32) -> Result<Option<Object>> {
        if index < 0 {
            return Ok(None);
        }
        let i = index as usize;
        let kid = self.read(|o| {
            Ok(match o {
                L::Array(a) => a.get(i).map(kid_of),
                _ => None,
            })
        })?;
        Ok(kid.map(|k| self.child(Step::Idx(i), k)))
    }
    /// Número de elementos de una matriz.
    pub fn len(&self) -> Result<usize> {
        self.read(|o| {
            Ok(match o {
                L::Array(a) => a.len(),
                _ => 0,
            })
        })
    }
    pub fn is_empty(&self) -> Result<bool> {
        Ok(self.len()? == 0)
    }
    /// Elementos de la matriz (asas nuevas, en orden).
    pub fn array_iter(&self) -> Result<Vec<Object>> {
        let kids: Vec<Kid> = self.read(|o| {
            Ok(match o {
                L::Array(a) => a.iter().map(kid_of).collect(),
                _ => Vec::new(),
            })
        })?;
        Ok(kids
            .into_iter()
            .enumerate()
            .map(|(i, k)| self.child(Step::Idx(i), k))
            .collect())
    }
    pub fn array_push(&mut self, value: Object) -> Result<()> {
        let v = value.embed_value(&self.st)?;
        self.modify(|o| match o {
            L::Array(a) => {
                a.push(v);
                Ok(())
            }
            _ => Err(pdf_err("el objeto no es una matriz")),
        })
    }
    pub fn array_put(&mut self, index: i32, value: Object) -> Result<()> {
        let v = value.embed_value(&self.st)?;
        self.modify(|o| match o {
            L::Array(a) => {
                let i = usize::try_from(index).map_err(|_| pdf_err("índice negativo"))?;
                if i < a.len() {
                    a[i] = v;
                } else {
                    // MuPDF rellena con `null` hasta llegar al índice.
                    a.resize(i, L::Null);
                    a.push(v);
                }
                Ok(())
            }
            _ => Err(pdf_err("el objeto no es una matriz")),
        })
    }
    pub fn array_delete(&mut self, index: i32) -> Result<()> {
        self.modify(|o| match o {
            L::Array(a) => {
                let i = usize::try_from(index).map_err(|_| pdf_err("índice negativo"))?;
                if i < a.len() {
                    a.remove(i);
                }
                Ok(())
            }
            _ => Err(pdf_err("el objeto no es una matriz")),
        })
    }
    /// Copia superficial de una matriz (objeto nuevo, directo).
    pub fn copy_array(&self) -> Result<Object> {
        let v = self.read(|o| match o {
            L::Array(a) => Ok(L::Array(a.clone())),
            _ => Err(pdf_err("el objeto no es una matriz")),
        })?;
        Ok(Object::free(self.st.clone(), v))
    }

    // --- Diccionarios ---

    /// Valor de `key` (sin barra). `None` si no existe. Una referencia indirecta se devuelve como asa
    /// indirecta, que se resuelve sola al leerla.
    pub fn get_dict(&self, key: &str) -> Result<Option<Object>> {
        let k = key.as_bytes();
        let kid = self.read(|o| {
            Ok(match o {
                L::Dictionary(d) => d.get(k).ok().map(kid_of),
                L::Stream(s) => s.dict.get(k).ok().map(kid_of),
                _ => None,
            })
        })?;
        Ok(kid.map(|kid| self.child(Step::Key(k.to_vec()), kid)))
    }
    /// Como [`Object::get_dict`] pero sube por `/Parent` (atributos heredables de página: `Resources`, `MediaBox`…).
    pub fn get_dict_inheritable(&self, key: &str) -> Result<Option<Object>> {
        let mut cur = self.clone();
        for _ in 0..64 {
            if let Some(v) = cur.get_dict(key)? {
                return Ok(Some(v));
            }
            match cur.get_dict("Parent")? {
                Some(p) if p.is_dict()? => cur = p,
                _ => break,
            }
        }
        Ok(None)
    }
    pub fn dict_len(&self) -> Result<usize> {
        self.read(|o| {
            Ok(match o {
                L::Dictionary(d) => d.len(),
                L::Stream(s) => s.dict.len(),
                _ => 0,
            })
        })
    }
    /// Clave número `idx` (nombre sin barra).
    pub fn get_dict_key(&self, idx: i32) -> Result<Option<String>> {
        let i = usize::try_from(idx).unwrap_or(usize::MAX);
        self.read(|o| {
            let d = match o {
                L::Dictionary(d) => d,
                L::Stream(s) => &s.dict,
                _ => return Ok(None),
            };
            Ok(d.iter().nth(i).map(|(k, _)| String::from_utf8_lossy(k).into_owned()))
        })
    }
    /// Valor número `idx`.
    pub fn get_dict_val(&self, idx: i32) -> Result<Option<Object>> {
        let i = usize::try_from(idx).unwrap_or(usize::MAX);
        let found = self.read(|o| {
            let d = match o {
                L::Dictionary(d) => d,
                L::Stream(s) => &s.dict,
                _ => return Ok(None),
            };
            Ok(d.iter().nth(i).map(|(k, v)| (k.clone(), kid_of(v))))
        })?;
        Ok(found.map(|(k, kid)| self.child(Step::Key(k), kid)))
    }
    /// Pares `(clave, valor)` del diccionario, en orden del archivo.
    pub fn dict_iter(&self) -> Result<Vec<(String, Object)>> {
        let entries: Vec<(Vec<u8>, Kid)> = self.read(|o| {
            let d = match o {
                L::Dictionary(d) => d,
                L::Stream(s) => &s.dict,
                _ => return Ok(Vec::new()),
            };
            Ok(d.iter().map(|(k, v)| (k.clone(), kid_of(v))).collect())
        })?;
        Ok(entries
            .into_iter()
            .map(|(k, kid)| {
                let name = String::from_utf8_lossy(&k).into_owned();
                (name, self.child(Step::Key(k), kid))
            })
            .collect())
    }
    /// Pone `key = value`. Ver la advertencia del módulo sobre modificar `value` después.
    pub fn dict_put(&mut self, key: &str, value: Object) -> Result<()> {
        let v = value.embed_value(&self.st)?;
        self.dict_set(key.as_bytes(), v)
    }

    pub(crate) fn dict_set(&self, key: &[u8], v: L) -> Result<()> {
        self.modify(|o| match o {
            L::Dictionary(d) => {
                d.set(key.to_vec(), v);
                Ok(())
            }
            L::Stream(s) => {
                s.dict.set(key.to_vec(), v);
                Ok(())
            }
            _ => Err(pdf_err("el objeto no es un diccionario")),
        })
    }

    pub fn dict_delete(&mut self, key: &str) -> Result<()> {
        self.modify(|o| match o {
            L::Dictionary(d) => {
                d.remove(key.as_bytes());
                Ok(())
            }
            L::Stream(s) => {
                s.dict.remove(key.as_bytes());
                Ok(())
            }
            _ => Err(pdf_err("el objeto no es un diccionario")),
        })
    }
    /// Copia superficial de un diccionario (objeto nuevo, directo).
    pub fn copy_dict(&self) -> Result<Object> {
        let v = self.read(|o| match o {
            L::Dictionary(d) => Ok(L::Dictionary(d.clone())),
            L::Stream(s) => Ok(L::Dictionary(s.dict.clone())),
            _ => Err(pdf_err("el objeto no es un diccionario")),
        })?;
        Ok(Object::free(self.st.clone(), v))
    }

    // --- Ayudas tipadas (evitan crear objetos intermedios) ---

    pub fn dict_put_name(&mut self, key: &str, name: &str) -> Result<()> {
        self.dict_set(key.as_bytes(), L::Name(name.as_bytes().to_vec()))
    }
    pub fn dict_put_int(&mut self, key: &str, v: i32) -> Result<()> {
        self.dict_set(key.as_bytes(), L::Integer(i64::from(v)))
    }
    pub fn dict_put_real(&mut self, key: &str, v: f32) -> Result<()> {
        self.dict_set(key.as_bytes(), L::Real(v))
    }
    pub fn dict_put_bool(&mut self, key: &str, v: bool) -> Result<()> {
        self.dict_set(key.as_bytes(), L::Boolean(v))
    }
    /// Entero de `key`, si existe y es numérico.
    pub fn dict_get_int(&self, key: &str) -> Result<Option<i32>> {
        match self.get_dict(key)? {
            Some(o) if o.is_number()? => Ok(Some(o.as_int().or_else(|_| o.as_float().map(|f| f as i32))?)),
            _ => Ok(None),
        }
    }
    /// Nombre de `key` como cadena, si existe y es un nombre.
    pub fn dict_get_name(&self, key: &str) -> Result<Option<String>> {
        match self.get_dict(key)? {
            Some(o) if o.is_name()? => Ok(Some(String::from_utf8_lossy(&o.as_name()?).into_owned())),
            _ => Ok(None),
        }
    }

    // --- Páginas ---

    /// Matriz que lleva el espacio de usuario de esta página (objeto `/Page`) al espacio de página de salida
    /// (origen arriba a la izquierda, con la rotación aplicada).
    pub fn page_ctm(&self) -> Result<Matrix> {
        super::page::page_ctm(self)
    }

    /// Sintaxis PDF del objeto (para depurar y para pruebas).
    pub fn to_pdf_string(&self) -> String {
        if self.indirect
            && let Root::Obj(id) = &self.root
        {
            return format!("{} {} R", id.0, id.1);
        }
        self.read(|o| Ok(object_to_string(o))).unwrap_or_else(|_| "null".into())
    }

    /// Objeto a partir de su sintaxis PDF, sin documento.
    pub(crate) fn parse_in(st: &Option<Shared>, src: &str) -> Result<Object> {
        Ok(Object::free(st.clone(), parse_object(src)?))
    }

    // --- Datos planos ---

    /// Copia el objeto a datos planos, hasta `max_depth` niveles (más abajo queda `Value::Null`). Las referencias
    /// indirectas **no** se siguen: quedan como [`Value::Ref`]. Los flujos se leen como su diccionario.
    pub fn to_value(&self, max_depth: usize) -> Result<Value> {
        if self.indirect {
            return Ok(Value::Ref(self.as_indirect()?));
        }
        if max_depth == 0 {
            return Ok(Value::Null);
        }
        self.read(|o| Ok(to_value(o, max_depth)))
    }
}

fn to_value(o: &L, depth: usize) -> Value {
    if depth == 0 {
        return Value::Null;
    }
    match o {
        L::Reference(id) => Value::Ref(id.0 as i32),
        L::Null => Value::Null,
        L::Boolean(b) => Value::Bool(*b),
        L::Integer(i) => Value::Int((*i).clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32),
        L::Real(f) => Value::Real(*f),
        L::Name(n) => Value::Name(String::from_utf8_lossy(n).into_owned()),
        L::String(b, _) => Value::String(b.clone()),
        L::Array(a) => Value::Array(a.iter().map(|v| to_value(v, depth - 1)).collect()),
        L::Dictionary(d) => dict_value(d, depth),
        L::Stream(s) => dict_value(&s.dict, depth),
    }
}

fn dict_value(d: &Dictionary, depth: usize) -> Value {
    Value::Dict(
        d.iter()
            .map(|(k, v)| (String::from_utf8_lossy(k).into_owned(), to_value(v, depth - 1)))
            .collect(),
    )
}

impl fmt::Debug for Object {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Object({})", self.to_pdf_string())
    }
}

impl fmt::Display for Object {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_pdf_string())
    }
}

impl From<bool> for Object {
    fn from(b: bool) -> Self {
        Object::new_bool(b)
    }
}

impl TryFrom<i32> for Object {
    type Error = Error;
    fn try_from(v: i32) -> Result<Self> {
        Object::new_int(v)
    }
}

impl TryFrom<f32> for Object {
    type Error = Error;
    fn try_from(v: f32) -> Result<Self> {
        Object::new_real(v)
    }
}
