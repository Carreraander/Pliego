//! Firma PAdES por actualización incremental: el contenido original no se toca.

use std::collections::BTreeMap;
use std::path::Path;

use lopdf::{Dictionary, Object, ObjectId, dictionary};
use pliego_core::{Error, JobContext, Report, Result};
use serde::{Deserialize, Serialize};

use crate::cms::{OID_TIMESTAMP_TOKEN, add_unsigned_attribute, build_cms, signature_value};
use crate::doctimestamp::add_document_timestamp;
use crate::dss::{
    HttpRevocation, RevocationData, RevocationSource, StaticRevocation, add_dss, add_dss_vri, collect_revocation,
};
use crate::pdfwrite::{text_string, write_dict, write_object};
use crate::signer::{DigestAlg, Signer, subject_name};
use crate::timestamp::{HttpTsa, TimestampAuthority, request_token, token_certificates};

/// Nivel PAdES de la firma.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub enum PadesLevel {
    #[default]
    #[serde(rename = "b-b")]
    BB,
    /// Con sello de tiempo de la firma (RFC 3161).
    #[serde(rename = "b-t")]
    BT,
    /// Con sello de tiempo y datos de validación a largo plazo (DSS con certificados y revocación).
    #[serde(rename = "b-lt")]
    BLt,
    /// Como B-LT, más un sello de tiempo del documento (`DocTimeStamp`) que protege el DSS.
    #[serde(rename = "b-lta")]
    BLta,
}

/// Posición de una firma visible: página (desde 0) y rectángulo `[x, y, ancho, alto]` en puntos,
/// con el origen en la esquina inferior izquierda de la página.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct VisibleSignature {
    pub page: usize,
    pub rect: [f32; 4],
}

/// Idioma del texto que la firma deja en el documento (apariencia visible y nombre del campo). Es el de la interfaz
/// de la app; por defecto, español.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SignLang {
    #[default]
    Es,
    En,
}

impl SignLang {
    /// Líneas de la apariencia visible: quién firma, cuándo y, si se indican, el motivo y el lugar.
    pub fn appearance_lines(
        self,
        signer: &str,
        when: i64,
        reason: Option<&str>,
        location: Option<&str>,
    ) -> Vec<String> {
        let (signed_by, date, reason_label, location_label) = match self {
            SignLang::Es => ("Firmado digitalmente por", "Fecha", "Motivo", "Lugar"),
            SignLang::En => ("Digitally signed by", "Date", "Reason", "Location"),
        };
        let mut lines = vec![
            format!("{signed_by} {signer}"),
            format!("{date}: {}", pdf_date_human(when)),
        ];
        lines.extend(reason.map(|r| format!("{reason_label}: {r}")));
        lines.extend(location.map(|l| format!("{location_label}: {l}")));
        lines
    }

    /// Nombre del campo de firma cuando no se indica uno (`Firma12` / `Signature12`).
    pub fn field_name(self, n: u32) -> String {
        match self {
            SignLang::Es => format!("Firma{n}"),
            SignLang::En => format!("Signature{n}"),
        }
    }
}

/// Opciones de firma. Se pueden guardar en acciones en lote.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SignOptions {
    /// Idioma de la apariencia visible de la firma.
    pub lang: SignLang,
    pub digest: DigestAlg,
    pub level: PadesLevel,
    /// Servidor de sellado de tiempo (http) para los niveles B-T o superiores.
    pub timestamp_url: Option<String>,
    /// Listas CRL (DER) para el nivel B-LT; si no hay, se descargan de los certificados.
    pub crl_files: Vec<std::path::PathBuf>,
    /// Página (desde 0) donde se coloca el campo de firma invisible.
    pub page: usize,
    /// Firma de certificación (DocMDP): 1 = ningún cambio, 2 = rellenar formularios y firmar,
    /// 3 = además anotar. Solo puede ser la primera firma del documento.
    pub certify: Option<u8>,
    /// Si se indica, la firma es visible con su apariencia en esa página y rectángulo.
    pub visible: Option<VisibleSignature>,
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
            lang: SignLang::default(),
            digest: DigestAlg::Sha256,
            level: PadesLevel::BB,
            timestamp_url: None,
            crl_files: Vec::new(),
            page: 0,
            visible: None,
            certify: None,
            field_name: None,
            reason: None,
            location: None,
            contact: None,
            signing_time: None,
            contents_size: 12288,
        }
    }
}

