//! [`PdfDocument`]: documento PDF sobre lopdf (modelo de objetos, páginas, etiquetas, adjuntos, capas).

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::path::Path;
use std::rc::Rc;

use lopdf::Object as L;
use lopdf::{Dictionary, LoadOptions, ObjectId, Stream, StringFormat};
use pliego_core::{Error, Result};

use super::graft::{self, GraftMap};
use super::object::{Object, decode_stream};
use super::page::PdfPage;
use super::{Base, Shared, State, err, outline, pdf_err, tree};
use crate::geom::Size;
use crate::links::LinkDestination;
use crate::object::Value;
use crate::options::{
    EmbeddedFileInfo, EmbeddedFileOptions, InsertPdfOptions, InsertPdfResult, InsertPosition, MetadataName,
    OptionalContentGroup, Outline, PageLabelRule, PageLabelStyle, PageRange, PageSelection, Permission,
};

/// Documento PDF. A diferencia de [`crate::PdfDocument`] no dibuja ni extrae texto (W37).
pub struct PdfDocument {
    pub(crate) st: Shared,
}

impl Default for PdfDocument {
    fn default() -> Self {
        Self::new()
    }
}

impl PdfDocument {
    pub(crate) fn state(&self) -> &Shared {
        &self.st
    }

    fn from_state(state: State) -> Self {
        Self {
            st: Rc::new(RefCell::new(state)),
        }
    }

    pub(crate) fn guard(&self) -> Result<()> {
        if self.st.borrow().locked {
            Err(Error::Password)
        } else {
            Ok(())
        }
    }

    /// Documento vacío (sin páginas).
    pub fn new() -> Self {
        let mut doc = lopdf::Document::new();
        doc.version = "1.7".into();
        doc.reference_table.cross_reference_type = lopdf::xref::XrefType::CrossReferenceTable;
        let mut state = State {
            doc,
            base: None,
            dirty: BTreeSet::new(),
            deleted: BTreeSet::new(),
            trailer_dirty: true,
            unsaved: false,
            no_incremental: false,
            locked: false,
            pages_cache: None,
            annot_dirty: BTreeSet::new(),
        };
        let mut pages = Dictionary::new();
        pages.set("Type", L::Name(b"Pages".to_vec()));
        pages.set("Kids", L::Array(Vec::new()));
        pages.set("Count", 0i64);
        let pages_id = state.add(L::Dictionary(pages));
        let mut cat = Dictionary::new();
        cat.set("Type", L::Name(b"Catalog".to_vec()));
        cat.set("Pages", L::Reference(pages_id));
        let cat_id = state.add(L::Dictionary(cat));
        state.doc.trailer.set("Root", L::Reference(cat_id));
        // Como en MuPDF, un documento nuevo cuenta como «con cambios sin guardar».
        state.unsaved = true;
        Self::from_state(state)
    }

