//! Documentos: [`Document`] (cualquier formato que el motor sepa leer) y [`PdfDocument`] (un PDF, con su modelo
//! de objetos y guardado).
//!
//! - Ni [`Document`] ni [`PdfDocument`] son `Send`: abrir y usar cada documento en un único hilo.
//! - Los índices de página son `i32` desde 0, igual que los de MuPDF, para que la migración de los crates sea
//!   mecánica; las funciones nuevas usan `usize`/`u32` (ver cada una).

use std::io::Write;
use std::ops::{Deref, DerefMut};
use std::path::Path;

use pliego_core::{Error, Result};

use crate::backend::{err, path_str, size_to};
use crate::geom::Size;
use crate::links::LinkDestination;
use crate::object::{Object, Value};
use crate::options::{
    EmbeddedFileInfo, EmbeddedFileOptions, Encryption, InsertPdfOptions, InsertPdfResult, InsertPosition, MetadataName,
    OptionalContentGroup, Outline, PageLabelRule, PageLabelStyle, PageRange, PageSelection, WriteOptions,
};
use crate::page::{Page, PdfPage};

pub(crate) enum DocRaw {
    Plain(mupdf::Document),
    Pdf(mupdf::pdf::PdfDocument),
}

/// Documento de cualquier formato legible (PDF, XPS, EPUB, imágenes, HTML…).
pub struct Document {
    pub(crate) raw: DocRaw,
}

impl Document {
    /// Abre un archivo (el formato se deduce de su contenido/extensión). **No** pide contraseña: usa
    /// [`Document::open_with_password`] o [`Document::authenticate`].
    pub fn open(path: impl AsRef<Path>) -> Result<Document> {
        crate::fonts::init_fonts();
        let path = path.as_ref();
        if !path.exists() {
            return Err(Error::io_at(path, std::io::Error::from(std::io::ErrorKind::NotFound)));
        }
        let doc = mupdf::Document::open(path_str(path)?).map_err(err)?;
        Ok(Document {
            raw: DocRaw::Plain(doc),
        })
    }

    /// Abre desde memoria. `magic` es el tipo MIME o la extensión (`"application/pdf"`, `"epub"`…).
    pub fn from_bytes(bytes: &[u8], magic: &str) -> Result<Document> {
        crate::fonts::init_fonts();
        let doc = mupdf::Document::from_bytes(bytes, magic).map_err(err)?;
        Ok(Document {
            raw: DocRaw::Plain(doc),
        })
    }

    /// Abre un archivo y, si pide contraseña, la comprueba. [`Error::Password`] si falta o es incorrecta.
    pub fn open_with_password(path: impl AsRef<Path>, password: Option<&str>) -> Result<Document> {
        let mut doc = Self::open(path)?;
        doc.unlock(password)?;
        Ok(doc)
    }

    /// Como [`Document::open_with_password`] desde memoria.
    pub fn from_bytes_with_password(bytes: &[u8], magic: &str, password: Option<&str>) -> Result<Document> {
        let mut doc = Self::from_bytes(bytes, magic)?;
        doc.unlock(password)?;
        Ok(doc)
    }

    fn unlock(&mut self, password: Option<&str>) -> Result<()> {
        if self.needs_password()? {
            let ok = match password {
                Some(p) => self.authenticate(p)?,
                None => false,
            };
            if !ok {
                return Err(Error::Password);
            }
        }
        Ok(())
    }

    pub(crate) fn base(&self) -> &mupdf::Document {
        match &self.raw {
            DocRaw::Plain(d) => d,
            DocRaw::Pdf(p) => p,
        }
    }

    fn base_mut(&mut self) -> &mut mupdf::Document {
        match &mut self.raw {
            DocRaw::Plain(d) => d,
            DocRaw::Pdf(p) => p,
        }
    }

    /// `true` si es un PDF.
    pub fn is_pdf(&self) -> bool {
        self.base().is_pdf()
    }

