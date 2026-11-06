//! Guardado: completo (con compresión, limpieza, deduplicado y cifrado) e incremental.
//!
//! Notas de comportamiento frente a la implementación con MuPDF:
//!
//! - `incremental`: el resultado es **el archivo completo** (original + sección añadida), también al escribir a
//!   un flujo o a un archivo que no existe. Solo se añaden los objetos cambiados desde que se abrió el documento.
//! - `linear`, `pretty`, `clean`, `sanitize` y `appearance` no hacen nada todavía (la linealización queda para
//!   qpdf como plan B; las apariencias son de W39; la reescritura de contenido, de W38/W39).
//! - Sin `compress` ni `garbage` el documento se escribe tal cual, con tabla de referencias clásica (los objetos
//!   que venían en flujos de objetos se escriben sueltos). Con `set_object_streams(true)` se usan flujos de
//!   objetos y de referencias (PDF 1.5; no válido para PDF/A-1).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Write;
use std::path::Path;
use std::sync::Arc;

use lopdf::Object as L;
use lopdf::encryption::crypt_filters::{Aes128CryptFilter, Aes256CryptFilter, CryptFilter};
use lopdf::xref::XrefType;
use lopdf::{
    EncryptionState, EncryptionVersion, IncrementalDocument, ObjectId, Permissions, SaveOptions, StringFormat,
};
use pliego_core::{Error, Result};

use super::document::{PdfDocument, md5};
use super::object::decode_stream;
use super::parse::object_to_string;
use super::{State, err, pdf_err};
use crate::options::{Encryption, WriteOptions};

/// Filtros que sabemos deshacer (los demás son de imagen y se dejan como están).
const DECODABLE: &[&[u8]] = &[
    b"FlateDecode",
    b"LZWDecode",
    b"ASCII85Decode",
    b"ASCIIHexDecode",
    b"RunLengthDecode",
];

impl PdfDocument {
    /// Serializa el documento a `w` con las opciones dadas.
    pub fn write_to_with_options<W: Write>(&self, w: &mut W, options: &WriteOptions) -> Result<u64> {
        let mut counted = Counting { inner: w, n: 0 };
        render_into(&self.st.borrow(), options, &mut counted)?;
        self.st.borrow_mut().unsaved = false;
        Ok(counted.n)
    }

    /// Serializa con las opciones por defecto.
    pub fn write_to<W: Write>(&self, w: &mut W) -> Result<u64> {
        self.write_to_with_options(w, &WriteOptions::default())
    }

    /// Serializa a memoria.
    pub fn to_bytes(&self, options: &WriteOptions) -> Result<Vec<u8>> {
        let bytes = self.render(options)?;
        self.st.borrow_mut().unsaved = false;
        Ok(bytes)
    }

    /// Escribe directamente en `path` (no atómico: para guardado seguro usa `pliego-document`). Con
    /// `incremental` se escribe el original más lo añadido, sin leer lo que hubiera en `path`.
    pub fn save_with_options(&self, path: impl AsRef<Path>, options: &WriteOptions) -> Result<()> {
        let path = path.as_ref();
        // Se escribe directamente al archivo: con un PDF de cientos de MB no se tiene el resultado entero en memoria.
        if self.st.borrow().view_only.is_some() || self.st.borrow().lean.is_some() {
            return Err(super::view::view_only_error());
        }
        let file = std::fs::File::create(path).map_err(|e| Error::io_at(path, e))?;
        let mut w = std::io::BufWriter::with_capacity(1 << 20, file);
        let written =
            render_into(&self.st.borrow(), options, &mut w).and_then(|()| w.flush().map_err(|e| Error::io_at(path, e)));
        if let Err(e) = written {
            // No se deja a medias un archivo que no es un PDF completo.
            drop(w);
            let _ = std::fs::remove_file(path);
            return Err(e);
        }
        self.st.borrow_mut().unsaved = false;
        Ok(())
    }