    /// Abre un PDF. No pide contraseña (ver [`PdfDocument::open_with_password`]). Falla si no es un PDF.
    pub fn open(path: impl AsRef<Path>) -> Result<PdfDocument> {
        Self::from_vec(read_file(path.as_ref())?, None, false)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<PdfDocument> {
        Self::from_vec(bytes.to_vec(), None, false)
    }

    /// Abre y desbloquea. [`Error::Password`] si hace falta contraseña y falta o es incorrecta.
    pub fn open_with_password(path: impl AsRef<Path>, password: Option<&str>) -> Result<PdfDocument> {
        Self::from_vec(read_file(path.as_ref())?, password, true)
    }

    pub fn from_bytes_with_password(bytes: &[u8], password: Option<&str>) -> Result<PdfDocument> {
        Self::from_vec(bytes.to_vec(), password, true)
    }

    fn from_vec(bytes: Vec<u8>, password: Option<&str>, require_unlocked: bool) -> Result<PdfDocument> {
        let bytes = indirect_encrypt(bytes);
        let (doc, locked) = load(&bytes, None)?;
        let mut state = new_state(bytes, doc, locked);
        if locked && let Some(pw) = password {
            // Segunda oportunidad con la contraseña indicada.
            let loaded = match load(&state.base.as_ref().expect("base").bytes, Some(pw)) {
                Ok(r) => Some(r),
                Err(Error::Password) => None,
                Err(e) => return Err(e),
            };
            if let Some((doc, false)) = loaded {
                let bytes = state.base.take().map(|b| b.bytes).unwrap_or_default();
                state = new_state(bytes, doc, false);
            }
        }
        if require_unlocked && state.locked {
            return Err(Error::Password);
        }
        Ok(Self::from_state(state))
    }

    pub fn needs_password(&self) -> Result<bool> {
        Ok(self.st.borrow().locked)
    }

    /// `true` si la contraseña es válida (de usuario o de propietario).
    pub fn authenticate(&mut self, password: &str) -> Result<bool> {
        if !self.st.borrow().locked {
            return Ok(true);
        }
        let bytes = match &self.st.borrow().base {
            Some(b) => b.bytes.clone(),
            None => return Ok(false),
        };
        let (doc, still) = match load(&bytes, Some(password)) {
            Ok(r) => r,
            Err(Error::Password) => return Ok(false),
            Err(e) => return Err(e),
        };
        if still {
            return Ok(false);
        }
        *self.st.borrow_mut() = new_state(bytes, doc, false);
        Ok(true)
    }

    // --- Páginas ---

    pub(crate) fn page_ids(&self) -> Vec<ObjectId> {
        tree::page_list(&mut self.st.borrow_mut())
    }

    pub fn page_count(&self) -> Result<i32> {
        self.guard()?;
        Ok(self.page_ids().len() as i32)
    }

    fn page_id(&self, page_no: i32) -> Result<ObjectId> {
        self.guard()?;
        let ids = self.page_ids();
        usize::try_from(page_no)
            .ok()
            .and_then(|i| ids.get(i).copied())
            .ok_or_else(|| pdf_err(format!("la página {} no existe (hay {})", page_no + 1, ids.len())))
    }

    pub fn load_pdf_page(&self, page_no: i32) -> Result<PdfPage> {
        Ok(PdfPage::new(self.find_page(page_no)?))
    }

    /// Objeto `/Page` de la página `page_no` (desde 0).
    pub fn find_page(&self, page_no: i32) -> Result<Object> {
        let id = self.page_id(page_no)?;
        Ok(Object::reference(&self.st, id))
    }

    /// Número de página (desde 0) de un objeto `/Page`.
    pub fn lookup_page_number(&self, page_obj: &Object) -> Result<i32> {
        self.guard()?;
        let id = page_obj
            .obj_id()
            .map(|id| self.st.borrow().resolve_id(id))
            .ok_or_else(|| pdf_err("el objeto no es una página"))?;
        self.page_ids()
            .iter()
            .position(|p| *p == id)
            .map(|i| i as i32)
            .ok_or_else(|| pdf_err("el objeto no es una página del documento"))
    }

    /// Añade una página en blanco al final.
    pub fn new_page(&mut self, size: Size) -> Result<PdfPage> {
        self.new_page_at(-1, size)
    }

    /// Añade una página en blanco en la posición `page_no` (desde 0; `-1` = al final).
    pub fn new_page_at(&mut self, page_no: i32, size: Size) -> Result<PdfPage> {
        self.guard()?;
        let mut st = self.st.borrow_mut();
        let n = tree::page_list(&mut st).len();
        let index = if page_no < 0 { n } else { (page_no as usize).min(n) };
        let mut d = Dictionary::new();
        d.set("Type", L::Name(b"Page".to_vec()));
        d.set(
            "MediaBox",
            L::Array(vec![
                L::Integer(0),
                L::Integer(0),
                L::Real(size.width),
                L::Real(size.height),
            ]),
        );
        d.set("Resources", L::Dictionary(Dictionary::new()));
        let id = st.add(L::Dictionary(d));
        tree::insert_page(&mut st, index, id)?;
        drop(st);
        Ok(PdfPage::new(Object::reference(&self.st, id)))
    }

    /// Inserta un objeto `/Page` ya construido en la posición `page_no`.
    pub fn insert_page(&mut self, page_no: i32, page: &Object) -> Result<()> {
        self.guard()?;
        let mut st = self.st.borrow_mut();
        let id = match page.obj_id() {
            Some(id) => st.resolve_id(id),
            None => {
                // Un diccionario suelto: se añade como objeto nuevo.
                let v = page.clone_value()?;
                st.add(v)
            }
        };
        let n = tree::page_list(&mut st).len();
        let index = if page_no < 0 { n } else { (page_no as usize).min(n) };
        tree::insert_page(&mut st, index, id)
    }

    pub fn delete_page(&mut self, page_no: i32) -> Result<()> {
        self.guard()?;
        let i = usize::try_from(page_no).map_err(|_| pdf_err("número de página negativo"))?;
        tree::remove_page(&mut self.st.borrow_mut(), i)
    }

    pub fn delete_pages(&mut self, selection: impl Into<PageSelection>) -> Result<()> {
        self.guard()?;
        let count = self.page_count()?.max(0) as usize;
        let mut idx = selection.into().validated_indices(count)?;
        reject_duplicates(&idx)?;
        idx.sort_unstable();
        let mut st = self.st.borrow_mut();
        for i in idx.into_iter().rev() {
            tree::remove_page(&mut st, i)?;
        }
        Ok(())
    }

    /// Deja solo las páginas indicadas (conservan el orden que tenían en el documento).
    pub fn select_pages(&mut self, selection: impl Into<PageSelection>) -> Result<()> {
        self.guard()?;
        let count = self.page_count()?.max(0) as usize;
        let mut idx = selection.into().validated_indices(count)?;
        reject_duplicates(&idx)?;
        // Como MuPDF: se conservan las páginas indicadas en el orden que ya tenían en el documento.
        idx.sort_unstable();
        tree::rearrange(&mut self.st.borrow_mut(), &idx)
    }

    pub fn move_page(&mut self, source_index: usize, target_index: usize) -> Result<()> {
        self.guard()?;
        let count = self.page_count()?.max(0) as usize;
        if source_index >= count || target_index >= count {
            return Err(Error::InvalidInput(format!(
                "la página {} no existe (hay {count})",
                source_index.max(target_index) + 1
            )));
        }
        let mut order: Vec<usize> = (0..count).collect();
        let p = order.remove(source_index);
        order.insert(target_index, p);
        tree::rearrange(&mut self.st.borrow_mut(), &order)
    }

    pub fn duplicate_page(&mut self, source_index: usize) -> Result<InsertPdfResult> {
        self.copy_page(source_index, InsertPosition::After(source_index))
    }

    pub fn copy_page(&mut self, source_index: usize, target: InsertPosition) -> Result<InsertPdfResult> {
        self.guard()?;
        let count = self.page_count()?.max(0) as usize;
        let src = *self
            .page_ids()
            .get(source_index)
            .ok_or_else(|| pdf_err(format!("la página {} no existe (hay {count})", source_index + 1)))?;
        let index = position_index(target, count)?;
        let mut st = self.st.borrow_mut();
        tree::flatten_inheritable(&mut st, src);
        let copy = st
            .doc
            .objects
            .get(&src)
            .cloned()
            .ok_or_else(|| pdf_err("página sin objeto"))?;
        let id = st.add(copy);
        graft::clone_page_annotations(&mut st, id)?;
        tree::insert_page(&mut st, index, id)?;
        Ok(InsertPdfResult {
            inserted_pages: PageRange::new(index, index + 1),
            page_count: 1,
        })
    }

    /// Copia páginas de `src` (con su contenido, recursos, enlaces, anotaciones y campos).
    pub fn insert_pdf(&mut self, src: &PdfDocument, options: InsertPdfOptions) -> Result<InsertPdfResult> {
        self.guard()?;
        src.guard()?;
        let count = src.page_count()?.max(0) as usize;
        let idx = options.source_pages.validated_indices(count)?;
        let dst_count = self.page_count()?.max(0) as usize;
        let at = position_index(options.target, dst_count)?;
        graft::insert_pages(self, src, &idx, at, options.rotate)?;
        Ok(InsertPdfResult {
            inserted_pages: PageRange::new(at, at + idx.len()),
            page_count: idx.len(),
        })
    }

    // --- Etiquetas de página ---

    /// Etiqueta de la página `page_index` (desde 0), p. ej. «iv» o «A-3».
    pub fn page_label(&self, page_index: usize) -> Result<String> {
        self.guard()?;
        let st = self.st.borrow();
        let rules = label_rules(&st);
        let Some(rule) = rules.iter().rev().find(|r| r.index <= page_index) else {
            return Ok(String::new());
        };
        let n = i64::from(rule.start) + (page_index - rule.index) as i64;
        let body = match rule.style {
            PageLabelStyle::None => String::new(),
            PageLabelStyle::Decimal => n.to_string(),
            PageLabelStyle::UpperRoman => roman(n, true),
            PageLabelStyle::LowerRoman => roman(n, false),
            PageLabelStyle::UpperAlpha => alpha(n, true),
            PageLabelStyle::LowerAlpha => alpha(n, false),
        };
        Ok(format!("{}{}", rule.prefix, body))
    }

    pub fn set_page_label_rule(&mut self, rule: PageLabelRule) -> Result<()> {
        self.guard()?;
        let mut st = self.st.borrow_mut();
        let mut rules = label_rules(&st);
        rules.retain(|r| r.index != rule.index);
        rules.push(rule);
        rules.sort_by_key(|r| r.index);
        let mut nums = Vec::new();
        for r in rules {
            let mut d = Dictionary::new();
            let style: Option<&[u8]> = match r.style {
                PageLabelStyle::None => None,
                PageLabelStyle::Decimal => Some(b"D"),
                PageLabelStyle::UpperRoman => Some(b"R"),
                PageLabelStyle::LowerRoman => Some(b"r"),
                PageLabelStyle::UpperAlpha => Some(b"A"),
                PageLabelStyle::LowerAlpha => Some(b"a"),
            };
            if let Some(s) = style {
                d.set("S", L::Name(s.to_vec()));
            }
            if !r.prefix.is_empty() {
                d.set("P", text_string(&r.prefix));
            }
            if r.start != 1 {
                d.set("St", i64::from(r.start));
            }
            nums.push(L::Integer(r.index as i64));
            nums.push(L::Dictionary(d));
        }
        let cat = catalog_id(&st)?;
        let mut labels = Dictionary::new();
        labels.set("Nums", L::Array(nums));
        set_dict_entry(&mut st, cat, "PageLabels", L::Dictionary(labels))
    }

    // --- Estructura del archivo ---

    /// Diccionario `trailer`.
    pub fn trailer(&self) -> Result<Object> {
        Ok(Object::trailer(&self.st))
    }

    /// Diccionario `/Root` (catálogo).
    pub fn catalog(&self) -> Result<Object> {
        self.guard()?;
        let id = catalog_id(&self.st.borrow())?;
        Ok(Object::reference(&self.st, id))
    }

    /// Número de entradas de la tabla de referencias (el mayor número de objeto + 1).
    pub fn count_objects(&self) -> Result<u32> {
        let st = self.st.borrow();
        let max = st.doc.objects.keys().map(|k| k.0).max().unwrap_or(0).max(st.doc.max_id);
        Ok(max + 1)
    }

    /// Objeto indirecto `num gen R`.
    pub fn new_indirect(&self, num: i32, _gen_num: i32) -> Result<Object> {
        let n = u32::try_from(num).map_err(|_| pdf_err("número de objeto negativo"))?;
        Ok(Object::reference(&self.st, (n, 0)))
    }

    /// Objeto número `xref` ya resuelto, o `None` si no existe.
    pub fn xref_object(&self, xref: i32) -> Result<Option<Object>> {
        let len = self.count_objects()?;
        let n = u32::try_from(xref)
            .ok()
            .filter(|n| *n < len)
            .ok_or_else(|| Error::InvalidInput(format!("xref {xref} exceeds xref length {len}")))?;
        let exists = self.st.borrow().get((n, 0)).is_some();
        Ok(exists.then(|| Object::resolved(&self.st, (n, 0))))
    }

    /// Flujo (descomprimido) del objeto número `xref`.
    pub fn xref_stream(&self, xref: i32) -> Result<Vec<u8>> {
        self.xref_object(xref)?
            .ok_or_else(|| pdf_err(format!("el objeto {xref} no existe")))?
            .read_stream()
    }

    pub fn xref_raw_stream(&self, xref: i32) -> Result<Vec<u8>> {
        self.xref_object(xref)?
            .ok_or_else(|| pdf_err(format!("el objeto {xref} no existe")))?
            .read_raw_stream()
    }

    // --- Crear objetos ---

    pub fn new_null(&self) -> Object {
        Object::free(Some(self.st.clone()), L::Null)
    }
    pub fn new_bool(&self, b: bool) -> Object {
        Object::free(Some(self.st.clone()), L::Boolean(b))
    }
    pub fn new_int(&self, i: i32) -> Result<Object> {
        Ok(Object::free(Some(self.st.clone()), L::Integer(i64::from(i))))
    }
    pub fn new_real(&self, f: f32) -> Result<Object> {
        Ok(Object::free(Some(self.st.clone()), L::Real(f)))
    }
    pub fn new_string(&self, s: &str) -> Result<Object> {
        Ok(Object::free(
            Some(self.st.clone()),
            L::String(s.as_bytes().to_vec(), StringFormat::Literal),
        ))
    }
    pub fn new_name(&self, name: &str) -> Result<Object> {
        Ok(Object::free(Some(self.st.clone()), L::Name(name.as_bytes().to_vec())))
    }

    /// Cadena de texto PDF: ASCII tal cual y, si no, UTF-16BE con BOM (como exige el formato para los textos de
    /// metadatos, marcadores y anotaciones).
    pub fn new_text_string(&self, s: &str) -> Result<Object> {
        Ok(Object::free(Some(self.st.clone()), text_string(s)))
    }

    pub fn new_array(&self) -> Result<Object> {
        Ok(Object::free(Some(self.st.clone()), L::Array(Vec::new())))
    }

    pub fn new_array_with_capacity(&self, capacity: i32) -> Result<Object> {
        Ok(Object::free(
            Some(self.st.clone()),
            L::Array(Vec::with_capacity(capacity.max(0) as usize)),
        ))
    }

    pub fn new_dict(&self) -> Result<Object> {
        Ok(Object::free(Some(self.st.clone()), L::Dictionary(Dictionary::new())))
    }

    pub fn new_dict_with_capacity(&self, _capacity: i32) -> Result<Object> {
        self.new_dict()
    }

    /// Objeto a partir de su sintaxis PDF (`"<< /Type /Page >>"`, `"[0 0 612 792]"`, `"<4142>"`…).
    pub fn new_object_from_str(&self, src: &str) -> Result<Object> {
        Object::parse_in(&Some(self.st.clone()), src)
    }

    /// Construye un objeto (directo) a partir de datos planos. Las [`Value::Ref`] se crean como referencias.
    pub fn object_from_value(&self, value: &Value) -> Result<Object> {
        Ok(Object::free(Some(self.st.clone()), value_to_lopdf(value)))
    }

    /// Añade un objeto al documento y devuelve su referencia indirecta.
    pub fn add_object(&mut self, obj: &Object) -> Result<Object> {
        self.guard()?;
        if !obj.same_doc(&Object::trailer(&self.st)) && obj.obj_id().is_some() {
            return Err(Error::InvalidInput(
                "el objeto es de otro documento; cópialo antes con graft_object".into(),
            ));
        }
        let v = obj.clone_value()?;
        let id = self.st.borrow_mut().add(v);
        Ok(Object::reference(&self.st, id))
    }

    /// Añade un flujo. `dict` son las claves del diccionario del flujo (sin `Length`); `data` va sin comprimir
    /// salvo que `compressed` sea `true` (y entonces `dict` debe llevar su `Filter`). Devuelve la referencia.
    pub fn add_stream(&mut self, data: &[u8], dict: Option<&Object>, _compressed: bool) -> Result<Object> {
        self.guard()?;
        let d = match dict {
            Some(o) => o.read(|v| match v {
                L::Dictionary(d) => Ok(d.clone()),
                L::Stream(s) => Ok(s.dict.clone()),
                L::Null => Ok(Dictionary::new()),
                _ => Err(pdf_err("el diccionario del flujo no es un diccionario")),
            })?,
            None => Dictionary::new(),
        };
        let id = self.st.borrow_mut().add(L::Stream(Stream::new(d, data.to_vec())));
        Ok(Object::reference(&self.st, id))
    }

    /// Reserva un número de objeto nuevo (vacío) y devuelve su referencia.
    pub fn create_object(&mut self) -> Result<Object> {
        self.guard()?;
        let id = self.st.borrow_mut().add(L::Null);
        Ok(Object::reference(&self.st, id))
    }

    pub fn delete_object(&mut self, num: i32) -> Result<()> {
        let n = u32::try_from(num).map_err(|_| pdf_err("número de objeto negativo"))?;
        let mut st = self.st.borrow_mut();
        if st.doc.objects.remove(&(n, 0)).is_some() {
            st.dirty.remove(&(n, 0));
            st.deleted.insert((n, 0));
            st.unsaved = true;
            st.invalidate_pages();
        }
        Ok(())
    }

    /// Copia `obj` (de **otro** documento) a este, con todo lo que referencia.
    pub fn graft_object(&self, obj: &Object) -> Result<Object> {
        self.new_graft_map()?.graft_object(obj)
    }

    /// Mapa para copiar varios objetos de un mismo documento origen sin duplicar lo compartido.
    pub fn new_graft_map(&self) -> Result<GraftMap> {
        Ok(GraftMap::new(self.st.clone()))
    }

    // --- Cifrado, permisos y estado ---

    pub fn permissions(&self) -> Permission {
        match self.st.borrow().base.as_ref().and_then(|b| b.encryption_state.as_ref()) {
            Some(state) => Permission((state.permissions().bits() & 0xffff_ffff) as u32 & Permission::all().bits()),
            None => Permission::all(),
        }
    }

    /// Hay cambios sin guardar desde que se abrió o se guardó por última vez.
    pub fn has_unsaved_changes(&self) -> bool {
        self.st.borrow().unsaved
    }

    pub fn can_be_saved_incrementally(&self) -> bool {
        let st = self.st.borrow();
        !st.locked && !st.no_incremental && st.base.as_ref().is_some_and(|b| !b.repaired && b.xref_start != 0)
    }

    /// `true` si tiene un `/AcroForm`.
    pub fn has_acro_form(&self) -> Result<bool> {
        let st = self.st.borrow();
        Ok(catalog_dict(&st).is_some_and(|c| c.has(b"AcroForm")))
    }

    pub fn has_xfa_form(&self) -> Result<bool> {
        let st = self.st.borrow();
        let form = catalog_dict(&st)
            .and_then(|c| c.get(b"AcroForm").ok())
            .and_then(|f| st.dict_of(f));
        Ok(form.is_some_and(|f| f.has(b"XFA")))
    }

    // --- Metadatos y marcadores ---

    /// Metadato estándar; cadena vacía si no existe.
    pub fn metadata(&self, name: MetadataName) -> Result<String> {
        let st = self.st.borrow();
        let key: &[u8] = match name {
            MetadataName::Format => {
                return Ok(format!("PDF {}", st.doc.version));
            }
            MetadataName::Encryption => {
                return Ok(match st.base.as_ref().and_then(|b| b.encryption_state.as_ref()) {
                    Some(e) => format!("Standard V{} R{}", e.version(), e.revision()),
                    None if st.locked => "Standard".into(),
                    None => "None".into(),
                });
            }
            MetadataName::Author => b"Author",
            MetadataName::Title => b"Title",
            MetadataName::Producer => b"Producer",
            MetadataName::Creator => b"Creator",
            MetadataName::CreationDate => b"CreationDate",
            MetadataName::ModDate => b"ModDate",
            MetadataName::Subject => b"Subject",
            MetadataName::Keywords => b"Keywords",
        };
        let info = st.doc.trailer.get(b"Info").ok().and_then(|i| st.dict_of(i));
        Ok(info
            .and_then(|d| d.get(key).ok())
            .and_then(|v| st.deref(v))
            .and_then(decode_text)
            .unwrap_or_default())
    }

    /// Marcadores del documento (tolera ciclos, marcadores sin `/Title` y destinos rotos).
    pub fn outlines(&self) -> Result<Vec<Outline>> {
        self.guard()?;
        outline::read(self)
    }

    /// Resuelve un enlace interno (`#page=3`, `#nameddest=Capitulo1`…) a página y vista. `None` si no existe.
    pub fn resolve_link(&self, uri: &str) -> Result<Option<LinkDestination>> {
        self.guard()?;
        outline::resolve_link(self, uri)
    }

    /// Sustituye el árbol de marcadores.
    pub fn set_outlines(&mut self, toc: &[Outline]) -> Result<()> {
        self.guard()?;
        outline::write(self, toc)
    }

    pub fn delete_outlines(&mut self) -> Result<()> {
        self.guard()?;
        let mut st = self.st.borrow_mut();
        let cat = catalog_id(&st)?;
        if let Some(d) = dict_of_mut(&mut st, cat) {
            d.remove(b"Outlines");
            st.touch(cat);
        }
        Ok(())
    }

    // --- Archivos incrustados ---

    pub fn add_embedded_file(
        &mut self,
        name: &str,
        contents: &[u8],
        options: EmbeddedFileOptions<'_>,
    ) -> Result<EmbeddedFileInfo> {
        self.guard()?;
        let mut st = self.st.borrow_mut();
        let mut params = Dictionary::new();
        params.set("Size", contents.len() as i64);
        if let Some(c) = options.created {
            params.set("CreationDate", date_string(c));
        }
        if let Some(m) = options.modified {
            params.set("ModDate", date_string(m));
        }
        if options.add_checksum {
            params.set("CheckSum", L::String(md5(contents).to_vec(), StringFormat::Hexadecimal));
        }
        let mut sd = Dictionary::new();
        sd.set("Type", L::Name(b"EmbeddedFile".to_vec()));
        sd.set(
            "Subtype",
            L::Name(
                options
                    .mime_type
                    .unwrap_or("application/octet-stream")
                    .as_bytes()
                    .to_vec(),
            ),
        );
        sd.set("Params", L::Dictionary(params));
        let stream_id = st.add(L::Stream(Stream::new(sd, contents.to_vec())));
        let mut ef = Dictionary::new();
        ef.set("F", L::Reference(stream_id));
        ef.set("UF", L::Reference(stream_id));
        let mut spec = Dictionary::new();
        spec.set("Type", L::Name(b"Filespec".to_vec()));
        spec.set(
            "F",
            L::String(options.filename.as_bytes().to_vec(), StringFormat::Literal),
        );
        spec.set("UF", text_string(options.filename));
        spec.set("EF", L::Dictionary(ef));
        let spec_id = st.add(L::Dictionary(spec));

        let mut entries = embedded_entries(&st);
        entries.retain(|(n, _)| n.as_slice() != name.as_bytes());
        entries.push((name.as_bytes().to_vec(), L::Reference(spec_id)));
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        write_embedded_entries(&mut st, entries)?;
        let info = embedded_info(&st, name.as_bytes(), spec_id);
        info.ok_or_else(|| pdf_err("no se pudo leer el adjunto recién creado"))
    }

    pub fn embedded_files(&self) -> Result<Vec<EmbeddedFileInfo>> {
        self.guard()?;
        let st = self.st.borrow();
        Ok(embedded_entries(&st)
            .into_iter()
            .filter_map(|(name, v)| match v {
                L::Reference(id) => embedded_info(&st, &name, st.resolve_id(id)),
                _ => None,
            })
            .collect())
    }

    pub fn embedded_file(&self, name: &str) -> Result<Option<EmbeddedFileInfo>> {
        Ok(self.embedded_files()?.into_iter().find(|e| e.name == name))
    }

    pub fn load_embedded_file(&self, name: &str) -> Result<Option<Vec<u8>>> {
        let Some(info) = self.embedded_file(name)? else {
            return Ok(None);
        };
        let st = self.st.borrow();
        let stream = embedded_stream(&st, st.resolve_id((info.xref as u32, 0)));
        match stream {
            Some(L::Stream(s)) => Ok(Some(decode_stream(s)?)),
            _ => Ok(None),
        }
    }

    pub fn verify_embedded_file_checksum(&self, name: &str) -> Result<Option<bool>> {
        let Some(info) = self.embedded_file(name)? else {
            return Ok(None);
        };
        let Some(data) = self.load_embedded_file(name)? else {
            return Ok(None);
        };
        let st = self.st.borrow();
        let sum = embedded_stream(&st, (info.xref as u32, 0))
            .and_then(|s| match s {
                L::Stream(s) => s
                    .dict
                    .get(b"Params")
                    .ok()
                    .and_then(|p| st.dict_of(p))
                    .and_then(|p| p.get(b"CheckSum").ok()),
                _ => None,
            })
            .and_then(|c| c.as_str().ok());
        Ok(sum.map(|s| s == md5(&data)))
    }

    pub fn delete_embedded_file(&mut self, name: &str) -> Result<bool> {
        self.guard()?;
        let mut st = self.st.borrow_mut();
        let mut entries = embedded_entries(&st);
        let before = entries.len();
        entries.retain(|(n, _)| n.as_slice() != name.as_bytes());
        if entries.len() == before {
            return Ok(false);
        }
        write_embedded_entries(&mut st, entries)?;
        Ok(true)
    }

    // --- Capas ---

    pub fn optional_content_groups(&self) -> Result<Vec<OptionalContentGroup>> {
        self.guard()?;
        let st = self.st.borrow();
        let Some((ocgs, off, on, base_off)) = oc_state(&st) else {
            return Ok(Vec::new());
        };
        Ok(ocgs
            .into_iter()
            .map(|id| {
                let name = st
                    .doc
                    .objects
                    .get(&id)
                    .and_then(|o| st.dict_of(o))
                    .and_then(|d| d.get(b"Name").ok())
                    .and_then(|n| st.deref(n))
                    .and_then(decode_text);
                let enabled = if base_off { on.contains(&id) } else { !off.contains(&id) };
                OptionalContentGroup {
                    xref: id.0 as i32,
                    name,
                    enabled,
                }
            })
            .collect())
    }

    pub fn add_optional_content_group(&mut self, name: &str) -> Result<i32> {
        self.guard()?;
        let mut st = self.st.borrow_mut();
        let mut g = Dictionary::new();
        g.set("Type", L::Name(b"OCG".to_vec()));
        g.set("Name", text_string(name));
        let id = st.add(L::Dictionary(g));
        let cat = catalog_id(&st)?;
        let mut props = oc_properties(&st).unwrap_or_default();
        push_ref(&mut props, "OCGs", id);
        let mut d = props
            .get(b"D")
            .ok()
            .and_then(|d| st.dict_of(d))
            .cloned()
            .unwrap_or_default();
        push_ref(&mut d, "Order", id);
        props.set("D", L::Dictionary(d));
        set_dict_entry(&mut st, cat, "OCProperties", L::Dictionary(props))?;
        Ok(id.0 as i32)
    }

    /// Cambia la visibilidad por defecto de la capa `xref` (se guarda en el archivo).
    pub fn set_optional_content_enabled(&mut self, xref: i32, enabled: bool) -> Result<()> {
        self.guard()?;
        let id = (
            u32::try_from(xref).map_err(|_| pdf_err("número de objeto negativo"))?,
            0,
        );
        let mut st = self.st.borrow_mut();
        let cat = catalog_id(&st)?;
        let mut props = oc_properties(&st).ok_or_else(|| pdf_err("el documento no tiene capas"))?;
        let mut d = props
            .get(b"D")
            .ok()
            .and_then(|d| st.dict_of(d))
            .cloned()
            .unwrap_or_default();
        let base_off = d.get(b"BaseState").and_then(L::as_name).is_ok_and(|n| n == b"OFF");
        // Con estado base ON la capa «apagada» se lista en OFF; con base OFF la «encendida» se lista en ON.
        let (listed, other) = if base_off { ("ON", "OFF") } else { ("OFF", "ON") };
        remove_ref(&mut d, other, id);
        if enabled == base_off {
            if !contains_ref(&d, listed, id) {
                push_ref(&mut d, listed, id);
            }
        } else {
            remove_ref(&mut d, listed, id);
        }
        props.set("D", L::Dictionary(d));
        set_dict_entry(&mut st, cat, "OCProperties", L::Dictionary(props))
    }

    pub fn optional_content_enabled(&self, xref: i32) -> Result<bool> {
        self.guard()?;
        let id = (
            u32::try_from(xref).map_err(|_| pdf_err("número de objeto negativo"))?,
            0,
        );
        let st = self.st.borrow();
        let (_, off, on, base_off) = oc_state(&st).ok_or_else(|| pdf_err("el documento no tiene capas"))?;
        Ok(if base_off { on.contains(&id) } else { !off.contains(&id) })
    }
}

fn read_file(path: &Path) -> Result<Vec<u8>> {
    if !path.exists() {
        return Err(Error::io_at(path, std::io::Error::from(std::io::ErrorKind::NotFound)));
    }
    std::fs::read(path).map_err(|e| Error::io_at(path, e))
}

/// lopdf solo entiende un `/Encrypt` **indirecto**; MuPDF (y otros) escriben el diccionario directo en el
/// `trailer` (o, con flujo de referencias, en el diccionario de ese flujo). En ese caso se añade al final una
/// actualización incremental con tabla clásica que lo convierte en objeto indirecto; el resto del archivo no cambia.
/// Si algo no cuadra devuelve los bytes tal cual.
fn indirect_encrypt(bytes: Vec<u8>) -> Vec<u8> {
    if !bytes.windows(8).any(|w| w == b"/Encrypt") {
        return bytes;
    }
    let Ok(doc) = lopdf::Document::load_mem(&bytes) else {
        return bytes;
    };
    let Ok(L::Dictionary(enc)) = doc.trailer.get(b"Encrypt").cloned() else {
        return bytes;
    };
    let size = doc.trailer.get(b"Size").ok().and_then(|o| o.as_i64().ok()).unwrap_or(0);
    let n = u32::try_from(size).unwrap_or(0).max(doc.max_id + 1);
    let mut trailer = doc.trailer.clone();
    trailer.set("Encrypt", L::Reference((n, 0)));
    trailer.set("Size", i64::from(n) + 1);
    trailer.set("Prev", doc.xref_start as i64);
    // Con un flujo de referencias el diccionario del `trailer` trae las claves propias del flujo: no van en un
    // `trailer` clásico. La sección nueva apunta con /Prev a la anterior, sea tabla o flujo (§7.5.8.4).
    for key in [
        &b"XRefStm"[..],
        b"Type",
        b"W",
        b"Index",
        b"Filter",
        b"DecodeParms",
        b"Length",
    ] {
        trailer.remove(key);
    }
    let mut out = bytes;
    if !out.ends_with(b"\n") {
        out.push(b'\n');
    }
    let obj_off = out.len();
    out.extend_from_slice(
        format!(
            "{n} 0 obj\n{}\nendobj\n",
            super::parse::object_to_string(&L::Dictionary(enc))
        )
        .as_bytes(),
    );
    let xref_off = out.len();
    out.extend_from_slice(
        format!(
            "xref\n{n} 1\n{obj_off:010} 00000 n \ntrailer\n{}\nstartxref\n{xref_off}\n%%EOF\n",
            super::parse::object_to_string(&L::Dictionary(trailer))
        )
        .as_bytes(),
    );
    out
}

/// Carga con lopdf. El segundo valor es «sigue cifrado» (falta la contraseña).
fn load(bytes: &[u8], password: Option<&str>) -> Result<(lopdf::Document, bool)> {
    let opts = LoadOptions {
        password: password.map(str::to_owned),
        ..Default::default()
    };
    let doc = match lopdf::Document::load_mem_with_options(bytes, opts) {
        Ok(d) => d,
        Err(lopdf::Error::InvalidPassword) => return Err(Error::Password),
        Err(e) => return Err(err(e)),
    };
    let locked = doc.trailer.get(b"Encrypt").is_ok();
    if doc.trailer.get(b"Root").is_err() && !locked {
        return Err(pdf_err("el PDF no tiene catálogo (/Root)"));
    }
    Ok((doc, locked))
}

fn new_state(bytes: Vec<u8>, doc: lopdf::Document, locked: bool) -> State {
    let base = Base {
        trailer: doc.trailer.clone(),
        xref_start: doc.xref_start,
        xref_type: doc.reference_table.cross_reference_type,
        max_id: doc.max_id,
        encryption_state: doc.encryption_state.clone(),
        repaired: doc.xref_recovery.is_some(),
        bytes,
    };
    State {
        doc,
        base: Some(base),
        dirty: BTreeSet::new(),
        deleted: BTreeSet::new(),
        trailer_dirty: false,
        unsaved: false,
        no_incremental: false,
        locked,
        pages_cache: None,
        annot_dirty: BTreeSet::new(),
    }
}

pub(crate) fn value_to_lopdf(v: &Value) -> L {
    match v {
        Value::Null => L::Null,
        Value::Bool(b) => L::Boolean(*b),
        Value::Int(i) => L::Integer(i64::from(*i)),
        Value::Real(f) => L::Real(*f),
        Value::Name(n) => L::Name(n.as_bytes().to_vec()),
        Value::String(b) => L::String(b.clone(), StringFormat::Hexadecimal),
        Value::Array(items) => L::Array(items.iter().map(value_to_lopdf).collect()),
        Value::Dict(entries) => {
            let mut d = Dictionary::new();
            for (k, v) in entries {
                d.set(k.as_bytes().to_vec(), value_to_lopdf(v));
            }
            L::Dictionary(d)
        }
        Value::Ref(n) => L::Reference((u32::try_from(*n).unwrap_or(0), 0)),
    }
}

fn reject_duplicates(idx: &[usize]) -> Result<()> {
    let mut seen = std::collections::HashSet::new();
    if idx.iter().all(|i| seen.insert(*i)) {
        Ok(())
    } else {
        Err(Error::InvalidInput(
            "la selección de páginas no admite repetidas".into(),
        ))
    }
}

fn position_index(p: InsertPosition, count: usize) -> Result<usize> {
    let check = |i: usize| {
        if i >= count {
            Err(Error::InvalidInput(format!(
                "la página {} no existe (hay {count})",
                i + 1
            )))
        } else {
            Ok(())
        }
    };
    Ok(match p {
        InsertPosition::Append => count,
        InsertPosition::Before(i) => {
            check(i)?;
            i
        }
        InsertPosition::After(i) => {
            check(i)?;
            i + 1
        }
    })
}

// --- Utilidades de catálogo y diccionarios ---

pub(crate) fn catalog_id(st: &State) -> Result<ObjectId> {
    tree::catalog_id(st).ok_or_else(|| pdf_err("el documento no tiene catálogo"))
}

pub(crate) fn catalog_dict(st: &State) -> Option<&Dictionary> {
    st.dict_of(st.get(tree::catalog_id(st)?)?)
}

pub(crate) fn dict_of_mut(st: &mut State, id: ObjectId) -> Option<&mut Dictionary> {
    match st.doc.objects.get_mut(&id)? {
        L::Dictionary(d) => Some(d),
        L::Stream(s) => Some(&mut s.dict),
        _ => None,
    }
}

pub(crate) fn set_dict_entry(st: &mut State, id: ObjectId, key: &str, value: L) -> Result<()> {
    let d = dict_of_mut(st, id).ok_or_else(|| pdf_err("el objeto no es un diccionario"))?;
    d.set(key, value);
    st.touch(id);
    if tree::is_tree_node(st.doc.objects.get(&id)) {
        st.invalidate_pages();
    }
    Ok(())
}

/// Cadena de texto PDF: ASCII tal cual, UTF-16BE con BOM en otro caso.
pub(crate) fn text_string(s: &str) -> L {
    if s.is_ascii() {
        L::String(s.as_bytes().to_vec(), StringFormat::Hexadecimal)
    } else {
        let mut b = vec![0xFE, 0xFF];
        for u in s.encode_utf16() {
            b.extend_from_slice(&u.to_be_bytes());
        }
        L::String(b, StringFormat::Hexadecimal)
    }
}

/// Cadena de texto PDF a `String` (UTF-16BE/LE con BOM, UTF-8 con BOM o PDFDocEncoding aproximado).
pub(crate) fn decode_text(o: &L) -> Option<String> {
    let b = match o {
        L::String(b, _) => b,
        L::Name(n) => return Some(String::from_utf8_lossy(n).into_owned()),
        _ => return None,
    };
    if b.starts_with(&[0xFE, 0xFF]) {
        let units: Vec<u16> = b[2..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16::from_be_bytes(*c))
            .collect();
        return Some(String::from_utf16_lossy(&units));
    }
    if b.starts_with(&[0xFF, 0xFE]) {
        let units: Vec<u16> = b[2..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16::from_le_bytes(*c))
            .collect();
        return Some(String::from_utf16_lossy(&units));
    }
    if b.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return Some(String::from_utf8_lossy(&b[3..]).into_owned());
    }
    Some(b.iter().map(|&c| c as char).collect())
}

