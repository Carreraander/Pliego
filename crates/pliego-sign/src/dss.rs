//! Datos de validación a largo plazo (PAdES B-LT): diccionario DSS con certificados y revocación.

use std::collections::BTreeMap;

use der::{Decode, Encode};
use lopdf::{Dictionary, Object, ObjectId};
use pliego_core::{Error, Result};
use x509_cert::Certificate;
use x509_cert::ext::pkix::CrlDistributionPoints;
use x509_cert::ext::pkix::name::{DistributionPointName, GeneralName};

use crate::pdfwrite::write_object;
use crate::sign::{last_startxref, write_xref};

/// Respuestas de revocación en DER: listas CRL y respuestas OCSP.
#[derive(Debug, Clone, Default)]
pub struct RevocationData {
    pub crls: Vec<Vec<u8>>,
    pub ocsps: Vec<Vec<u8>>,
}

/// Origen de los datos de revocación de un certificado.
pub trait RevocationSource {
    fn revocation_for(&self, cert: &Certificate, issuer: Option<&Certificate>) -> Result<RevocationData>;
}

/// Datos de revocación ya obtenidos (p. ej. CRL descargadas aparte); se incluyen todos.
pub struct StaticRevocation(pub RevocationData);

impl RevocationSource for StaticRevocation {
    fn revocation_for(&self, _: &Certificate, _: Option<&Certificate>) -> Result<RevocationData> {
        Ok(self.0.clone())
    }
}

/// Descarga la CRL indicada en el certificado (puntos de distribución http). OCSP llegará más adelante.
pub struct HttpRevocation;

impl RevocationSource for HttpRevocation {
    fn revocation_for(&self, cert: &Certificate, _: Option<&Certificate>) -> Result<RevocationData> {
        let mut data = RevocationData::default();
        for url in crl_urls(cert) {
            let fetched = ureq::get(&url)
                .call()
                .ok()
                .and_then(|mut r| r.body_mut().with_config().limit(32 << 20).read_to_vec().ok());
            if let Some(bytes) = fetched {
                data.crls.push(bytes);
            }
        }
        Ok(data)
    }
}

fn crl_urls(cert: &Certificate) -> Vec<String> {
    let mut urls = Vec::new();
    for ext in cert.tbs_certificate.extensions.iter().flatten() {
        if ext.extn_id.to_string() != "2.5.29.31" {
            continue;
        }
        if let Ok(points) = CrlDistributionPoints::from_der(ext.extn_value.as_bytes()) {
            for p in points.0 {
                if let Some(DistributionPointName::FullName(names)) = p.distribution_point {
                    for n in names {
                        if let GeneralName::UniformResourceIdentifier(u) = n {
                            let u = u.to_string();
                            if u.starts_with("http://") {
                                urls.push(u);
                            }
                        }
                    }
                }
            }
        }
    }
    urls
}

/// Obtiene la revocación de cada certificado de la cadena (el emisor es el siguiente por nombre).
pub fn collect_revocation(certs: &[Certificate], source: &dyn RevocationSource) -> Result<RevocationData> {
    let mut all = RevocationData::default();
    for c in certs {
        // Una raíz autofirmada no se revoca: no tiene emisor distinto.
        if c.tbs_certificate.issuer == c.tbs_certificate.subject {
            continue;
        }
        let issuer = certs
            .iter()
            .find(|i| i.tbs_certificate.subject == c.tbs_certificate.issuer);
        let r = source.revocation_for(c, issuer)?;
        for x in r.crls {
            if !all.crls.contains(&x) {
                all.crls.push(x);
            }
        }
        for x in r.ocsps {
            if !all.ocsps.contains(&x) {
                all.ocsps.push(x);
            }
        }
    }
    Ok(all)
}

fn write_stream(out: &mut Vec<u8>, id: u32, data: &[u8]) -> usize {
    let start = out.len();
    out.extend_from_slice(format!("{id} 0 obj\n<</Length {}>>\nstream\n", data.len()).as_bytes());
    out.extend_from_slice(data);
    out.extend_from_slice(b"\nendstream\nendobj\n");
    start
}

