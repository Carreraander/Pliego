//! Firma PAdES por actualización incremental: el contenido original no se toca.

use std::collections::BTreeMap;
use std::path::Path;

use lopdf::{Dictionary, Object, ObjectId, dictionary};
use pliego_core::{Error, JobContext, Report, Result};
use serde::{Deserialize, Serialize};

use crate::cms::build_cms;
use crate::pdfwrite::{text_string, write_dict, write_object};
use crate::signer::{DigestAlg, Signer, subject_name};

/// Opciones de firma. Se pueden guardar en acciones en lote.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SignOptions {
    pub digest: DigestAlg,
    /// Página (desde 0) donde se coloca el campo de firma. Invisible en esta versión.
    pub page: usize,
    pub field_name: Option<String>,
    pub reason: Option<String>,
    pub location: Option<String>,
    pub contact: Option<String>,
    /// Momento de la firma en segundos Unix; por defecto, ahora.
    pub signing_time: Option<i64>,
    /// Bytes reservados para la firma CMS (el doble en hexadecimal).
    pub contents_size: usize,
}

impl Default for SignOptions {
    fn default() -> Self {
        Self {
            digest: DigestAlg::Sha256,
            page: 0,
            field_name: None,
            reason: None,
            location: None,
            contact: None,
            signing_time: None,
            contents_size: 12288,
        }
    }
}

fn pdf_err(e: lopdf::Error) -> Error {
    Error::Pdf(format!("PDF no válido para firmar: {e}"))
}

fn pdf_date(unix: i64) -> String {
    let dt = chrono::DateTime::from_timestamp(unix, 0).unwrap_or_default();
    format!("D:{}+00'00'", dt.format("%Y%m%d%H%M%S"))
}

/// Última posición indicada por `startxref` y si la sección anterior es una tabla clásica.
fn last_startxref(data: &[u8]) -> Result<(usize, bool)> {
    let pos = data
        .windows(9)
        .rposition(|w| w == b"startxref")
        .ok_or_else(|| Error::Pdf("el PDF no tiene startxref".into()))?;
    let digits: String = data[pos + 9..]
        .iter()
        .skip_while(|b| b.is_ascii_whitespace())
        .take_while(|b| b.is_ascii_digit())
        .map(|&b| b as char)
        .collect();
    let offset: usize = digits.parse().map_err(|_| Error::Pdf("startxref ilegible".into()))?;
    let is_table = data.get(offset..offset + 4) == Some(b"xref");
    Ok((offset, is_table))
}

/// Objetos nuevos o modificados de la actualización.
struct Changes<'a> {
    doc: &'a lopdf::Document,
    objects: BTreeMap<ObjectId, Object>,
}

impl Changes<'_> {
    /// Copia de trabajo de un objeto: la ya modificada o un clon del original.
    fn get(&mut self, id: ObjectId) -> Result<&mut Object> {
        if !self.objects.contains_key(&id) {
            let o = self.doc.get_object(id).map_err(pdf_err)?.clone();
            self.objects.insert(id, o);
        }
        Ok(self.objects.get_mut(&id).expect("recién insertado"))
    }

    /// Añade `new_ref` al array `key` de `owner`, sea directo, indirecto o inexistente.
    fn push_into_array(&mut self, owner: ObjectId, key: &[u8], new_ref: ObjectId) -> Result<()> {
        let existing = match self.get(owner)? {
            Object::Dictionary(d) => d.get(key).ok().cloned(),
            _ => return Err(Error::Pdf("objeto contenedor no es un diccionario".into())),
        };
        match existing {
            Some(Object::Reference(arr_id)) => match self.get(arr_id)? {
                Object::Array(a) => a.push(Object::Reference(new_ref)),
                _ => return Err(Error::Pdf("se esperaba un array".into())),
            },
            Some(Object::Array(mut a)) => {
                a.push(Object::Reference(new_ref));
                self.dict(owner)?.set(key.to_vec(), Object::Array(a));
            }
            _ => self
                .dict(owner)?
                .set(key.to_vec(), Object::Array(vec![Object::Reference(new_ref)])),
        }
        Ok(())
    }

    fn dict(&mut self, id: ObjectId) -> Result<&mut Dictionary> {
        match self.get(id)? {
            Object::Dictionary(d) => Ok(d),
            _ => Err(Error::Pdf("se esperaba un diccionario".into())),
        }
    }
}