fn push_ref(d: &mut Dictionary, key: &str, id: ObjectId) {
    match d.get_mut(key.as_bytes()) {
        Ok(L::Array(a)) => a.push(L::Reference(id)),
        _ => d.set(key, L::Array(vec![L::Reference(id)])),
    }
}

fn contains_ref(d: &Dictionary, key: &str, id: ObjectId) -> bool {
    matches!(d.get(key.as_bytes()), Ok(L::Array(a)) if a.iter().any(|o| *o == L::Reference(id)))
}

fn remove_ref(d: &mut Dictionary, key: &str, id: ObjectId) {
    if let Ok(L::Array(a)) = d.get_mut(key.as_bytes()) {
        a.retain(|o| *o != L::Reference(id));
    }
}

// --- Capas ---

fn oc_properties(st: &State) -> Option<Dictionary> {
    catalog_dict(st)?
        .get(b"OCProperties")
        .ok()
        .and_then(|p| st.dict_of(p))
        .cloned()
}

fn ref_list(st: &State, d: &Dictionary, key: &str) -> Vec<ObjectId> {
    d.get(key.as_bytes())
        .ok()
        .and_then(|a| st.deref(a))
        .and_then(|a| a.as_array().ok())
        .map(|a| {
            a.iter()
                .filter_map(|o| o.as_reference().ok())
                .map(|id| st.resolve_id(id))
                .collect()
        })
        .unwrap_or_default()
}