    /// Convierte a [`PdfDocument`]. Error si no es un PDF (compruébalo antes con [`Document::is_pdf`]).
    pub fn into_pdf(self) -> Result<PdfDocument> {
        match self.raw {
            DocRaw::Pdf(p) => Ok(PdfDocument::from_raw(p)),
            DocRaw::Plain(d) => {
                if !d.is_pdf() {
                    return Err(Error::Unsupported("el documento no es un PDF".into()));
                }
                mupdf::pdf::PdfDocument::try_from(d)
                    .map(PdfDocument::from_raw)
                    .map_err(err)
            }
        }
    }

    pub fn needs_password(&self) -> Result<bool> {
        self.base().needs_password().map_err(err)
    }

    /// `true` si la contraseña es válida (de usuario o de propietario).
    pub fn authenticate(&mut self, password: &str) -> Result<bool> {
        self.base_mut().authenticate(password).map_err(err)
    }

    pub fn page_count(&self) -> Result<i32> {
        self.base().page_count().map_err(err)
    }

    pub fn load_page(&self, page_no: i32) -> Result<Page> {
        self.base().load_page(page_no).map(Page::from_plain).map_err(err)
    }

    /// Metadato estándar; cadena vacía si no existe.
    pub fn metadata(&self, name: MetadataName) -> Result<String> {
        self.base().metadata(metadata_to(name)).map_err(err)
    }

    /// Marcadores. En un PDF con un marcador sin `/Title` MuPDF aborta: usa `PdfDocument::outlines`, que los
    /// recorre con cuidado.
    pub fn outlines(&self) -> Result<Vec<Outline>> {
        Ok(convert_outlines(self.base().outlines().map_err(err)?))
    }

    /// Resuelve un enlace interno (`#page=3`, `#nameddest=Capitulo1`…) a página y vista. `None` si no existe.
    pub fn resolve_link(&self, uri: &str) -> Result<Option<LinkDestination>> {
        Ok(self.base().resolve_link(uri).map_err(err)?.map(|d| LinkDestination {
            page: d.loc.page_number,
            kind: crate::page::kind_from(d.kind),
        }))
    }

    /// Formatos reflowable (EPUB, HTML): recoloca el texto para una página de `width × height` puntos.
    pub fn layout(&mut self, width: f32, height: f32, em: f32) -> Result<()> {
        self.base_mut().layout(width, height, em).map_err(err)
    }

    pub fn is_reflowable(&self) -> Result<bool> {
        self.base().is_reflowable().map_err(err)
    }
}

pub(crate) fn metadata_to(n: MetadataName) -> mupdf::MetadataName {
    use mupdf::MetadataName as M;
    match n {
        MetadataName::Format => M::Format,
        MetadataName::Encryption => M::Encryption,
        MetadataName::Author => M::Author,
        MetadataName::Title => M::Title,
        MetadataName::Producer => M::Producer,
        MetadataName::Creator => M::Creator,
        MetadataName::CreationDate => M::CreationDate,
        MetadataName::ModDate => M::ModDate,
        MetadataName::Subject => M::Subject,
        MetadataName::Keywords => M::Keywords,
    }
}

pub(crate) fn convert_outlines(items: Vec<mupdf::Outline>) -> Vec<Outline> {
    items
        .into_iter()
        .map(|o| Outline {
            title: o.title,
            uri: o.uri,
            dest: o.dest.map(|d| LinkDestination {
                page: d.loc.page_number,
                kind: crate::page::kind_from(d.kind),
            }),
            down: convert_outlines(o.down),
        })
        .collect()
}

/// Documento PDF. Deriva a [`Document`] para todo lo común (páginas, texto, metadatos).
pub struct PdfDocument {
    doc: Document,
}

impl Deref for PdfDocument {
    type Target = Document;
    fn deref(&self) -> &Document {
        &self.doc
    }
}

impl DerefMut for PdfDocument {
    fn deref_mut(&mut self) -> &mut Document {
        &mut self.doc
    }
}