pub(crate) fn pdf_err(e: lopdf::Error) -> Error {
    Error::Pdf(format!("PDF no válido para firmar: {e}"))
}

fn pdf_date_human(unix: i64) -> String {
    let dt = chrono::DateTime::from_timestamp(unix, 0).unwrap_or_default();
    dt.format("%Y-%m-%d %H:%M:%S UTC").to_string()
}

pub(crate) fn pdf_date(unix: i64) -> String {
    let dt = chrono::DateTime::from_timestamp(unix, 0).unwrap_or_default();
    format!("D:{}+00'00'", dt.format("%Y%m%d%H%M%S"))
}

/// Última posición indicada por `startxref` y si la sección anterior es una tabla clásica.
pub(crate) fn last_startxref(data: &[u8]) -> Result<(usize, bool)> {
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
pub(crate) struct Changes<'a> {
    pub(crate) doc: &'a lopdf::Document,
    pub(crate) objects: BTreeMap<ObjectId, Object>,
}

impl Changes<'_> {
    /// Copia de trabajo de un objeto: la ya modificada o un clon del original.
    pub(crate) fn get(&mut self, id: ObjectId) -> Result<&mut Object> {
        if !self.objects.contains_key(&id) {
            let o = self.doc.get_object(id).map_err(pdf_err)?.clone();
            self.objects.insert(id, o);
        }
        Ok(self.objects.get_mut(&id).expect("recién insertado"))
    }

    /// Añade `new_ref` al array `key` de `owner`, sea directo, indirecto o inexistente.
    pub(crate) fn push_into_array(&mut self, owner: ObjectId, key: &[u8], new_ref: ObjectId) -> Result<()> {
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

    pub(crate) fn dict(&mut self, id: ObjectId) -> Result<&mut Dictionary> {
        match self.get(id)? {
            Object::Dictionary(d) => Ok(d),
            _ => Err(Error::Pdf("se esperaba un diccionario".into())),
        }
    }
}

/// Escribe la sección de referencias cruzadas y la cola del archivo para una actualización incremental.
/// `offsets` lleva las posiciones de los objetos escritos; `size` es el nuevo número de objetos.
pub(crate) fn write_xref(
    out: &mut Vec<u8>,
    doc: &lopdf::Document,
    prev: usize,
    prev_is_table: bool,
    offsets: &mut BTreeMap<u32, (usize, u16)>,
    new_size: u32,
) {
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
        write_dict(&trailer, out);
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
        write_dict(&trailer, out);
        out.extend_from_slice(b"\nstream\n");
        out.extend_from_slice(&data);
        out.extend_from_slice(format!("\nendstream\nendobj\nstartxref\n{xref_start}\n%%EOF\n").as_bytes());
    }
}