/// `(grupos, apagados, encendidos, estado base apagado)`
type OcState = (Vec<ObjectId>, Vec<ObjectId>, Vec<ObjectId>, bool);

fn oc_state(st: &State) -> Option<OcState> {
    let props = oc_properties(st)?;
    let ocgs = ref_list(st, &props, "OCGs");
    let d = props
        .get(b"D")
        .ok()
        .and_then(|d| st.dict_of(d))
        .cloned()
        .unwrap_or_default();
    let base_off = d.get(b"BaseState").and_then(L::as_name).is_ok_and(|n| n == b"OFF");
    Some((ocgs, ref_list(st, &d, "OFF"), ref_list(st, &d, "ON"), base_off))
}

// --- Adjuntos ---

/// Recorre un árbol de nombres (`/Names` plano o `/Kids`) y devuelve los pares `(nombre, valor)`.
pub(crate) fn name_tree(st: &State, node: &L) -> Vec<(Vec<u8>, L)> {
    fn go(st: &State, node: &L, out: &mut Vec<(Vec<u8>, L)>, depth: usize) {
        if depth > 32 {
            return;
        }
        let Some(d) = st.dict_of(node) else { return };
        if let Some(names) = d
            .get(b"Names")
            .ok()
            .and_then(|n| st.deref(n))
            .and_then(|n| n.as_array().ok())
        {
            for pair in names.chunks(2) {
                if let [k, v] = pair
                    && let Some(key) = st.deref(k).and_then(|k| k.as_str().ok())
                {
                    out.push((key.to_vec(), v.clone()));
                }
            }
        }
        if let Some(kids) = d
            .get(b"Kids")
            .ok()
            .and_then(|n| st.deref(n))
            .and_then(|n| n.as_array().ok())
        {
            for k in kids {
                go(st, k, out, depth + 1);
            }
        }
    }
    let mut out = Vec::new();
    go(st, node, &mut out, 0);
    out
}