impl Default for PdfDocument {
    fn default() -> Self {
        Self::new()
    }
}

impl PdfDocument {
    pub(crate) fn from_raw(raw: mupdf::pdf::PdfDocument) -> Self {
        Self {
            doc: Document { raw: DocRaw::Pdf(raw) },
        }
    }

    pub(crate) fn pdf(&self) -> &mupdf::pdf::PdfDocument {
        match &self.doc.raw {
            DocRaw::Pdf(p) => p,
            DocRaw::Plain(_) => unreachable!("PdfDocument siempre contiene un PDF"),
        }
    }

    pub(crate) fn pdf_mut(&mut self) -> &mut mupdf::pdf::PdfDocument {
        match &mut self.doc.raw {
            DocRaw::Pdf(p) => p,
            DocRaw::Plain(_) => unreachable!("PdfDocument siempre contiene un PDF"),
        }
    }

    /// Documento vacío (sin páginas).
    pub fn new() -> Self {
        crate::fonts::init_fonts();
        Self::from_raw(mupdf::pdf::PdfDocument::new())
    }

    /// Abre un PDF. No pide contraseña (ver [`PdfDocument::open_with_password`]). Falla si no es un PDF.
    pub fn open(path: impl AsRef<Path>) -> Result<PdfDocument> {
        crate::fonts::init_fonts();
        let path = path.as_ref();
        if !path.exists() {
            return Err(Error::io_at(path, std::io::Error::from(std::io::ErrorKind::NotFound)));
        }
        mupdf::pdf::PdfDocument::open(path_str(path)?)
            .map(Self::from_raw)
            .map_err(err)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<PdfDocument> {
        crate::fonts::init_fonts();
        mupdf::pdf::PdfDocument::from_bytes(bytes)
            .map(Self::from_raw)
            .map_err(err)
    }

    /// Abre y desbloquea. [`Error::Password`] si hace falta contraseña y falta o es incorrecta.
    pub fn open_with_password(path: impl AsRef<Path>, password: Option<&str>) -> Result<PdfDocument> {
        let mut doc = Self::open(path)?;
        doc.doc.unlock(password)?;
        Ok(doc)
    }

    pub fn from_bytes_with_password(bytes: &[u8], password: Option<&str>) -> Result<PdfDocument> {
        let mut doc = Self::from_bytes(bytes)?;
        doc.doc.unlock(password)?;
        Ok(doc)
    }

    // --- Páginas ---

    pub fn load_pdf_page(&self, page_no: i32) -> Result<PdfPage> {
        self.pdf().load_pdf_page(page_no).map(PdfPage::from_raw).map_err(err)
    }

    /// Objeto `/Page` de la página `page_no` (desde 0).
    pub fn find_page(&self, page_no: i32) -> Result<Object> {
        self.pdf().find_page(page_no).map(Object::wrap).map_err(err)
    }

    /// Número de página (desde 0) de un objeto `/Page`.
    pub fn lookup_page_number(&self, page_obj: &Object) -> Result<i32> {
        self.pdf().lookup_page_number(&page_obj.raw).map_err(err)
    }

    /// Añade una página en blanco al final.
    pub fn new_page(&mut self, size: Size) -> Result<PdfPage> {
        self.pdf_mut()
            .new_page(size_to(size))
            .map(PdfPage::from_raw)
            .map_err(err)
    }

    /// Añade una página en blanco en la posición `page_no` (desde 0; `-1` = al final).
    pub fn new_page_at(&mut self, page_no: i32, size: Size) -> Result<PdfPage> {
        self.pdf_mut()
            .new_page_at(page_no, size_to(size))
            .map(PdfPage::from_raw)
            .map_err(err)
    }

    /// Inserta un objeto `/Page` ya construido en la posición `page_no`.
    pub fn insert_page(&mut self, page_no: i32, page: &Object) -> Result<()> {
        self.pdf_mut().insert_page(page_no, &page.raw).map_err(err)
    }

    pub fn delete_page(&mut self, page_no: i32) -> Result<()> {
        self.pdf_mut().delete_page(page_no).map_err(err)
    }

    pub fn delete_pages(&mut self, selection: impl Into<PageSelection>) -> Result<()> {
        let sel = selection.into();
        let count = self.page_count()?.max(0) as usize;
        let idx = sel.validated_indices(count)?;
        self.pdf_mut()
            .delete_pages(mupdf::pdf::PageSelection::Pages(idx))
            .map_err(err)
    }

    /// Deja solo las páginas indicadas, en ese orden.
    pub fn select_pages(&mut self, selection: impl Into<PageSelection>) -> Result<()> {
        let sel = selection.into();
        let count = self.page_count()?.max(0) as usize;
        let idx = sel.validated_indices(count)?;
        self.pdf_mut()
            .select_pages(mupdf::pdf::PageSelection::Pages(idx))
            .map_err(err)
    }

    pub fn move_page(&mut self, source_index: usize, target_index: usize) -> Result<()> {
        self.pdf_mut().move_page(source_index, target_index).map_err(err)
    }

    pub fn duplicate_page(&mut self, source_index: usize) -> Result<InsertPdfResult> {
        self.pdf_mut()
            .duplicate_page(source_index)
            .map(insert_result)
            .map_err(err)
    }

    pub fn copy_page(&mut self, source_index: usize, target: InsertPosition) -> Result<InsertPdfResult> {
        self.pdf_mut()
            .copy_page(source_index, position_to(target))
            .map(insert_result)
            .map_err(err)
    }

    /// Copia páginas de `src` (con su contenido, recursos, enlaces, anotaciones y campos).
    pub fn insert_pdf(&mut self, src: &PdfDocument, options: InsertPdfOptions) -> Result<InsertPdfResult> {
        let count = src.page_count()?.max(0) as usize;
        let idx = options.source_pages.validated_indices(count)?;
        let opts = mupdf::pdf::InsertPdfOptions {
            source_pages: mupdf::pdf::PageSelection::Pages(idx),
            target: position_to(options.target),
            rotate: options.rotate,
            ..Default::default()
        };
        self.pdf_mut()
            .insert_pdf(src.pdf(), opts)
            .map(insert_result)
            .map_err(err)
    }

    /// Etiqueta de la página `page_index` (desde 0), p. ej. «iv» o «A-3».
    pub fn page_label(&self, page_index: usize) -> Result<String> {
        self.pdf().page_label(page_index).map_err(err)
    }

    pub fn set_page_label_rule(&mut self, rule: PageLabelRule) -> Result<()> {
        let style = match rule.style {
            PageLabelStyle::None => mupdf::pdf::PageLabelStyle::None,
            PageLabelStyle::Decimal => mupdf::pdf::PageLabelStyle::Decimal,
            PageLabelStyle::UpperRoman => mupdf::pdf::PageLabelStyle::UpperRoman,
            PageLabelStyle::LowerRoman => mupdf::pdf::PageLabelStyle::LowerRoman,
            PageLabelStyle::UpperAlpha => mupdf::pdf::PageLabelStyle::UpperAlpha,
            PageLabelStyle::LowerAlpha => mupdf::pdf::PageLabelStyle::LowerAlpha,
        };
        let mut r = mupdf::pdf::PageLabelRule::new(rule.index, style);
        r.prefix = rule.prefix;
        r.start = rule.start;
        self.pdf_mut().set_page_label_rule(r).map_err(err)
    }

    // --- Estructura del archivo ---

    /// Diccionario `trailer`.
    pub fn trailer(&self) -> Result<Object> {
        self.pdf().trailer().map(Object::wrap).map_err(err)
    }

    /// Diccionario `/Root` (catálogo).
    pub fn catalog(&self) -> Result<Object> {
        self.pdf().catalog().map(Object::wrap).map_err(err)
    }

    /// Número de entradas de la tabla de referencias (el mayor número de objeto + 1).
    pub fn count_objects(&self) -> Result<u32> {
        self.pdf().count_objects().map_err(err)
    }

    /// Objeto indirecto `num gen R`.
    pub fn new_indirect(&self, num: i32, gen_num: i32) -> Result<Object> {
        self.pdf().new_indirect(num, gen_num).map(Object::wrap).map_err(err)
    }

    /// Objeto número `xref` ya resuelto, o `None` si no existe.
    pub fn xref_object(&self, xref: i32) -> Result<Option<Object>> {
        self.pdf().xref_object(xref).map(|o| o.map(Object::wrap)).map_err(err)
    }

    /// Flujo (descomprimido) del objeto número `xref`.
    pub fn xref_stream(&self, xref: i32) -> Result<Vec<u8>> {
        self.pdf().xref_stream(xref).map_err(err)
    }

    pub fn xref_raw_stream(&self, xref: i32) -> Result<Vec<u8>> {
        self.pdf().xref_raw_stream(xref).map_err(err)
    }

    // --- Crear objetos ---

    pub fn new_null(&self) -> Object {
        Object::new_null()
    }
    pub fn new_bool(&self, b: bool) -> Object {
        Object::new_bool(b)
    }
    pub fn new_int(&self, i: i32) -> Result<Object> {
        Object::new_int(i)
    }
    pub fn new_real(&self, f: f32) -> Result<Object> {
        Object::new_real(f)
    }
    pub fn new_string(&self, s: &str) -> Result<Object> {
        Object::new_string(s)
    }
    pub fn new_name(&self, name: &str) -> Result<Object> {
        Object::new_name(name)
    }

    /// Cadena de texto PDF: ASCII tal cual y, si no, UTF-16BE con BOM (como exige el formato para los textos de
    /// metadatos, marcadores y anotaciones).
    pub fn new_text_string(&self, s: &str) -> Result<Object> {
        use std::fmt::Write as _;
        let mut hex = String::from("<");
        if s.is_ascii() {
            for b in s.bytes() {
                let _ = write!(hex, "{b:02X}");
            }
        } else {
            hex.push_str("FEFF");
            for u in s.encode_utf16() {
                let _ = write!(hex, "{u:04X}");
            }
        }
        hex.push('>');
        self.new_object_from_str(&hex)
    }

    pub fn new_array(&self) -> Result<Object> {
        self.pdf().new_array().map(Object::wrap).map_err(err)
    }

    pub fn new_array_with_capacity(&self, capacity: i32) -> Result<Object> {
        self.pdf()
            .new_array_with_capacity(capacity)
            .map(Object::wrap)
            .map_err(err)
    }

    pub fn new_dict(&self) -> Result<Object> {
        self.pdf().new_dict().map(Object::wrap).map_err(err)
    }

    pub fn new_dict_with_capacity(&self, capacity: i32) -> Result<Object> {
        self.pdf()
            .new_dict_with_capacity(capacity)
            .map(Object::wrap)
            .map_err(err)
    }

    /// Objeto a partir de su sintaxis PDF (`"<< /Type /Page >>"`, `"[0 0 612 792]"`, `"<4142>"`…).
    pub fn new_object_from_str(&self, src: &str) -> Result<Object> {
        self.pdf().new_object_from_str(src).map(Object::wrap).map_err(err)
    }

    /// Construye un objeto (directo) a partir de datos planos. Las [`Value::Ref`] se crean como referencias.
    pub fn object_from_value(&self, value: &Value) -> Result<Object> {
        match value {
            Value::Null => Ok(Object::new_null()),
            Value::Bool(b) => Ok(Object::new_bool(*b)),
            Value::Int(i) => Object::new_int(*i),
            Value::Real(f) => Object::new_real(*f),
            Value::Name(n) => Object::new_name(n),
            Value::String(bytes) => {
                use std::fmt::Write as _;
                let mut hex = String::from("<");
                for b in bytes {
                    let _ = write!(hex, "{b:02X}");
                }
                hex.push('>');
                self.new_object_from_str(&hex)
            }
            Value::Array(items) => {
                let mut a = self.new_array_with_capacity(items.len() as i32)?;
                for it in items {
                    a.array_push(self.object_from_value(it)?)?;
                }
                Ok(a)
            }
            Value::Dict(entries) => {
                let mut d = self.new_dict_with_capacity(entries.len() as i32)?;
                for (k, v) in entries {
                    d.dict_put(k, self.object_from_value(v)?)?;
                }
                Ok(d)
            }
            Value::Ref(n) => self.new_indirect(*n, 0),
        }
    }

    /// Añade un objeto al documento y devuelve su referencia indirecta.
    pub fn add_object(&mut self, obj: &Object) -> Result<Object> {
        self.pdf_mut().add_object(&obj.raw).map(Object::wrap).map_err(err)
    }

    /// Añade un flujo. `dict` son las claves del diccionario del flujo (sin `Length`); `data` va sin comprimir
    /// salvo que `compressed` sea `true` (y entonces `dict` debe llevar su `Filter`). Devuelve la referencia.
    pub fn add_stream(&mut self, data: &[u8], dict: Option<&Object>, compressed: bool) -> Result<Object> {
        let buf = mupdf::Buffer::from_bytes(data).map_err(err)?;
        self.pdf_mut()
            .add_stream(&buf, dict.map(|d| &d.raw), compressed)
            .map(Object::wrap)
            .map_err(err)
    }

    /// Reserva un número de objeto nuevo (vacío) y devuelve su referencia.
    pub fn create_object(&mut self) -> Result<Object> {
        self.pdf_mut().create_object().map(Object::wrap).map_err(err)
    }

    pub fn delete_object(&mut self, num: i32) -> Result<()> {
        self.pdf_mut().delete_object(num).map_err(err)
    }

    /// Copia `obj` (de **otro** documento) a este, con todo lo que referencia.
    pub fn graft_object(&self, obj: &Object) -> Result<Object> {
        self.pdf().graft_object(&obj.raw).map(Object::wrap).map_err(err)
    }

    /// Mapa para copiar varios objetos de un mismo documento origen sin duplicar lo compartido.
    pub fn new_graft_map(&self) -> Result<GraftMap> {
        self.pdf().new_graft_map().map(|raw| GraftMap { raw }).map_err(err)
    }

    // --- Cifrado, permisos y estado ---

    pub fn permissions(&self) -> crate::options::Permission {
        crate::options::Permission(self.pdf().permissions().bits())
    }

    /// Hay cambios sin guardar desde que se abrió o se guardó por última vez.
    pub fn has_unsaved_changes(&self) -> bool {
        self.pdf().has_unsaved_changes()
    }

    pub fn can_be_saved_incrementally(&self) -> bool {
        self.pdf().can_be_saved_incrementally()
    }

    /// `true` si tiene un `/AcroForm`.
    pub fn has_acro_form(&self) -> Result<bool> {
        self.pdf().has_acro_form().map_err(err)
    }

    pub fn has_xfa_form(&self) -> Result<bool> {
        self.pdf().has_xfa_form().map_err(err)
    }

    /// Recalcula los campos con acciones de cálculo (necesita JavaScript habilitado).
    pub fn calculate_form(&mut self) -> Result<()> {
        self.pdf_mut().calculate_form().map_err(err)
    }

    pub fn enable_js(&mut self) -> Result<()> {
        self.pdf_mut().enable_js().map_err(err)
    }

    pub fn disable_js(&mut self) -> Result<()> {
        self.pdf_mut().disable_js().map_err(err)
    }

    pub fn is_js_supported(&self) -> Result<bool> {
        self.pdf().is_js_supported().map_err(err)
    }

    /// Aplana anotaciones y/o campos de formulario en el contenido de las páginas.
    pub fn bake(&mut self, bake_annots: bool, bake_widgets: bool) -> Result<()> {
        self.pdf_mut().bake(bake_annots, bake_widgets).map_err(err)
    }

    // --- Guardado ---

    /// Serializa el documento a `w` con las opciones dadas.
    pub fn write_to_with_options<W: Write>(&self, w: &mut W, options: &WriteOptions) -> Result<u64> {
        self.pdf()
            .write_to_with_options(w, write_options_to(options))
            .map_err(err)
    }

    /// Serializa con las opciones por defecto.
    pub fn write_to<W: Write>(&self, w: &mut W) -> Result<u64> {
        self.pdf().write_to(w).map_err(err)
    }

    /// Serializa a memoria.
    pub fn to_bytes(&self, options: &WriteOptions) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        self.write_to_with_options(&mut out, options)?;
        Ok(out)
    }

