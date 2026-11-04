//! Sello de tiempo del documento (PAdES B-LTA): una revisión final con un `DocTimeStamp` RFC 3161
//! que cubre todo el documento, incluido el DSS.

use std::collections::BTreeMap;

use lopdf::{Object, dictionary};
use pliego_core::{Error, Result};
use x509_cert::Certificate;

use crate::pdfwrite::write_object;
use crate::sign::{Changes, last_startxref, pdf_date, pdf_err, register_field, write_xref};
use crate::signer::DigestAlg;
use crate::timestamp::{TimestampAuthority, request_token, token_certificates};

/// Añade al final de `input` un sello de tiempo del documento. Devuelve el PDF y los certificados del sello
/// (para ampliar el DSS con ellos).
pub(crate) fn add_document_timestamp(
    input: &[u8],
    tsa: &dyn TimestampAuthority,
    alg: DigestAlg,
    contents_size: usize,
    when: i64,
) -> Result<(Vec<u8>, Vec<Certificate>)> {
    let doc = lopdf::Document::load_mem(input).map_err(pdf_err)?;
    let root_id = doc
        .trailer
        .get(b"Root")
        .and_then(Object::as_reference)
        .map_err(pdf_err)?;
    let page_id = *doc
        .get_pages()
        .get(&1)
        .ok_or_else(|| Error::InvalidInput("el documento no tiene páginas".into()))?;
    let (prev, prev_is_table) = last_startxref(input)?;
    let size = doc
        .trailer
        .get(b"Size")
        .and_then(Object::as_i64)
        .map_or(0, |s| s as u32)
        .max(doc.max_id + 1);
    let (sig_id, field_id) = ((size, 0u16), (size + 1, 0u16));

    let mut ch = Changes {
        doc: &doc,
        objects: BTreeMap::new(),
    };
    let field = dictionary! {
        "Type" => "Annot",
        "Subtype" => "Widget",
        "FT" => "Sig",
        "F" => 132,
        "Rect" => vec![0.into(), 0.into(), 0.into(), 0.into()],
        "P" => Object::Reference(page_id),
        "V" => Object::Reference(sig_id),
        "T" => Object::string_literal(format!("SelloDocumento{size}")),
    };
    ch.objects.insert(field_id, Object::Dictionary(field));
    ch.push_into_array(page_id, b"Annots", field_id)?;
    register_field(&mut ch, root_id, field_id)?;

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
    let sig_start = out.len();
    offsets.insert(sig_id.0, (sig_start, 0));
    out.extend_from_slice(format!("{} 0 obj\n", sig_id.0).as_bytes());
    out.extend_from_slice(b"<</Type/DocTimeStamp/Filter/Adobe.PPKLite/SubFilter/ETSI.RFC3161");
    out.extend_from_slice(b"/ByteRange[0 0000000000 0000000000 0000000000]/Contents<");
    let contents_open = out.len() - 1;
    out.extend(std::iter::repeat_n(b'0', contents_size * 2));
    out.extend_from_slice(b">/M(");
    out.extend_from_slice(pdf_date(when).as_bytes());
    out.extend_from_slice(b")>>\nendobj\n");
    let contents_close = contents_open + 1 + contents_size * 2 + 1;
    write_xref(
        &mut out,
        &doc,
        prev,
        prev_is_table,
        &mut offsets,
        field_id.0.max(sig_id.0) + 1,
    );

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
    let token = request_token(tsa, alg, &covered)?;
    if token.len() > contents_size {
        return Err(Error::InvalidInput(format!(
            "el sello del documento ({} bytes) no cabe en el espacio reservado ({contents_size} bytes)",
            token.len()
        )));
    }
    let hex = hex::encode_upper(&token);
    out[contents_open + 1..contents_open + 1 + hex.len()].copy_from_slice(hex.as_bytes());
    Ok((out, token_certificates(&token)?))
}