fn embedded_entries(st: &State) -> Vec<(Vec<u8>, L)> {
    let root = catalog_dict(st)
        .and_then(|c| c.get(b"Names").ok())
        .and_then(|n| st.dict_of(n))
        .and_then(|n| n.get(b"EmbeddedFiles").ok());
    match root {
        Some(r) => name_tree(st, r),
        None => Vec::new(),
    }
}

fn write_embedded_entries(st: &mut State, entries: Vec<(Vec<u8>, L)>) -> Result<()> {
    let cat = catalog_id(st)?;
    let mut names = catalog_dict(st)
        .and_then(|c| c.get(b"Names").ok())
        .and_then(|n| st.dict_of(n))
        .cloned()
        .unwrap_or_default();
    if entries.is_empty() {
        names.remove(b"EmbeddedFiles");
    } else {
        let mut nums = Vec::new();
        for (k, v) in entries {
            nums.push(L::String(k, StringFormat::Literal));
            nums.push(v);
        }
        let mut tree = Dictionary::new();
        tree.set("Names", L::Array(nums));
        names.set("EmbeddedFiles", L::Dictionary(tree));
    }
    set_dict_entry(st, cat, "Names", L::Dictionary(names))
}

fn embedded_stream(st: &State, spec: ObjectId) -> Option<&L> {
    let d = st.dict_of(st.doc.objects.get(&spec)?)?;
    let ef = d.get(b"EF").ok().and_then(|e| st.dict_of(e))?;
    let f = ef.get(b"UF").or_else(|_| ef.get(b"F")).ok()?;
    st.deref(f)
}