    /// Escribe directamente en `path` (no atómico: para guardado seguro usa `pliego-document`). Con
    /// `incremental`, `path` debe ser una copia del archivo de origen.
    pub fn save_with_options(&self, path: impl AsRef<Path>, options: &WriteOptions) -> Result<()> {
        self.pdf()
            .save_with_options(path_str(path.as_ref())?, write_options_to(options))
            .map_err(err)
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        self.pdf().save(path_str(path.as_ref())?).map_err(err)
    }

    // --- Archivos incrustados ---

    pub fn add_embedded_file(
        &mut self,
        name: &str,
        contents: &[u8],
        options: EmbeddedFileOptions<'_>,
    ) -> Result<EmbeddedFileInfo> {
        let o = mupdf::pdf::EmbeddedFileOptions {
            filename: options.filename,
            mime_type: options.mime_type,
            created: options.created,
            modified: options.modified,
            add_checksum: options.add_checksum,
        };
        self.pdf_mut()
            .add_embedded_file(name, contents, o)
            .map(embedded_from)
            .map_err(err)
    }

    pub fn embedded_files(&self) -> Result<Vec<EmbeddedFileInfo>> {
        Ok(self
            .pdf()
            .embedded_files()
            .map_err(err)?
            .into_iter()
            .map(embedded_from)
            .collect())
    }