/// Firma `input` (bytes de un PDF) y devuelve el PDF firmado.
pub fn sign_bytes(input: &[u8], signer: &dyn Signer, opts: &SignOptions) -> Result<Vec<u8>> {
    let doc = lopdf::Document::load_mem(input).map_err(pdf_err)?;
    if doc.trailer.has(b"Encrypt") {
        return Err(Error::Unsupported(
            "firmar documentos cifrados todavía no está soportado".into(),
        ));
    }
    let root_id = doc
        .trailer
        .get(b"Root")
        .and_then(Object::as_reference)
        .map_err(pdf_err)?;
    let page_id = *doc
        .get_pages()
        .get(&(opts.page as u32 + 1))
        .ok_or_else(|| Error::InvalidInput(format!("la página {} no existe", opts.page + 1)))?;
    let (prev, prev_is_table) = last_startxref(input)?;
    let size = doc
        .trailer
        .get(b"Size")
        .and_then(Object::as_i64)
        .map_or(0, |s| s as u32)
        .max(doc.max_id + 1);
    let sig_id = (size, 0u16);
    let field_id = (size + 1, 0u16);

    let mut ch = Changes {
        doc: &doc,
        objects: BTreeMap::new(),
    };

    // Campo de firma invisible (widget) en la página.
    let field_name = opts.field_name.clone().unwrap_or_else(|| format!("Firma{}", size));
    let mut field = dictionary! {
        "Type" => "Annot",
        "Subtype" => "Widget",
        "FT" => "Sig",
        "F" => 132,
        "Rect" => vec![0.into(), 0.into(), 0.into(), 0.into()],
        "P" => Object::Reference(page_id),
        "V" => Object::Reference(sig_id),
    };
    field.set("T", Object::string_literal(field_name.as_str()));
    ch.objects.insert(field_id, Object::Dictionary(field));
    ch.push_into_array(page_id, b"Annots", field_id)?;

    // AcroForm con el campo y SigFlags = 3 (firmas existentes + añadir a un documento firmado).
    let acro = ch.get(root_id).and_then(|o| match o {
        Object::Dictionary(d) => Ok(d.get(b"AcroForm").ok().cloned()),
        _ => Err(Error::Pdf("catálogo no válido".into())),
    })?;
    match acro {
        Some(Object::Reference(acro_id)) => {
            ch.push_into_array(acro_id, b"Fields", field_id)?;
            ch.dict(acro_id)?.set("SigFlags", 3);
        }
        Some(Object::Dictionary(_)) => {
            // AcroForm directo dentro del catálogo: se reescribe el catálogo con el campo añadido.
            let Some(Object::Dictionary(mut a)) = ch.dict(root_id)?.get(b"AcroForm").ok().cloned() else {
                unreachable!("comprobado arriba");
            };
            match a.get(b"Fields").ok().cloned() {
                Some(Object::Array(mut f)) => {
                    f.push(Object::Reference(field_id));
                    a.set("Fields", Object::Array(f));
                }
                Some(Object::Reference(fid)) => match ch.get(fid)? {
                    Object::Array(f) => f.push(Object::Reference(field_id)),
                    _ => return Err(Error::Pdf("Fields no es un array".into())),
                },
                _ => a.set("Fields", vec![Object::Reference(field_id)]),
            }
            a.set("SigFlags", 3);
            ch.dict(root_id)?.set("AcroForm", Object::Dictionary(a));
        }
        _ => {
            let a = dictionary! { "Fields" => vec![Object::Reference(field_id)], "SigFlags" => 3 };
            ch.dict(root_id)?.set("AcroForm", Object::Dictionary(a));
        }
    }

    // Serialización de la actualización.
    let mut out = input.to_vec();
    if !out.ends_with(b"\n") {
        out.push(b'\n');
    }
    let mut offsets: BTreeMap<u32, (usize, u16)> = BTreeMap::new();
    for (id, obj) in &ch.objects {
        offsets.insert(id.0, (out.len(), id.1));
        out.extend_from_slice(format!("{} {} obj\n", id.0, id.1).as_bytes());
        write_object(obj, &mut out);
        out.extend_from_slice(b"\nendobj\n");
    }

    // Diccionario de firma con marcadores de posición de ByteRange y Contents.
    let sig_start = out.len();
    offsets.insert(sig_id.0, (sig_start, 0));
    let chain = signer.certificate_chain();
    let leaf = chain
        .first()
        .ok_or_else(|| Error::InvalidInput("el firmante no tiene certificado".into()))?;
    let when = opts.signing_time.unwrap_or_else(|| chrono::Utc::now().timestamp());
    out.extend_from_slice(format!("{} 0 obj\n", sig_id.0).as_bytes());
    out.extend_from_slice(b"<</Type/Sig/Filter/Adobe.PPKLite/SubFilter/ETSI.CAdES.detached");
    out.extend_from_slice(b"/ByteRange[0 0000000000 0000000000 0000000000]/Contents<");
    let contents_open = out.len() - 1;
    out.extend(std::iter::repeat_n(b'0', opts.contents_size * 2));
    out.extend_from_slice(b">/M(");
    out.extend_from_slice(pdf_date(when).as_bytes());
    out.extend_from_slice(b")/Name");
    text_string(&subject_name(leaf), &mut out);
    for (key, val) in [
        ("Reason", &opts.reason),
        ("Location", &opts.location),
        ("ContactInfo", &opts.contact),
    ] {
        if let Some(v) = val {
            out.extend_from_slice(format!("/{key}").as_bytes());
            text_string(v, &mut out);
        }
    }
    out.extend_from_slice(b">>\nendobj\n");
    let contents_close = contents_open + 1 + opts.contents_size * 2 + 1;

    // Tabla de referencias cruzadas y cola.
    let new_size = field_id.0.max(sig_id.0) + 1;
    let xref_start = out.len();
    let mut trailer = Dictionary::new();
    for key in [&b"Root"[..], b"Info", b"ID"] {
        if let Ok(v) = doc.trailer.get(key) {
            trailer.set(key.to_vec(), v.clone());
        }
    }
    trailer.set("Prev", prev as i64);
    if prev_is_table {
        trailer.set("Size", new_size as i64);
        out.extend_from_slice(b"xref\n");
        let nums: Vec<u32> = offsets.keys().copied().collect();
        let mut i = 0;
        while i < nums.len() {
            let mut j = i;
            while j + 1 < nums.len() && nums[j + 1] == nums[j] + 1 {
                j += 1;
            }
            out.extend_from_slice(format!("{} {}\n", nums[i], j - i + 1).as_bytes());
            for n in &nums[i..=j] {
                let (off, gen_) = offsets[n];
                out.extend_from_slice(format!("{off:010} {gen_:05} n \r\n").as_bytes());
            }
            i = j + 1;
        }
        out.extend_from_slice(b"trailer\n");
        write_dict(&trailer, &mut out);
        out.extend_from_slice(format!("\nstartxref\n{xref_start}\n%%EOF\n").as_bytes());
    } else {
        // Flujo de referencias (PDF 1.5): entradas de tipo 1 con campos de 1, 4 y 2 bytes.
        let xref_id = new_size;
        offsets.insert(xref_id, (xref_start, 0));
        let nums: Vec<u32> = offsets.keys().copied().collect();
        let mut data = Vec::new();
        let mut index = Vec::new();
        let mut i = 0;
        while i < nums.len() {
            let mut j = i;
            while j + 1 < nums.len() && nums[j + 1] == nums[j] + 1 {
                j += 1;
            }
            index.push(Object::Integer(nums[i] as i64));
            index.push(Object::Integer((j - i + 1) as i64));
            for n in &nums[i..=j] {
                let (off, gen_) = offsets[n];
                data.push(1);
                data.extend_from_slice(&(off as u32).to_be_bytes());
                data.extend_from_slice(&gen_.to_be_bytes());
            }
            i = j + 1;
        }
        trailer.set("Type", Object::Name(b"XRef".to_vec()));
        trailer.set("Size", (xref_id + 1) as i64);
        trailer.set("W", vec![1.into(), 4.into(), 2.into()]);
        trailer.set("Index", Object::Array(index));
        trailer.set("Length", data.len() as i64);
        out.extend_from_slice(format!("{xref_id} 0 obj\n").as_bytes());
        write_dict(&trailer, &mut out);
        out.extend_from_slice(b"\nstream\n");
        out.extend_from_slice(&data);
        out.extend_from_slice(format!("\nendstream\nendobj\nstartxref\n{xref_start}\n%%EOF\n").as_bytes());
    }

    // ByteRange definitivo, resumen del documento y firma CMS.
    let total = out.len();
    let br = format!(
        "[0 {:010} {:010} {:010}]",
        contents_open,
        contents_close,
        total - contents_close
    );
    let br_pos = out[sig_start..]
        .windows(11)
        .position(|w| w == b"/ByteRange[")
        .map(|p| sig_start + p + 10)
        .ok_or_else(|| Error::Other("marcador ByteRange no encontrado".into()))?;
    out[br_pos..br_pos + br.len()].copy_from_slice(br.as_bytes());
    let mut covered = out[..contents_open].to_vec();
    covered.extend_from_slice(&out[contents_close..]);
    let cms = build_cms(signer, opts.digest, &opts.digest.digest(&covered))?;
    if cms.len() > opts.contents_size {
        return Err(Error::InvalidInput(format!(
            "la firma ({} bytes) no cabe en el espacio reservado ({} bytes); aumente contents_size",
            cms.len(),
            opts.contents_size
        )));
    }
    let hex = hex::encode_upper(&cms);
    out[contents_open + 1..contents_open + 1 + hex.len()].copy_from_slice(hex.as_bytes());
    Ok(out)
}