fn embedded_info(st: &State, name: &[u8], spec: ObjectId) -> Option<EmbeddedFileInfo> {
    let d = st.dict_of(st.doc.objects.get(&spec)?)?;
    let stream = embedded_stream(st, spec);
    let sd = stream.and_then(|s| st.dict_of(s));
    let params = sd.and_then(|s| s.get(b"Params").ok()).and_then(|p| st.dict_of(p));
    let size = params
        .and_then(|p| p.get(b"Size").ok())
        .and_then(|s| s.as_i64().ok())
        .map(|n| n.max(0) as usize)
        .or_else(|| match stream {
            Some(L::Stream(s)) => decode_stream(s).ok().map(|v| v.len()),
            _ => None,
        })
        .unwrap_or(0);
    let date = |k: &[u8]| {
        params
            .and_then(|p| p.get(k).ok())
            .and_then(decode_text)
            .and_then(|s| parse_date(&s))
    };
    Some(EmbeddedFileInfo {
        name: String::from_utf8_lossy(name).into_owned(),
        xref: spec.0 as i32,
        filename: d
            .get(b"UF")
            .or_else(|_| d.get(b"F"))
            .ok()
            .and_then(|f| st.deref(f))
            .and_then(decode_text),
        mime_type: sd
            .and_then(|s| s.get(b"Subtype").ok())
            .and_then(|s| s.as_name().ok())
            .map(|n| String::from_utf8_lossy(n).into_owned()),
        size,
        created: date(b"CreationDate"),
        modified: date(b"ModDate"),
    })
}