    pub fn embedded_file(&self, name: &str) -> Result<Option<EmbeddedFileInfo>> {
        self.pdf()
            .embedded_file(name)
            .map(|o| o.map(embedded_from))
            .map_err(err)
    }

    pub fn load_embedded_file(&self, name: &str) -> Result<Option<Vec<u8>>> {
        self.pdf().load_embedded_file(name).map_err(err)
    }

    pub fn verify_embedded_file_checksum(&self, name: &str) -> Result<Option<bool>> {
        self.pdf().verify_embedded_file_checksum(name).map_err(err)
    }

    pub fn delete_embedded_file(&mut self, name: &str) -> Result<bool> {
        self.pdf_mut().delete_embedded_file(name).map_err(err)
    }

    // --- Capas ---

    pub fn optional_content_groups(&self) -> Result<Vec<OptionalContentGroup>> {
        Ok(self
            .pdf()
            .optional_content_groups()
            .map_err(err)?
            .into_iter()
            .map(|g| OptionalContentGroup {
                xref: g.reference.xref(),
                name: g.name,
                enabled: g.enabled,
            })
            .collect())
    }

    pub fn add_optional_content_group(&mut self, name: &str) -> Result<i32> {
        self.pdf_mut()
            .add_optional_content_group(name)
            .map(|r| r.xref())
            .map_err(err)
    }

