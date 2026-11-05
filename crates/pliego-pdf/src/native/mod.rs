//! Modelo de objetos y guardado **propios** (lopdf), con la misma forma que la API de `pliego-pdf` sobre MuPDF.
//!
//! Es el bloque 2 de la ola 5 (W36): el motor nuevo del *modelo de documento* detrás de la capa. Contiene
//! [`Object`], [`PdfDocument`], [`PdfPage`] y [`GraftMap`] con los mismos nombres y firmas que los de la raíz de
//! la capa, de modo que el cambio de motor sea `pub use native::*` cuando W37 (PDFium) aporte render y texto.
//! Mientras tanto MuPDF sigue siendo el motor de la capa y esto se compara con él (pruebas de paridad,
//! `docs/qa/arnes-de-motores.md`).
//!
//! # Qué hay y qué no
//!
//! - Hay: apertura (con contraseña), objetos, flujos, páginas (crear, borrar, seleccionar, mover, duplicar,
//!   insertar de otro PDF), etiquetas, marcadores, adjuntos, capas, metadatos, guardado completo (compresión,
//!   limpieza, deduplicado, ASCII, cifrado) e **incremental** (solo los objetos cambiados).
//! - No hay (otros flujos): render, texto, búsqueda (W37); anotaciones, widgets, fuentes, imágenes (W39);
//!   redacción (W38); conversión (W40). Las páginas de este módulo son solo de objetos.
//!
//! # Diseño del asa [`Object`]
//!
//! lopdf guarda los objetos como valores dentro de un mapa; no hay punteros compartidos como en MuPDF. Un
//! [`Object`] es por tanto una **ruta**: documento + objeto raíz (o el `trailer`, o un valor suelto) + pasos
//! (`/Clave` o índice). Leer sigue la ruta; escribir la sigue en modo mutable y marca el objeto raíz como
//! cambiado (lo necesita el guardado incremental). Consecuencias, ya anticipadas en el contrato de
//! [`crate::Object`]:
//!
//! - Insertar un objeto con `dict_put`/`array_push` **copia** su valor (o guarda la referencia si es indirecto).
//! - Una asa a un elemento directo deja de ser válida si se borra o reordena su contenedor.
//! - Nada es `Send` (comparten `Rc<RefCell<…>>`).

mod document;
mod graft;
mod object;
mod outline;
mod page;
mod parse;
mod raster;
mod save;
mod tree;

#[cfg(test)]
mod tests;

pub use document::PdfDocument;
pub use graft::GraftMap;
pub use object::Object;
pub use page::PdfPage;
pub use raster::Raster;

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use lopdf::{Dictionary, ObjectId};

use pliego_core::Error;

/// Estado compartido por las asas de un documento.
pub(crate) struct State {
    pub(crate) doc: lopdf::Document,
    /// Cómo estaba el archivo al abrirlo (para el guardado incremental y para `authenticate`).
    pub(crate) base: Option<Base>,
    /// Objetos nuevos o modificados desde la apertura.
    pub(crate) dirty: BTreeSet<ObjectId>,
    /// Objetos borrados desde la apertura.
    pub(crate) deleted: BTreeSet<ObjectId>,
    /// El `trailer` cambió.
    pub(crate) trailer_dirty: bool,
    /// Hay cambios sin guardar.
    pub(crate) unsaved: bool,
    /// El documento está cifrado y no se ha podido descifrar (falta contraseña).
    pub(crate) locked: bool,
    /// Lista de páginas (ids de objeto) en orden; se recalcula al cambiar el árbol.
    pub(crate) pages_cache: Option<Vec<ObjectId>>,
}

/// Datos del archivo de origen.
pub(crate) struct Base {
    pub(crate) bytes: Vec<u8>,
    /// `trailer` tal como quedó tras descifrar.
    pub(crate) trailer: Dictionary,
    pub(crate) xref_start: usize,
    pub(crate) xref_type: lopdf::xref::XrefType,
    pub(crate) max_id: u32,
    pub(crate) encryption_state: Option<lopdf::EncryptionState>,
    /// lopdf tuvo que reconstruir la tabla de referencias: el archivo no admite añadidos al final.
    pub(crate) repaired: bool,
}

pub(crate) type Shared = Rc<RefCell<State>>;

/// Convierte un error de lopdf en el error común de Pliego.
pub(crate) fn err(e: lopdf::Error) -> Error {
    Error::Pdf(e.to_string())
}

pub(crate) fn pdf_err(msg: impl Into<String>) -> Error {
    Error::Pdf(msg.into())
}

impl State {
    pub(crate) fn touch(&mut self, id: ObjectId) {
        self.dirty.insert(id);
        self.deleted.remove(&id);
        self.unsaved = true;
    }

    pub(crate) fn invalidate_pages(&mut self) {
        self.pages_cache = None;
    }

    /// Reserva un número de objeto nuevo.
    pub(crate) fn new_id(&mut self) -> ObjectId {
        self.doc.max_id += 1;
        (self.doc.max_id, 0)
    }

    pub(crate) fn add(&mut self, obj: lopdf::Object) -> ObjectId {
        let id = self.new_id();
        self.doc.objects.insert(id, obj);
        self.touch(id);
        id
    }

    /// Sigue una cadena de referencias hasta el último id existente (con límite contra ciclos).
    pub(crate) fn resolve_id(&self, mut id: ObjectId) -> ObjectId {
        for _ in 0..32 {
            match self.doc.objects.get(&id) {
                Some(lopdf::Object::Reference(next)) => id = *next,
                _ => break,
            }
        }
        id
    }

    /// Objeto `id` resuelto (sigue referencias). `None` si no existe.
    pub(crate) fn get(&self, id: ObjectId) -> Option<&lopdf::Object> {
        self.doc.objects.get(&self.resolve_id(id))
    }

    /// Como `get` pero también sigue una referencia que aparezca como valor.
    pub(crate) fn deref<'a>(&'a self, o: &'a lopdf::Object) -> Option<&'a lopdf::Object> {
        match o {
            lopdf::Object::Reference(id) => self.get(*id),
            other => Some(other),
        }
    }

    /// Diccionario de un objeto (de un diccionario o del diccionario de un flujo).
    pub(crate) fn dict_of<'a>(&'a self, o: &'a lopdf::Object) -> Option<&'a Dictionary> {
        match self.deref(o)? {
            lopdf::Object::Dictionary(d) => Some(d),
            lopdf::Object::Stream(s) => Some(&s.dict),
            _ => None,
        }
    }
}

/// Valor nulo con vida de `static`, para leer objetos que no existen como `null`.
pub(crate) static NULL: lopdf::Object = lopdf::Object::Null;