// --- Etiquetas ---

fn label_rules(st: &State) -> Vec<PageLabelRule> {
    let Some(labels) = catalog_dict(st).and_then(|c| c.get(b"PageLabels").ok()) else {
        return Vec::new();
    };
    fn go(st: &State, node: &L, out: &mut Vec<(i64, L)>, depth: usize) {
        if depth > 32 {
            return;
        }
        let Some(d) = st.dict_of(node) else { return };
        if let Some(nums) = d
            .get(b"Nums")
            .ok()
            .and_then(|n| st.deref(n))
            .and_then(|n| n.as_array().ok())
        {
            for pair in nums.chunks(2) {
                if let [k, v] = pair
                    && let Some(i) = st.deref(k).and_then(|k| k.as_i64().ok())
                {
                    out.push((i, v.clone()));
                }
            }
        }
        if let Some(kids) = d
            .get(b"Kids")
            .ok()
            .and_then(|n| st.deref(n))
            .and_then(|n| n.as_array().ok())
        {
            for k in kids {
                go(st, k, out, depth + 1);
            }
        }
    }
    let mut raw = Vec::new();
    go(st, labels, &mut raw, 0);
    raw.sort_by_key(|(i, _)| *i);
    raw.into_iter()
        .filter(|(i, _)| *i >= 0)
        .map(|(i, v)| {
            let d = st.dict_of(&v);
            let style = match d.and_then(|d| d.get(b"S").ok()).and_then(|s| s.as_name().ok()) {
                Some(b"D") => PageLabelStyle::Decimal,
                Some(b"R") => PageLabelStyle::UpperRoman,
                Some(b"r") => PageLabelStyle::LowerRoman,
                Some(b"A") => PageLabelStyle::UpperAlpha,
                Some(b"a") => PageLabelStyle::LowerAlpha,
                _ => PageLabelStyle::None,
            };
            PageLabelRule {
                index: i as usize,
                style,
                prefix: d
                    .and_then(|d| d.get(b"P").ok())
                    .and_then(|p| st.deref(p))
                    .and_then(decode_text)
                    .unwrap_or_default(),
                start: d
                    .and_then(|d| d.get(b"St").ok())
                    .and_then(|s| s.as_i64().ok())
                    .unwrap_or(1)
                    .clamp(1, i64::from(i32::MAX)) as i32,
            }
        })
        .collect()
}