    /// Cambia la visibilidad por defecto de la capa `xref` (se guarda en el archivo).
    pub fn set_optional_content_enabled(&mut self, xref: i32, enabled: bool) -> Result<()> {
        let r = mupdf::pdf::OptionalContentRef::new(xref).map_err(err)?;
        self.pdf_mut().set_optional_content_enabled(r, enabled).map_err(err)
    }

    pub fn optional_content_enabled(&self, xref: i32) -> Result<bool> {
        let r = mupdf::pdf::OptionalContentRef::new(xref).map_err(err)?;
        self.pdf().optional_content_enabled(r).map_err(err)
    }

    // --- Marcadores ---

    /// Sustituye el árbol de marcadores.
    pub fn set_outlines(&mut self, toc: &[Outline]) -> Result<()> {
        let conv = outlines_to(toc);
        self.pdf_mut().set_outlines(&conv).map_err(err)
    }

    pub fn delete_outlines(&mut self) -> Result<()> {
        self.pdf_mut().delete_outlines().map_err(err)
    }
}

/// Copia de objetos entre documentos conservando lo compartido (ver [`PdfDocument::new_graft_map`]).
pub struct GraftMap {
    raw: mupdf::pdf::PdfGraftMap,
}

impl GraftMap {
    pub fn graft_object(&mut self, obj: &Object) -> Result<Object> {
        self.raw.graft_object(&obj.raw).map(Object::wrap).map_err(err)
    }
}