    /// Guarda en `out` un documento abierto con [`PdfDocument::open_lean`]: copia el archivo original tal cual y le añade
    /// al final los cambios (guardado incremental). No lee el original a memoria ni toca sus flujos.
    ///
    /// [`Error::Unsupported`] si el documento no está en modo ligero, si el archivo de origen cambió desde que se abrió o
    /// si algún cambio toca un flujo grande que no se cargó. En cualquier error `out` no queda escrito.
    pub fn save_lean(&self, out: impl AsRef<Path>) -> Result<()> {
        let out = out.as_ref();
        let st = self.st.borrow();
        let lean = st
            .lean
            .as_ref()
            .ok_or_else(|| Error::Unsupported("el documento no está abierto en modo ligero".into()))?;
        let meta = std::fs::metadata(&lean.path).map_err(|e| Error::io_at(&lean.path, e))?;
        if meta.len() != lean.len || meta.modified().ok() != lean.modified {
            return Err(Error::Unsupported(
                "el archivo cambió mientras se editaba; vuelve a abrirlo".into(),
            ));
        }
        // Un flujo omitido al cargar (sin contenido ni posición en el archivo) no puede volver a escribirse.
        let touches_skipped = st.dirty.iter().any(|id| {
            matches!(st.doc.objects.get(id), Some(L::Stream(s)) if s.content.is_empty() && s.start_position.is_none())
        });
        if touches_skipped {
            return Err(Error::Unsupported(
                "el cambio afecta a un flujo grande que no está cargado".into(),
            ));
        }
        let mut options = WriteOptions::default();
        options.set_incremental(true);
        let section = incremental(
            &st,
            &options,
            Prefix::Absent {
                len: lean.len as usize,
                last: lean.last_byte,
            },
        )?;
        let written = std::fs::copy(&lean.path, out)
            .map_err(|e| Error::io_at(out, e))
            .and_then(|n| {
                if n != lean.len {
                    return Err(Error::Unsupported("el archivo cambió mientras se guardaba".into()));
                }
                let mut f = std::fs::OpenOptions::new()
                    .append(true)
                    .open(out)
                    .map_err(|e| Error::io_at(out, e))?;
                f.write_all(&section)
                    .and_then(|()| f.flush())
                    .map_err(|e| Error::io_at(out, e))
            });
        if let Err(e) = written {
            let _ = std::fs::remove_file(out);
            return Err(e);
        }
        drop(st);
        self.st.borrow_mut().unsaved = false;
        Ok(())
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        self.save_with_options(path, &WriteOptions::default())
    }

    fn render(&self, options: &WriteOptions) -> Result<Vec<u8>> {
        render_state(&self.st.borrow(), options)
    }
}

/// Cuenta lo que se escribe.
struct Counting<'a, W: Write> {
    inner: &'a mut W,
    n: u64,
}

impl<W: Write> Write for Counting<'_, W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.n += n as u64;
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

/// Copia del estado tal como está, sin cifrar y sin tocar `unsaved`: es la que lee PDFium (véase `view.rs`).
pub(crate) fn snapshot_clear(st: &State) -> Result<Vec<u8>> {
    let mut options = WriteOptions::default();
    options.set_encryption(Encryption::None);
    render_state(st, &options)
}

fn render_state(st: &State, options: &WriteOptions) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    render_into(st, options, &mut out)?;
    Ok(out)
}

/// Escribe el documento en `out` (a memoria o a un archivo con búfer).
fn render_into<W: Write>(st: &State, options: &WriteOptions, out: &mut W) -> Result<()> {
    if st.locked {
        return Err(Error::Password);
    }
    if st.view_only.is_some() || st.lean.is_some() {
        return Err(super::view::view_only_error());
    }
    if options.incremental {
        return out.write_all(&incremental(st, options, Prefix::Bytes)?).map_err(io_err);
    }
    let mut doc = st.doc.clone();
    let original_encryption = st.base.as_ref().and_then(|b| b.encryption_state.clone());
    prepare(&mut doc, options, original_encryption)?;
    if options.object_streams {
        let so = SaveOptions::builder()
            .use_object_streams(true)
            .use_xref_streams(true)
            .compression_level(6)
            .build();
        doc.save_with_options(out, so).map_err(io_err)?;
    } else {
        doc.reference_table.cross_reference_type = XrefType::CrossReferenceTable;
        doc.save_to(out).map_err(io_err)?;
    }
    Ok(())
}