/// Referencias que ya tiene un array del DSS (directo o indirecto).
fn existing_refs(doc: &lopdf::Document, dss: &Dictionary, key: &[u8]) -> Vec<Object> {
    let arr = match dss.get(key) {
        Ok(Object::Array(a)) => Some(a.clone()),
        Ok(Object::Reference(r)) => doc.get_object(*r).ok().and_then(|o| o.as_array().ok().cloned()),
        _ => None,
    };
    arr.unwrap_or_default()
}

/// Añade (o amplía) el DSS del documento en una nueva revisión incremental.
pub fn add_dss(input: &[u8], certs: &[Certificate], rev: &RevocationData) -> Result<Vec<u8>> {
    let pdf_err = |e: lopdf::Error| Error::Pdf(format!("PDF no válido: {e}"));
    let doc = lopdf::Document::load_mem(input).map_err(pdf_err)?;
    let root_id = doc
        .trailer
        .get(b"Root")
        .and_then(Object::as_reference)
        .map_err(pdf_err)?;
    let mut root = doc.get_dictionary(root_id).map_err(pdf_err)?.clone();
    let (prev, prev_is_table) = last_startxref(input)?;
    let mut next = doc
        .trailer
        .get(b"Size")
        .and_then(Object::as_i64)
        .map_or(0, |s| s as u32)
        .max(doc.max_id + 1);

    // DSS previo: se conserva lo que ya tenía.
    let old_dss = match root.get(b"DSS") {
        Ok(Object::Reference(r)) => doc.get_dictionary(*r).ok().cloned(),
        Ok(Object::Dictionary(d)) => Some(d.clone()),
        _ => None,
    }
    .unwrap_or_default();

    let mut out = input.to_vec();
    if !out.ends_with(b"\n") {
        out.push(b'\n');
    }
    let mut offsets: BTreeMap<u32, (usize, u16)> = BTreeMap::new();
    let mut add_all = |items: Vec<Vec<u8>>, mut refs: Vec<Object>, out: &mut Vec<u8>| -> Vec<Object> {
        for data in items {
            let id = next;
            next += 1;
            offsets.insert(id, (write_stream(out, id, &data), 0));
            refs.push(Object::Reference((id, 0)));
        }
        refs
    };
    let cert_ders = certs
        .iter()
        .map(|c| {
            c.to_der()
                .map_err(|e| Error::Other(format!("certificado no codificable: {e}")))
        })
        .collect::<Result<Vec<_>>>()?;
    let certs_refs = add_all(cert_ders, existing_refs(&doc, &old_dss, b"Certs"), &mut out);
    let crls_refs = add_all(rev.crls.clone(), existing_refs(&doc, &old_dss, b"CRLs"), &mut out);
    let ocsps_refs = add_all(rev.ocsps.clone(), existing_refs(&doc, &old_dss, b"OCSPs"), &mut out);

    let mut dss = old_dss;
    dss.set("Certs", Object::Array(certs_refs));
    if !crls_refs.is_empty() {
        dss.set("CRLs", Object::Array(crls_refs));
    }
    if !ocsps_refs.is_empty() {
        dss.set("OCSPs", Object::Array(ocsps_refs));
    }
    let dss_id: ObjectId = (next, 0);
    next += 1;
    offsets.insert(dss_id.0, (out.len(), 0));
    out.extend_from_slice(format!("{} 0 obj\n", dss_id.0).as_bytes());
    write_object(&Object::Dictionary(dss), &mut out);
    out.extend_from_slice(b"\nendobj\n");

    root.set("DSS", Object::Reference(dss_id));
    offsets.insert(root_id.0, (out.len(), root_id.1));
    out.extend_from_slice(format!("{} {} obj\n", root_id.0, root_id.1).as_bytes());
    write_object(&Object::Dictionary(root), &mut out);
    out.extend_from_slice(b"\nendobj\n");

    write_xref(&mut out, &doc, prev, prev_is_table, &mut offsets, next);
    Ok(out)
}