/// Busca un campo de firma vacío por nombre. Devuelve su id y su rectángulo, o `None` si no existe.
fn find_signature_field(doc: &lopdf::Document, name: &str) -> Result<Option<(ObjectId, [f32; 4])>> {
    let catalog = doc.catalog().map_err(pdf_err)?;
    let acro = match catalog.get(b"AcroForm") {
        Ok(Object::Reference(r)) => doc.get_dictionary(*r).ok(),
        Ok(Object::Dictionary(d)) => Some(d),
        _ => None,
    };
    let Some(acro) = acro else { return Ok(None) };
    let fields = match acro.get(b"Fields") {
        Ok(Object::Reference(r)) => doc.get_object(*r).ok().and_then(|o| o.as_array().ok()),
        Ok(Object::Array(a)) => Some(a),
        _ => None,
    };
    let mut stack: Vec<ObjectId> = fields
        .map(|a| a.iter().filter_map(|o| o.as_reference().ok()).collect())
        .unwrap_or_default();
    while let Some(id) = stack.pop() {
        let Ok(d) = doc.get_dictionary(id) else { continue };
        if let Ok(Object::Array(kids)) = d.get(b"Kids") {
            stack.extend(kids.iter().filter_map(|o| o.as_reference().ok()));
        }
        let title = match d.get(b"T") {
            Ok(Object::String(t, _)) => String::from_utf8_lossy(t).into_owned(),
            _ => continue,
        };
        if title != name {
            continue;
        }
        if d.get(b"FT").ok() != Some(&Object::Name(b"Sig".to_vec())) {
            return Err(Error::InvalidInput(format!(
                "el campo «{name}» no es un campo de firma"
            )));
        }
        if d.get(b"V").is_ok_and(|v| !matches!(v, Object::Null)) {
            return Err(Error::InvalidInput(format!("el campo «{name}» ya está firmado")));
        }
        let rect: Vec<f32> = d
            .get(b"Rect")
            .ok()
            .and_then(|o| o.as_array().ok())
            .map(|a| a.iter().filter_map(|n| n.as_float().ok()).collect())
            .unwrap_or_default();
        let rect = match rect[..] {
            [a, b, c, d] => [a.min(c), b.min(d), a.max(c), b.max(d)],
            _ => [0.0; 4],
        };
        return Ok(Some((id, rect)));
    }
    Ok(None)
}

/// Añade el campo al AcroForm del catálogo (lo crea si no existe) y marca `SigFlags = 3`.
pub(crate) fn register_field(ch: &mut Changes, root_id: ObjectId, field_id: ObjectId) -> Result<()> {
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
    Ok(())
}

/// Marca `SigFlags = 3` en el AcroForm existente.
fn set_sig_flags(ch: &mut Changes, root_id: ObjectId) -> Result<()> {
    let acro = ch.dict(root_id)?.get(b"AcroForm").ok().cloned();
    match acro {
        Some(Object::Reference(r)) => ch.dict(r)?.set("SigFlags", 3),
        Some(Object::Dictionary(mut a)) => {
            a.set("SigFlags", 3);
            ch.dict(root_id)?.set("AcroForm", Object::Dictionary(a));
        }
        _ => {}
    }
    Ok(())
}

/// Firma `input` (bytes de un PDF) y devuelve el PDF firmado.
pub fn sign_bytes(input: &[u8], signer: &dyn Signer, opts: &SignOptions) -> Result<Vec<u8>> {
    let http;
    let tsa: Option<&dyn TimestampAuthority> = match (&opts.timestamp_url, opts.level) {
        (_, PadesLevel::BB) => None,
        (Some(url), _) => {
            http = HttpTsa { url: url.clone() };
            Some(&http)
        }
        (None, _) => {
            return Err(Error::InvalidInput(
                "este nivel de firma necesita la dirección de un servidor de sellado de tiempo".into(),
            ));
        }
    };
    let static_rev;
    let rev: Option<&dyn RevocationSource> = if opts.level < PadesLevel::BLt {
        None
    } else if opts.crl_files.is_empty() {
        Some(&HttpRevocation)
    } else {
        let crls = opts
            .crl_files
            .iter()
            .map(|p| std::fs::read(p).map_err(|e| Error::io_at(p, e)))
            .collect::<Result<Vec<_>>>()?;
        static_rev = StaticRevocation(RevocationData {
            crls,
            ocsps: Vec::new(),
        });
        Some(&static_rev)
    };
    sign_bytes_with(input, signer, opts, tsa, rev)
}

/// Como [`sign_bytes`], con la autoridad de sellado de tiempo indicada (p. ej. una local en pruebas).
pub fn sign_bytes_with_tsa(
    input: &[u8],
    signer: &dyn Signer,
    opts: &SignOptions,
    tsa: Option<&dyn TimestampAuthority>,
) -> Result<Vec<u8>> {
    sign_bytes_with(input, signer, opts, tsa, None)
}