fn outlines_to(toc: &[Outline]) -> Vec<mupdf::Outline> {
    toc.iter()
        .map(|o| mupdf::Outline {
            title: o.title.clone(),
            uri: o.uri.clone(),
            dest: o.dest.map(|d| mupdf::link::LinkDestination {
                loc: mupdf::document::Location {
                    chapter: 0,
                    page_in_chapter: d.page,
                    page_number: d.page,
                },
                kind: crate::page::kind_to(d.kind),
            }),
            down: outlines_to(&o.down),
        })
        .collect()
}

fn position_to(p: InsertPosition) -> mupdf::pdf::InsertPosition {
    match p {
        InsertPosition::Append => mupdf::pdf::InsertPosition::Append,
        InsertPosition::Before(i) => mupdf::pdf::InsertPosition::Before(i),
        InsertPosition::After(i) => mupdf::pdf::InsertPosition::After(i),
    }
}

fn insert_result(r: mupdf::pdf::InsertPdfResult) -> InsertPdfResult {
    InsertPdfResult {
        inserted_pages: PageRange::new(r.inserted_pages.start, r.inserted_pages.end),
        page_count: r.page_count,
    }
}

fn embedded_from(e: mupdf::pdf::EmbeddedFileInfo) -> EmbeddedFileInfo {
    EmbeddedFileInfo {
        name: e.name,
        xref: e.xref,
        filename: e.filename,
        mime_type: e.mime_type,
        size: e.size,
        created: e.created,
        modified: e.modified,
    }
}

