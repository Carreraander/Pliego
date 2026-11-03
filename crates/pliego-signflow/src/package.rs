//! Preparación del paquete: incrusta el manifiesto como adjunto mediante una actualización incremental.

use lopdf::{Dictionary, IncrementalDocument, Object, Stream, dictionary};
use pliego_core::{Error, Result};
use sha2::{Digest, Sha256};

use crate::manifest::{ATTACHMENT_NAME, Manifest, Signers};

/// Paquete listo para enviar al primer firmante.
#[derive(Debug, Clone)]
pub struct Package {
    /// El PDF con el manifiesto incrustado.
    pub pdf: Vec<u8>,
    pub manifest: Manifest,
}

pub(crate) fn pdf_err(e: lopdf::Error) -> Error {
    Error::Pdf(format!("PDF no válido: {e}"))
}

pub(crate) fn sha256_hex(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

/// Prepara `doc` para que lo firmen `signers` en orden. El PDF original no puede tener ya firmas.
pub fn prepare(doc: &[u8], signers: &Signers) -> Result<Package> {
    prepare_at(doc, signers, chrono::Utc::now().timestamp())
}

pub(crate) fn prepare_at(doc: &[u8], signers: &Signers, now: i64) -> Result<Package> {
    signers.validate()?;
    let prev = lopdf::Document::load_mem(doc).map_err(pdf_err)?;
    if prev.trailer.has(b"Encrypt") {
        return Err(Error::Unsupported(
            "los documentos cifrados todavía no se pueden preparar para firma".into(),
        ));
    }
    if has_signatures(&prev) {
        return Err(Error::InvalidInput(
            "el documento ya está firmado; prepare el flujo antes de la primera firma".into(),
        ));
    }
    if find_manifest_stream(&prev)?.is_some() {
        return Err(Error::InvalidInput("el documento ya es un paquete de firmas".into()));
    }
    let manifest = Manifest {
        version: 1,
        original_sha256: sha256_hex(doc),
        prepared_at: now,
        signers: signers.clone(),
    };
    let json = serde_json::to_vec_pretty(&manifest).map_err(|e| Error::Other(e.to_string()))?;

    let root_id = prev
        .trailer
        .get(b"Root")
        .and_then(Object::as_reference)
        .map_err(pdf_err)?;
    let mut inc = IncrementalDocument::create_from(doc.to_vec(), prev);
    inc.opt_clone_object_to_new_document(root_id).map_err(pdf_err)?;

    let stream = Stream::new(
        dictionary! {
            "Type" => "EmbeddedFile",
            "Subtype" => Object::Name(b"application/json".to_vec()),
            "Params" => dictionary! { "Size" => json.len() as i64 },
        },
        json,
    );
    let stream_id = inc.new_document.add_object(Object::Stream(stream));
    let mut filespec = dictionary! {
        "Type" => "Filespec",
        "AFRelationship" => "Data",
        "EF" => dictionary! { "F" => Object::Reference(stream_id), "UF" => Object::Reference(stream_id) },
    };
    filespec.set("F", Object::string_literal(ATTACHMENT_NAME));
    filespec.set("UF", Object::string_literal(ATTACHMENT_NAME));
    filespec.set(
        "Desc",
        Object::string_literal("Firmantes y orden del flujo de firmas de Pliego"),
    );
    let spec_id = inc.new_document.add_object(Object::Dictionary(filespec));

    register_attachment(&mut inc, root_id, spec_id)?;
    add_summary_page(&mut inc, root_id, &manifest)?;
    let mut out = Vec::new();
    inc.save_to(&mut out)
        .map_err(|e| Error::Pdf(format!("no se pudo escribir el PDF: {e}")))?;
    Ok(Package { pdf: out, manifest })
}

/// Añade el adjunto al árbol de nombres `/Names /EmbeddedFiles` del catálogo.
fn register_attachment(
    inc: &mut IncrementalDocument,
    root_id: lopdf::ObjectId,
    spec_id: lopdf::ObjectId,
) -> Result<()> {
    let entry = [Object::string_literal(ATTACHMENT_NAME), Object::Reference(spec_id)];
    let prev = inc.get_prev_documents().clone();
    let root = inc
        .new_document
        .get_object_mut(root_id)
        .and_then(Object::as_dict_mut)
        .map_err(pdf_err)?;
    let mut names = match root.get(b"Names").ok().map(|o| deref_dict(&prev, o)) {
        Some(Some(d)) => d,
        Some(None) => return Err(Error::Pdf("el diccionario Names del catálogo no es válido".into())),
        None => Dictionary::new(),
    };
    let mut tree = match names.get(b"EmbeddedFiles").ok().map(|o| deref_dict(&prev, o)) {
        Some(Some(d)) => d,
        Some(None) => return Err(Error::Pdf("EmbeddedFiles no es válido".into())),
        None => Dictionary::new(),
    };
    if tree.has(b"Kids") {
        return Err(Error::Unsupported(
            "el árbol de adjuntos del documento es jerárquico; todavía no se admite".into(),
        ));
    }
    let mut list: Vec<Object> = match tree.get(b"Names").ok() {
        Some(Object::Array(a)) => a.clone(),
        Some(Object::Reference(id)) => prev
            .get_object(*id)
            .and_then(Object::as_array)
            .map_err(pdf_err)?
            .clone(),
        _ => Vec::new(),
    };
    // El árbol de nombres va ordenado por clave: se inserta antes de la primera clave mayor.
    let key = ATTACHMENT_NAME.as_bytes();
    let pos = list
        .chunks(2)
        .position(|p| matches!(&p[0], Object::String(s, _) if s.as_slice() > key))
        .map_or(list.len(), |i| i * 2);
    list.splice(pos..pos, entry);
    tree.set("Names", Object::Array(list));
    names.set("EmbeddedFiles", Object::Dictionary(tree));
    root.set("Names", Object::Dictionary(names));
    Ok(())
}

fn deref_dict(doc: &lopdf::Document, o: &Object) -> Option<Dictionary> {
    match o {
        Object::Dictionary(d) => Some(d.clone()),
        Object::Reference(id) => doc.get_dictionary(*id).ok().cloned(),
        _ => None,
    }
}

pub(crate) fn has_signatures(doc: &lopdf::Document) -> bool {
    doc.objects
        .values()
        .any(|o| matches!(o, Object::Dictionary(d) if d.has(b"ByteRange") && d.has(b"Contents")))
}

/// Contenido del adjunto del manifiesto, si el documento es un paquete.
fn find_manifest_stream(doc: &lopdf::Document) -> Result<Option<Vec<u8>>> {
    let Ok(root) = doc.catalog() else { return Ok(None) };
    let Some(names) = root.get(b"Names").ok().and_then(|o| deref_dict(doc, o)) else {
        return Ok(None);
    };
    let Some(tree) = names.get(b"EmbeddedFiles").ok().and_then(|o| deref_dict(doc, o)) else {
        return Ok(None);
    };
    let list = match tree.get(b"Names") {
        Ok(Object::Array(a)) => a.clone(),
        Ok(Object::Reference(id)) => doc.get_object(*id).and_then(Object::as_array).map_err(pdf_err)?.clone(),
        _ => return Ok(None),
    };
    for pair in list.chunks(2) {
        let [Object::String(name, _), spec] = pair else {
            continue;
        };
        if name.as_slice() != ATTACHMENT_NAME.as_bytes() {
            continue;
        }
        let Some(spec) = deref_dict(doc, spec) else { continue };
        let Some(ef) = spec.get(b"EF").ok().and_then(|o| deref_dict(doc, o)) else {
            continue;
        };
        let Ok(Object::Reference(sid)) = ef.get(b"F") else {
            continue;
        };
        let stream = doc.get_object(*sid).and_then(Object::as_stream).map_err(pdf_err)?;
        let data = stream.decompressed_content().unwrap_or_else(|_| stream.content.clone());
        return Ok(Some(data));
    }
    Ok(None)
}

/// Lee el manifiesto de un paquete.
pub fn read_manifest(doc: &[u8]) -> Result<Manifest> {
    let d = lopdf::Document::load_mem(doc).map_err(pdf_err)?;
    let raw = find_manifest_stream(&d)?
        .ok_or_else(|| Error::InvalidInput("el documento no es un paquete de firmas de Pliego".into()))?;
    serde_json::from_slice(&raw)
        .map_err(|e| Error::InvalidInput(format!("el manifiesto del paquete no es válido: {e}")))
}

/// Texto en Latin-1 (WinAnsi) escapado para una cadena literal PDF; lo que no cabe se sustituye por «?».
fn pdf_text(s: &str) -> Vec<u8> {
    let mut out = Vec::new();
    for c in s.chars() {
        let b = if (c as u32) < 256 && (c as u32) >= 32 {
            c as u8
        } else {
            b'?'
        };
        if matches!(b, b'(' | b')' | b'\\') {
            out.push(b'\\');
        }
        out.push(b);
    }
    out
}

/// Añade al final una «hoja de firmas» con los firmantes, su orden y la huella del original.
fn add_summary_page(inc: &mut IncrementalDocument, root_id: lopdf::ObjectId, m: &Manifest) -> Result<()> {
    let prev = inc.get_prev_documents().clone();
    let pages_id = prev
        .get_dictionary(root_id)
        .and_then(|c| c.get(b"Pages"))
        .and_then(Object::as_reference)
        .map_err(pdf_err)?;
    inc.opt_clone_object_to_new_document(pages_id).map_err(pdf_err)?;

    let mut lines: Vec<(f32, String)> = vec![
        (18.0, "Hoja de firmas".into()),
        (10.0, "Flujo de firmas local de Pliego".into()),
        (
            10.0,
            format!("Huella SHA-256 del documento original: {}", m.original_sha256),
        ),
        (10.0, String::new()),
        (12.0, "Firmantes, por orden de firma:".into()),
    ];
    for (i, s) in m.signers.0.iter().enumerate() {
        let mail = s.email.as_deref().map(|e| format!(" <{e}>")).unwrap_or_default();
        lines.push((11.0, format!("{}. {}{}", i + 1, s.name, mail)));
    }
    let mut content = b"BT\n".to_vec();
    let mut y = 780.0f32;
    for (size, text) in &lines {
        content.extend_from_slice(format!("/F1 {size} Tf 1 0 0 1 56 {y} Tm (").as_bytes());
        content.extend_from_slice(&pdf_text(text));
        content.extend_from_slice(b") Tj\n");
        y -= size + 8.0;
    }
    content.extend_from_slice(b"ET");
    let content_id = inc
        .new_document
        .add_object(Object::Stream(Stream::new(Dictionary::new(), content)));
    let font = dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica", "Encoding" => "WinAnsiEncoding" };
    let page = dictionary! {
        "Type" => "Page",
        "Parent" => Object::Reference(pages_id),
        "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        "Contents" => Object::Reference(content_id),
        "Resources" => dictionary! { "Font" => dictionary! { "F1" => font } },
    };
    let page_id = inc.new_document.add_object(Object::Dictionary(page));

    let pages = inc
        .new_document
        .get_object_mut(pages_id)
        .and_then(Object::as_dict_mut)
        .map_err(pdf_err)?;
    let count = pages.get(b"Count").and_then(Object::as_i64).unwrap_or(0);
    pages.set("Count", count + 1);
    match pages.get_mut(b"Kids") {
        Ok(Object::Array(k)) => k.push(Object::Reference(page_id)),
        _ => {
            return Err(Error::Unsupported(
                "el árbol de páginas del documento no es compatible".into(),
            ));
        }
    }
    Ok(())
}