fn roman(mut n: i64, upper: bool) -> String {
    if n <= 0 {
        return n.to_string();
    }
    const T: [(i64, &str); 13] = [
        (1000, "m"),
        (900, "cm"),
        (500, "d"),
        (400, "cd"),
        (100, "c"),
        (90, "xc"),
        (50, "l"),
        (40, "xl"),
        (10, "x"),
        (9, "ix"),
        (5, "v"),
        (4, "iv"),
        (1, "i"),
    ];
    let mut s = String::new();
    for (v, r) in T {
        while n >= v {
            s.push_str(r);
            n -= v;
        }
    }
    if upper { s.to_uppercase() } else { s }
}

/// A, B, … Z, AA, BB, … (la regla de la especificación: la letra se repite).
fn alpha(n: i64, upper: bool) -> String {
    if n <= 0 {
        return n.to_string();
    }
    let idx = (n - 1) % 26;
    let reps = (n - 1) / 26 + 1;
    let c = (b'a' + idx as u8) as char;
    let s: String = std::iter::repeat_n(c, reps as usize).collect();
    if upper { s.to_uppercase() } else { s }
}

// --- Fechas y resumen MD5 ---

/// Segundos Unix a `D:AAAAMMDDHHmmSSZ`.
fn date_string(secs: i64) -> L {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    L::String(
        format!(
            "D:{y:04}{m:02}{d:02}{:02}{:02}{:02}Z",
            rem / 3600,
            rem % 3600 / 60,
            rem % 60
        )
        .into_bytes(),
        StringFormat::Literal,
    )
}

fn parse_date(s: &str) -> Option<i64> {
    let s = s.strip_prefix("D:").unwrap_or(s);
    let digits: String = s.chars().take_while(char::is_ascii_digit).collect();
    if digits.len() < 4 {
        return None;
    }
    let num =
        |a: usize, b: usize, default: i64| digits.get(a..b).and_then(|x| x.parse::<i64>().ok()).unwrap_or(default);
    let (y, mo, d) = (num(0, 4, 1970), num(4, 6, 1), num(6, 8, 1));
    let (h, mi, se) = (num(8, 10, 0), num(10, 12, 0), num(12, 14, 0));
    let mut secs = days_from_civil(y, mo, d) * 86_400 + h * 3600 + mi * 60 + se;
    let rest = &s[digits.len()..];
    if let Some(sign) = rest.chars().next().filter(|c| *c == '+' || *c == '-') {
        let tz: Vec<i64> = rest[1..]
            .split(|c: char| !c.is_ascii_digit())
            .filter(|p| !p.is_empty())
            .filter_map(|p| p.parse().ok())
            .collect();
        let off = tz.first().copied().unwrap_or(0) * 3600 + tz.get(1).copied().unwrap_or(0) * 60;
        secs -= if sign == '+' { off } else { -off };
    }
    Some(secs)
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub(crate) fn md5(data: &[u8]) -> [u8; 16] {
    use md5::{Digest, Md5};
    let mut h = Md5::new();
    h.update(data);
    h.finalize().into()
}