fn write_options_to(o: &WriteOptions) -> mupdf::pdf::PdfWriteOptions {
    use mupdf::pdf::Encryption as E;
    let mut w = mupdf::pdf::PdfWriteOptions::default();
    w.set_incremental(o.incremental)
        .set_pretty(o.pretty)
        .set_ascii(o.ascii)
        .set_compress(o.compress)
        .set_compress_images(o.compress_images)
        .set_compress_fonts(o.compress_fonts)
        .set_decompress(o.decompress)
        .set_garbage_level(o.garbage_level)
        .set_linear(o.linear)
        .set_clean(o.clean)
        .set_sanitize(o.sanitize)
        .set_appearance(o.appearance);
    if o.encryption != Encryption::Keep {
        w.set_encryption(match o.encryption {
            Encryption::Keep => E::Keep,
            Encryption::None => E::None,
            Encryption::Rc4_40 => E::Rc4_40,
            Encryption::Rc4_128 => E::Rc4_128,
            Encryption::Aes128 => E::Aes128,
            Encryption::Aes256 => E::Aes256,
        });
    }
    if let Some(p) = o.permissions {
        w.set_permissions(mupdf::pdf::Permission::from_bits_truncate(p.bits()));
    }
    if !o.owner_password.is_empty() {
        w.set_owner_password(&o.owner_password);
    }
    if !o.user_password.is_empty() {
        w.set_user_password(&o.user_password);
    }
    w
}