/// Firma el PDF `input` y escribe el resultado en `out` (nunca sobrescribe el original).
pub fn sign(input: &Path, out: &Path, signer: &dyn Signer, opts: SignOptions, ctx: &JobContext) -> Result<Report> {
    if let (Ok(a), Ok(b)) = (input.canonicalize(), out.canonicalize())
        && a == b
    {
        return Err(Error::InvalidInput(
            "el archivo de salida no puede ser el mismo que el de entrada".into(),
        ));
    }
    ctx.step(0, 2, "Leyendo el documento")?;
    let data = std::fs::read(input).map_err(|e| Error::io_at(input, e))?;
    ctx.step(1, 2, "Firmando")?;
    let signed = sign_bytes(&data, signer, &opts)?;
    let dir = out
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let tmp = tempfile_in(dir)?;
    std::fs::write(&tmp, &signed).map_err(|e| Error::io_at(&tmp, e))?;
    std::fs::rename(&tmp, out).map_err(|e| Error::io_at(out, e))?;
    ctx.step(2, 2, "Listo")?;
    Ok(Report::with_output(out, "Documento firmado"))
}

fn tempfile_in(dir: &Path) -> Result<std::path::PathBuf> {
    let name = format!(".pliego-firma-{}.tmp", std::process::id());
    Ok(dir.join(name))
}