/// Firma con las autoridades de tiempo y de revocación indicadas (necesarias según el nivel).
pub fn sign_bytes_with(
    input: &[u8],
    signer: &dyn Signer,
    opts: &SignOptions,
    tsa: Option<&dyn TimestampAuthority>,
    revocation: Option<&dyn RevocationSource>,
) -> Result<Vec<u8>> {
    if opts.level >= PadesLevel::BT && tsa.is_none() {
        return Err(Error::InvalidInput(
            "este nivel necesita un servidor de sellado de tiempo".into(),
        ));
    }
    if opts.level >= PadesLevel::BLt && revocation.is_none() {
        return Err(Error::InvalidInput(
            "el nivel B-LT necesita un origen de datos de revocación".into(),
        ));
    }
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
    let page_index = opts.visible.map_or(opts.page, |v| v.page);
    let page_id = *doc
        .get_pages()
        .get(&(page_index as u32 + 1))
        .ok_or_else(|| Error::InvalidInput(format!("la página {} no existe", page_index + 1)))?;
    if let Some(v) = opts.visible
        && !(v.rect[2] >= 20.0 && v.rect[3] >= 10.0)
    {
        return Err(Error::InvalidInput(
            "el recuadro de la firma es demasiado pequeño".into(),
        ));
    }
    if let Some(p) = opts.certify {
        if !(1..=3).contains(&p) {
            return Err(Error::InvalidInput(
                "el nivel de certificación debe ser 1, 2 o 3".into(),
            ));
        }
        let already_signed = doc
            .objects
            .values()
            .any(|o| o.as_dict().is_ok_and(|d| d.has(b"ByteRange")));
        if already_signed {
            return Err(Error::InvalidInput(
                "solo la primera firma de un documento puede certificarlo".into(),
            ));
        }
    }
    let (prev, prev_is_table) = last_startxref(input)?;
    let size = doc
        .trailer
        .get(b"Size")
        .and_then(Object::as_i64)
        .map_or(0, |s| s as u32)
        .max(doc.max_id + 1);
    let sig_id = (size, 0u16);
    let field_id = (size + 1, 0u16);
    let ap_id = (size + 2, 0u16);
    let chain = signer.certificate_chain();
    let leaf = chain
        .first()
        .ok_or_else(|| Error::InvalidInput("el firmante no tiene certificado".into()))?;
    let when = opts.signing_time.unwrap_or_else(|| chrono::Utc::now().timestamp());

    let mut ch = Changes {
        doc: &doc,
        objects: BTreeMap::new(),
    };

    let sig_lines = |w: f32, h: f32| {
        let lines = opts.lang.appearance_lines(
            &subject_name(leaf),
            when,
            opts.reason.as_deref(),
            opts.location.as_deref(),
        );
        crate::appearance::build(w, h, &lines)
    };
    let existing = match opts.field_name.as_deref() {
        Some(name) => find_signature_field(&doc, name)?,
        None => None,
    };
    let appearance;
    if let Some((fid, rect)) = existing {
        // Firma de un campo vacío que ya existe: se rellena su /V y, si tiene tamaño, su apariencia.
        let (w, h) = (rect[2] - rect[0], rect[3] - rect[1]);
        appearance = (w >= 20.0 && h >= 10.0).then(|| sig_lines(w, h));
        let d = ch.dict(fid)?;
        d.set("V", Object::Reference(sig_id));
        if appearance.is_some() {
            d.set("AP", dictionary! { "N" => Object::Reference(ap_id) });
        }
        set_sig_flags(&mut ch, root_id)?;
    } else {
        let field_name = opts.field_name.clone().unwrap_or_else(|| opts.lang.field_name(size));
        let mut field = dictionary! {
            "Type" => "Annot",
            "Subtype" => "Widget",
            "FT" => "Sig",
            "F" => 132,
            "Rect" => match opts.visible {
                Some(v) => vec![
                    Object::Real(v.rect[0]),
                    Object::Real(v.rect[1]),
                    Object::Real(v.rect[0] + v.rect[2]),
                    Object::Real(v.rect[1] + v.rect[3]),
                ],
                None => vec![0.into(), 0.into(), 0.into(), 0.into()],
            },
            "P" => Object::Reference(page_id),
            "V" => Object::Reference(sig_id),
        };
        field.set("T", Object::string_literal(field_name.as_str()));
        appearance = opts.visible.map(|v| sig_lines(v.rect[2], v.rect[3]));
        if appearance.is_some() {
            field.set("AP", dictionary! { "N" => Object::Reference(ap_id) });
        }
        ch.objects.insert(field_id, Object::Dictionary(field));
        ch.push_into_array(page_id, b"Annots", field_id)?;

        register_field(&mut ch, root_id, field_id)?;
    }

    if opts.certify.is_some() {
        let mut perms = match ch.dict(root_id)?.get(b"Perms").ok() {
            Some(Object::Dictionary(d)) => d.clone(),
            _ => Dictionary::new(),
        };
        perms.set("DocMDP", Object::Reference(sig_id));
        ch.dict(root_id)?.set("Perms", Object::Dictionary(perms));
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

    if let Some((dict, content)) = &appearance {
        offsets.insert(ap_id.0, (out.len(), 0));
        out.extend_from_slice(format!("{} 0 obj\n", ap_id.0).as_bytes());
        write_dict(dict, &mut out);
        out.extend_from_slice(b"\nstream\n");
        out.extend_from_slice(content);
        out.extend_from_slice(b"\nendstream\nendobj\n");
    }

    // Diccionario de firma con marcadores de posición de ByteRange y Contents.
    let sig_start = out.len();
    offsets.insert(sig_id.0, (sig_start, 0));
    out.extend_from_slice(format!("{} 0 obj\n", sig_id.0).as_bytes());
    out.extend_from_slice(b"<</Type/Sig/Filter/Adobe.PPKLite/SubFilter/ETSI.CAdES.detached");
    if let Some(p) = opts.certify {
        out.extend_from_slice(
            format!(
                "/Reference[<</Type/SigRef/TransformMethod/DocMDP/DigestMethod/SHA256/TransformParams<</Type/TransformParams/P {p}/V/1.2>>>>]"
            )
            .as_bytes(),
        );
    }
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
    let new_size = field_id
        .0
        .max(sig_id.0)
        .max(if appearance.is_some() { ap_id.0 } else { 0 })
        + 1;
    write_xref(&mut out, &doc, prev, prev_is_table, &mut offsets, new_size);

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
    let mut cms = build_cms(signer, opts.digest, &opts.digest.digest(&covered))?;
    let mut token_certs = Vec::new();
    if let (true, Some(tsa)) = (opts.level >= PadesLevel::BT, tsa) {
        let token = request_token(tsa, opts.digest, &signature_value(&cms)?)?;
        token_certs = token_certificates(&token)?;
        cms = add_unsigned_attribute(&cms, OID_TIMESTAMP_TOKEN, &token)?;
    }
    if cms.len() > opts.contents_size {
        return Err(Error::InvalidInput(format!(
            "la firma ({} bytes) no cabe en el espacio reservado ({} bytes); aumente contents_size",
            cms.len(),
            opts.contents_size
        )));
    }
    let hex = hex::encode_upper(&cms);
    out[contents_open + 1..contents_open + 1 + hex.len()].copy_from_slice(hex.as_bytes());
    if let (true, Some(source)) = (opts.level >= PadesLevel::BLt, revocation) {
        let mut certs = chain.clone();
        certs.extend(token_certs);
        let data = collect_revocation(&certs, source)?;
        if data.crls.is_empty() && data.ocsps.is_empty() {
            return Err(Error::Other(
                "no se obtuvieron datos de revocación (CRL u OCSP) para los certificados de la firma".into(),
            ));
        }
        let with_dss = add_dss_vri(&out, &certs, &data, Some(&cms))?;
        if let (PadesLevel::BLta, Some(tsa)) = (opts.level, tsa) {
            let (stamped, tsa_certs) = add_document_timestamp(&with_dss, tsa, opts.digest, opts.contents_size, when)?;
            // Validación del propio sello del documento, para que siga siendo comprobable más adelante.
            let tsa_data = collect_revocation(&tsa_certs, source)?;
            return add_dss(&stamped, &tsa_certs, &tsa_data);
        }
        return Ok(with_dss);
    }
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