fn io_err(e: std::io::Error) -> Error {
    Error::Pdf(format!("no se pudo serializar el documento: {e}"))
}

// --- Guardado incremental ---

/// De dónde salen los bytes del archivo original que el guardado incremental deja delante de lo añadido.
enum Prefix {
    /// Los tiene el documento (`Base::bytes`).
    Bytes,
    /// No están en memoria (documento en modo ligero): solo se calcula lo que se añade. lopdf necesita un búfer de la
    /// longitud del original para numerar las posiciones; es un búfer a cero que nadie escribe ni lee (el sistema no
    /// reserva memoria física hasta que se toca) y `Tail` descarta su contenido.
    Absent { len: usize, last: u8 },
}

/// Recibe lo que lopdf escribe y se queda solo con lo posterior a los primeros `skip` bytes (el original).
struct Tail {
    skip: usize,
    out: Vec<u8>,
}

impl Write for Tail {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        let n = self.skip.min(data.len());
        self.skip -= n;
        self.out.extend_from_slice(&data[n..]);
        Ok(data.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn incremental(st: &State, options: &WriteOptions, prefix: Prefix) -> Result<Vec<u8>> {
    let base = st
        .base
        .as_ref()
        .filter(|b| !b.repaired && b.xref_start != 0 && !st.no_incremental)
        .ok_or_else(|| Error::Unsupported("este documento no admite guardado incremental".into()))?;
    if !matches!(options.encryption, Encryption::Keep)
        && base.encryption_state.is_some() == matches!(options.encryption, Encryption::None)
    {
        return Err(Error::Unsupported(
            "un guardado incremental no puede cambiar el cifrado".into(),
        ));
    }
    if base.encryption_state.is_none() && !matches!(options.encryption, Encryption::Keep | Encryption::None) {
        return Err(Error::Unsupported(
            "un guardado incremental no puede cambiar el cifrado".into(),
        ));
    }
    // Documento «anterior»: solo lo que lopdf necesita saber del archivo de origen.
    let mut prev = lopdf::Document::new();
    prev.trailer = base.trailer.clone();
    prev.xref_start = base.xref_start;
    prev.reference_table.cross_reference_type = base.xref_type;
    prev.max_id = base.max_id;
    prev.encryption_state = base.encryption_state.clone();
    prev.version = st.doc.version.clone();

    let (bytes, skip) = match prefix {
        Prefix::Bytes => (base.bytes.clone(), 0),
        Prefix::Absent { len, last } => {
            let mut zeros = vec![0u8; len];
            if let Some(b) = zeros.last_mut() {
                *b = last;
            }
            (zeros, len)
        }
    };
    let mut inc = IncrementalDocument::create_from(bytes, prev);
    inc.new_document.max_id = st.doc.max_id.max(base.max_id);
    for id in &st.dirty {
        if let Some(o) = st.doc.objects.get(id) {
            inc.new_document.objects.insert(*id, o.clone());
        }
    }
    // Objetos borrados: se anulan para que ya no se lean (un objeto liberado no se puede expresar en lopdf).
    for id in &st.deleted {
        if id.0 <= base.max_id && !st.doc.objects.contains_key(id) {
            inc.new_document.objects.insert(*id, L::Null);
        }
    }
    // `trailer` nuevo: el actual sin los datos de la tabla de referencias anterior.
    let mut trailer = st.doc.trailer.clone();
    for k in [
        "Prev",
        "XRefStm",
        "Size",
        "Type",
        "W",
        "Index",
        "Filter",
        "DecodeParms",
        "Length",
        "Encrypt",
    ] {
        trailer.remove(k.as_bytes());
    }
    trailer.set("Prev", L::Integer(base.xref_start as i64));
    inc.new_document.trailer = trailer;

    stream_options(&mut inc.new_document.objects, options, &HashSet::new());
    if skip == 0 {
        let mut out = Vec::new();
        inc.save_to(&mut out).map_err(io_err)?;
        return Ok(out);
    }
    let mut tail = Tail { skip, out: Vec::new() };
    inc.save_to(&mut tail).map_err(io_err)?;
    Ok(tail.out)
}

// --- Guardado completo ---

fn prepare(doc: &mut lopdf::Document, options: &WriteOptions, original: Option<EncryptionState>) -> Result<()> {
    // Los flujos de objetos y de referencias y la linealización se regeneran al escribir.
    doc.objects.retain(|_, o| match o {
        L::Stream(s) => !matches!(s.dict.get(b"Type").and_then(L::as_name), Ok(b"ObjStm" | b"XRef")),
        L::Dictionary(d) => !d.has(b"Linearized"),
        _ => true,
    });
    // Solo sobreviven las claves del `trailer` que describen el documento.
    let keep = ["Root", "Info", "ID"];
    let drop: Vec<Vec<u8>> = doc
        .trailer
        .iter()
        .filter(|(k, _)| !keep.contains(&String::from_utf8_lossy(k).as_ref()))
        .map(|(k, _)| k.clone())
        .collect();
    for k in drop {
        doc.trailer.remove(&k);
    }
    ensure_id(doc)?;

    let level = options.garbage_level.clamp(0, 4);
    if level >= 1 {
        prune(doc);
    }
    if level >= 2 {
        renumber(doc);
    }
    if level >= 3 {
        for _ in 0..5 {
            if !dedupe(doc, level >= 4) {
                break;
            }
        }
        prune(doc);
        renumber(doc);
    }

    let fonts = font_streams(doc);
    stream_options(&mut doc.objects, options, &fonts);
    encrypt(doc, options, original)
}

/// Aplica descompresión, compresión y ASCII a los flujos.
fn stream_options(objects: &mut BTreeMap<ObjectId, L>, options: &WriteOptions, fonts: &HashSet<ObjectId>) {
    for (id, obj) in objects.iter_mut() {
        let L::Stream(s) = obj else { continue };
        if options.decompress {
            decompress(s);
        }
        if options.compress && !options.decompress && can_compress(s, options, fonts.contains(id)) {
            let _ = s.compress();
        }
        if options.ascii {
            ascii_hex(s);
        }
        // Una longitud indirecta se vuelve directa (los flujos se escriben con su longitud real).
        let n = s.content.len() as i64;
        s.dict.set("Length", n);
    }
}

fn can_compress(s: &lopdf::Stream, options: &WriteOptions, is_font: bool) -> bool {
    if !s.allows_compression || s.dict.has(b"Filter") {
        return false;
    }
    if matches!(s.dict.get(b"Type").and_then(L::as_name), Ok(b"Metadata")) {
        // PDF/A y los lectores esperan los metadatos XMP sin comprimir.
        return false;
    }
    if matches!(s.dict.get(b"Subtype").and_then(L::as_name), Ok(b"Image")) {
        return options.compress_images;
    }
    if is_font || s.dict.has(b"Length1") {
        return options.compress_fonts;
    }
    true
}

/// Descomprime un flujo si todos sus filtros se saben deshacer.
fn decompress(s: &mut lopdf::Stream) {
    let Ok(filters) = s.filters() else { return };
    if filters.is_empty() || !filters.iter().all(|f| DECODABLE.contains(f)) {
        return;
    }
    if let Ok(data) = decode_stream(s) {
        s.set_plain_content(data);
    }
}

fn ascii_hex(s: &mut lopdf::Stream) {
    if s.content.is_empty() || matches!(s.dict.get(b"Type").and_then(L::as_name), Ok(b"Metadata")) {
        return;
    }
    let already = s
        .filters()
        .map(|f| f.first().is_some_and(|f| *f == b"ASCIIHexDecode"))
        .unwrap_or(false);
    if already {
        return;
    }
    let mut hex = String::with_capacity(s.content.len() * 2 + 1);
    for b in &s.content {
        use std::fmt::Write as _;
        let _ = write!(hex, "{b:02X}");
    }
    hex.push('>');
    let new_filters = match s.dict.get(b"Filter") {
        Ok(L::Name(n)) => vec![L::Name(b"ASCIIHexDecode".to_vec()), L::Name(n.clone())],
        Ok(L::Array(a)) => std::iter::once(L::Name(b"ASCIIHexDecode".to_vec()))
            .chain(a.iter().cloned())
            .collect(),
        _ => vec![L::Name(b"ASCIIHexDecode".to_vec())],
    };
    if let Ok(parms) = s.dict.get(b"DecodeParms").cloned() {
        let list = match parms {
            L::Array(a) => std::iter::once(L::Null).chain(a).collect(),
            other => vec![L::Null, other],
        };
        s.dict.set("DecodeParms", L::Array(list));
    }
    s.dict.set(
        "Filter",
        if new_filters.len() == 1 {
            new_filters.into_iter().next().unwrap()
        } else {
            L::Array(new_filters)
        },
    );
    s.set_content(hex.into_bytes());
}

/// Flujos de fuentes incrustadas (`/FontFile`, `/FontFile2`, `/FontFile3` de un descriptor).
fn font_streams(doc: &lopdf::Document) -> HashSet<ObjectId> {
    let mut out = HashSet::new();
    for o in doc.objects.values() {
        if let L::Dictionary(d) = o {
            for k in ["FontFile", "FontFile2", "FontFile3"] {
                if let Ok(L::Reference(id)) = d.get(k.as_bytes()) {
                    out.insert(*id);
                }
            }
        }
    }
    out
}

/// Ids de objeto a los que apunta un valor (recorre matrices y diccionarios).
fn collect_refs(o: &L, out: &mut Vec<ObjectId>) {
    match o {
        L::Reference(id) => out.push(*id),
        L::Array(a) => a.iter().for_each(|x| collect_refs(x, out)),
        L::Dictionary(d) => d.iter().for_each(|(_, v)| collect_refs(v, out)),
        L::Stream(s) => s.dict.iter().for_each(|(_, v)| collect_refs(v, out)),
        _ => {}
    }
}

/// Quita los objetos a los que no se llega desde el `trailer`.
pub(super) fn prune(doc: &mut lopdf::Document) {
    let mut seen: HashSet<ObjectId> = HashSet::new();
    let mut work: Vec<ObjectId> = Vec::new();
    for (_, v) in doc.trailer.iter() {
        collect_refs(v, &mut work);
    }
    while let Some(id) = work.pop() {
        if !seen.insert(id) {
            continue;
        }
        if let Some(o) = doc.objects.get(&id) {
            collect_refs(o, &mut work);
        }
    }
    doc.objects.retain(|id, _| seen.contains(id));
}

fn remap_refs(o: &mut L, map: &HashMap<ObjectId, ObjectId>) {
    match o {
        L::Reference(id) => {
            if let Some(n) = map.get(id) {
                *id = *n;
            }
        }
        L::Array(a) => a.iter_mut().for_each(|x| remap_refs(x, map)),
        L::Dictionary(d) => d.iter_mut().for_each(|(_, v)| remap_refs(v, map)),
        L::Stream(s) => s.dict.iter_mut().for_each(|(_, v)| remap_refs(v, map)),
        _ => {}
    }
}

fn remap_all(doc: &mut lopdf::Document, map: &HashMap<ObjectId, ObjectId>) {
    for o in doc.objects.values_mut() {
        remap_refs(o, map);
    }
    for (_, v) in doc.trailer.iter_mut() {
        remap_refs(v, map);
    }
}

/// Renumera los objetos de 1 en adelante, en el orden de los números actuales.
fn renumber(doc: &mut lopdf::Document) {
    let mut map = HashMap::new();
    let mut next = 1u32;
    for id in doc.objects.keys() {
        if id.0 != next || id.1 != 0 {
            map.insert(*id, (next, 0));
        }
        next += 1;
    }
    if !map.is_empty() {
        let old = std::mem::take(&mut doc.objects);
        for (id, o) in old {
            doc.objects.insert(*map.get(&id).unwrap_or(&id), o);
        }
        remap_all(doc, &map);
    }
    doc.max_id = next - 1;
}

/// ¿Puede fusionarse este objeto con otro idéntico? (Páginas, anotaciones y nodos de árbol, nunca.)
fn mergeable(o: &L, streams: bool) -> bool {
    match o {
        L::Stream(s) => {
            streams && !s.dict.has(b"Type")
                || streams && matches!(s.dict.get(b"Type").and_then(L::as_name), Ok(b"XObject" | b"ExtGState"))
        }
        L::Dictionary(d) => {
            if matches!(
                d.get(b"Type").and_then(L::as_name),
                Ok(b"Page" | b"Pages" | b"Catalog" | b"Annot" | b"OCG" | b"Outlines" | b"Filespec" | b"EmbeddedFile")
            ) {
                return false;
            }
            !["Parent", "P", "Kids", "Prev", "Next", "First", "Last", "T", "Subtype"]
                .iter()
                .any(|k| d.has(k.as_bytes()))
                || matches!(
                    d.get(b"Subtype").and_then(L::as_name),
                    Ok(b"Type1" | b"TrueType" | b"Type0" | b"Image" | b"Form")
                )
        }
        L::Array(_) | L::Name(_) | L::String(..) | L::Integer(_) | L::Real(_) | L::Boolean(_) => true,
        _ => false,
    }
}

fn fingerprint(o: &L) -> Vec<u8> {
    match o {
        L::Stream(s) => {
            let mut k = object_to_string(&L::Dictionary(s.dict.clone())).into_bytes();
            k.extend_from_slice(&md5(&s.content));
            k.extend_from_slice(&(s.content.len() as u64).to_le_bytes());
            k
        }
        other => object_to_string(other).into_bytes(),
    }
}

/// Fusiona objetos idénticos. Devuelve `true` si cambió algo.
fn dedupe(doc: &mut lopdf::Document, streams: bool) -> bool {
    let mut first: HashMap<Vec<u8>, ObjectId> = HashMap::new();
    let mut map: HashMap<ObjectId, ObjectId> = HashMap::new();
    for (id, o) in &doc.objects {
        if !mergeable(o, streams) {
            continue;
        }
        let key = fingerprint(o);
        match first.get(&key) {
            Some(canon) => {
                map.insert(*id, *canon);
            }
            None => {
                first.insert(key, *id);
            }
        }
    }
    if map.is_empty() {
        return false;
    }
    doc.objects.retain(|id, _| !map.contains_key(id));
    remap_all(doc, &map);
    true
}

fn random(buf: &mut [u8]) -> Result<()> {
    getrandom::fill(buf).map_err(|e| pdf_err(format!("no hay fuente de aleatoriedad: {e}")))
}

/// Asegura `/ID` en el `trailer` (hace falta para cifrar y lo espera todo escritor de PDF).
fn ensure_id(doc: &mut lopdf::Document) -> Result<()> {
    if matches!(doc.trailer.get(b"ID"), Ok(L::Array(a)) if a.len() == 2) {
        return Ok(());
    }
    let mut id = [0u8; 16];
    random(&mut id)?;
    let s = L::String(id.to_vec(), StringFormat::Hexadecimal);
    doc.trailer.set("ID", L::Array(vec![s.clone(), s]));
    Ok(())
}

/// Recorta `s` a `max` bytes sin partir un carácter.
fn truncate_utf8(s: &str, max: usize) -> &str {
    let mut end = s.len().min(max);
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

fn encrypt(doc: &mut lopdf::Document, options: &WriteOptions, original: Option<EncryptionState>) -> Result<()> {
    let permissions = Permissions::from_bits_truncate(u64::from(options.permissions().bits()));
    // La norma (ISO 32000-2, algoritmos 8 y 9) usa como máximo 127 bytes UTF-8 de la contraseña, y quien abre el
    // archivo los recorta igual; lopdf al escribir no lo hace, y una contraseña más larga no volvería a abrir.
    let user = truncate_utf8(options.user_password.as_str(), 127);
    let owner = if options.owner_password.is_empty() {
        user
    } else {
        truncate_utf8(options.owner_password.as_str(), 127)
    };
    let state = match options.encryption {
        Encryption::None => return Ok(()),
        Encryption::Keep => match original {
            Some(state) => state,
            None => return Ok(()),
        },
        Encryption::Rc4_40 => EncryptionState::try_from(EncryptionVersion::V1 {
            document: doc,
            owner_password: owner,
            user_password: user,
            permissions,
        })
        .map_err(err)?,
        Encryption::Rc4_128 => EncryptionState::try_from(EncryptionVersion::V2 {
            document: doc,
            owner_password: owner,
            user_password: user,
            key_length: 128,
            permissions,
        })
        .map_err(err)?,
        Encryption::Aes128 => {
            let filter: Arc<dyn CryptFilter> = Arc::new(Aes128CryptFilter);
            EncryptionState::try_from(EncryptionVersion::V4 {
                document: doc,
                encrypt_metadata: true,
                crypt_filters: BTreeMap::from([(b"StdCF".to_vec(), filter)]),
                stream_filter: b"StdCF".to_vec(),
                string_filter: b"StdCF".to_vec(),
                owner_password: owner,
                user_password: user,
                permissions,
            })
            .map_err(err)?
        }
        Encryption::Aes256 => {
            let filter: Arc<dyn CryptFilter> = Arc::new(Aes256CryptFilter);
            let mut key = [0u8; 32];
            random(&mut key)?;
            if doc.version.as_str() < "1.7" {
                doc.version = "1.7".into();
            }
            EncryptionState::try_from(EncryptionVersion::V5 {
                encrypt_metadata: true,
                crypt_filters: BTreeMap::from([(b"StdCF".to_vec(), filter)]),
                file_encryption_key: &key,
                stream_filter: b"StdCF".to_vec(),
                string_filter: b"StdCF".to_vec(),
                owner_password: owner,
                user_password: user,
                permissions,
            })
            .map_err(err)?
        }
    };
    doc.encrypt(&state).map_err(err)?;
    complete_encrypt_dict(doc, options.encryption);
    Ok(())
}

/// lopdf omite claves que la norma marca como opcionales pero que lectores estrictos (qpdf 12 avisa) esperan, y que
/// Acrobat y qpdf escriben siempre: `/Length` de la clave en bits (arriba) y, en cada filtro de cifrado, su `/Length`
/// en bytes y `/AuthEvent /DocOpen`.
fn complete_encrypt_dict(doc: &mut lopdf::Document, enc: Encryption) {
    let bits: i64 = match enc {
        Encryption::Rc4_40 => 40,
        Encryption::Rc4_128 | Encryption::Aes128 => 128,
        Encryption::Aes256 => 256,
        Encryption::Keep | Encryption::None => return,
    };
    let Ok(L::Reference(id)) = doc.trailer.get(b"Encrypt").cloned() else {
        return;
    };
    let Some(L::Dictionary(dict)) = doc.objects.get_mut(&id) else {
        return;
    };
    if !dict.has(b"Length") {
        dict.set("Length", bits);
    }
    if let Ok(L::Dictionary(cf)) = dict.get_mut(b"CF") {
        for (_, filter) in cf.iter_mut() {
            if let L::Dictionary(f) = filter {
                if !f.has(b"AuthEvent") {
                    f.set("AuthEvent", L::Name(b"DocOpen".to_vec()));
                }
                if !f.has(b"Length") {
                    f.set("Length", bits / 8);
                }
            }
        }
    }
}
