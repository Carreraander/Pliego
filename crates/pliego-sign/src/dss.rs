//! Datos de validación a largo plazo (PAdES B-LT): diccionario DSS con certificados y revocación.

use std::collections::BTreeMap;

use der::asn1::{Null, OctetString};
use der::{Decode, Encode, Sequence};
use lopdf::{Dictionary, Object, ObjectId};
use pliego_core::{Error, Result};
use sha1::{Digest, Sha1};
use spki::AlgorithmIdentifierOwned;
use x509_cert::Certificate;
use x509_cert::ext::pkix::name::{DistributionPointName, GeneralName};
use x509_cert::ext::pkix::{AuthorityInfoAccessSyntax, CrlDistributionPoints};
use x509_cert::serial_number::SerialNumber;

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

/// Descarga la CRL indicada en el certificado (puntos de distribución http) y pide una respuesta OCSP
/// al servidor que el certificado declara (acceso a la información de la autoridad), si conoce a su emisor.
pub struct HttpRevocation;

impl RevocationSource for HttpRevocation {
    fn revocation_for(&self, cert: &Certificate, issuer: Option<&Certificate>) -> Result<RevocationData> {
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
        if let Some(issuer) = issuer
            && let Ok(request) = build_ocsp_request(cert, issuer)
        {
            for url in ocsp_urls(cert) {
                let fetched = ureq::post(&url)
                    .header("Content-Type", "application/ocsp-request")
                    .send(&request[..])
                    .ok()
                    .and_then(|mut r| r.body_mut().with_config().limit(4 << 20).read_to_vec().ok());
                if let Some(bytes) = fetched
                    && ocsp_response_is_successful(&bytes)
                {
                    data.ocsps.push(bytes);
                    break;
                }
            }
        }
        Ok(data)
    }
}

#[derive(Sequence)]
struct OcspCertId {
    hash_algorithm: AlgorithmIdentifierOwned,
    issuer_name_hash: OctetString,
    issuer_key_hash: OctetString,
    serial_number: SerialNumber,
}

#[derive(Sequence)]
struct OcspSingleRequest {
    req_cert: OcspCertId,
}

#[derive(Sequence)]
struct OcspTbsRequest {
    request_list: Vec<OcspSingleRequest>,
}

#[derive(Sequence)]
struct OcspRequest {
    tbs_request: OcspTbsRequest,
}

/// Solicitud OCSP (RFC 6960) sin firma ni extensiones para `cert`, emitido por `issuer`. Usa SHA-1 en el
/// identificador del certificado, que todos los servidores aceptan.
pub fn build_ocsp_request(cert: &Certificate, issuer: &Certificate) -> Result<Vec<u8>> {
    let err = |e: der::Error| Error::Other(format!("solicitud OCSP no codificable: {e}"));
    let name = issuer.tbs_certificate.subject.to_der().map_err(err)?;
    let key = issuer
        .tbs_certificate
        .subject_public_key_info
        .subject_public_key
        .raw_bytes();
    OcspRequest {
        tbs_request: OcspTbsRequest {
            request_list: vec![OcspSingleRequest {
                req_cert: OcspCertId {
                    hash_algorithm: AlgorithmIdentifierOwned {
                        oid: const_oid::ObjectIdentifier::new_unwrap("1.3.14.3.2.26"),
                        parameters: Some(der::Any::from(Null)),
                    },
                    issuer_name_hash: OctetString::new(Sha1::digest(&name).to_vec()).map_err(err)?,
                    issuer_key_hash: OctetString::new(Sha1::digest(key).to_vec()).map_err(err)?,
                    serial_number: cert.tbs_certificate.serial_number.clone(),
                },
            }],
        },
    }
    .to_der()
    .map_err(err)
}

/// ¿Es una `OCSPResponse` con estado `successful` (0) y con cuerpo? No valida la firma del respondedor.
pub fn ocsp_response_is_successful(resp: &[u8]) -> bool {
    #[derive(Sequence)]
    struct Resp {
        status: der::Any,
        #[asn1(context_specific = "0", tag_mode = "EXPLICIT", optional = "true")]
        response_bytes: Option<der::Any>,
    }
    Resp::from_der(resp).is_ok_and(|r| r.status.value() == [0] && r.response_bytes.is_some())
}

/// URL de los respondedores OCSP (método de acceso `id-ad-ocsp`) del certificado.
fn ocsp_urls(cert: &Certificate) -> Vec<String> {
    let mut urls = Vec::new();
    for ext in cert.tbs_certificate.extensions.iter().flatten() {
        if ext.extn_id.to_string() != "1.3.6.1.5.5.7.1.1" {
            continue;
        }
        if let Ok(aia) = AuthorityInfoAccessSyntax::from_der(ext.extn_value.as_bytes()) {
            for ad in aia.0 {
                if ad.access_method.to_string() == "1.3.6.1.5.5.7.48.1"
                    && let GeneralName::UniformResourceIdentifier(u) = ad.access_location
                {
                    let u = u.to_string();
                    if u.starts_with("http://") || u.starts_with("https://") {
                        urls.push(u);
                    }
                }
            }
        }
    }
    urls
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
    add_dss_vri(input, certs, rev, None)
}

/// Como [`add_dss`]; con `signature` (los bytes del CMS de una firma) añade además la entrada VRI de esa
/// firma, con sus certificados y revocación, bajo la clave SHA-1 en mayúsculas de la firma (ETSI EN 319 142).
pub fn add_dss_vri(
    input: &[u8],
    certs: &[Certificate],
    rev: &RevocationData,
    signature: Option<&[u8]>,
) -> Result<Vec<u8>> {
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
    // Devuelve todas las referencias (previas y nuevas) y solo las nuevas.
    let mut add_all = |items: Vec<Vec<u8>>, mut refs: Vec<Object>, out: &mut Vec<u8>| -> (Vec<Object>, Vec<Object>) {
        let mut added = Vec::new();
        for data in items {
            let id = next;
            next += 1;
            offsets.insert(id, (write_stream(out, id, &data), 0));
            refs.push(Object::Reference((id, 0)));
            added.push(Object::Reference((id, 0)));
        }
        (refs, added)
    };
    let cert_ders = certs
        .iter()
        .map(|c| {
            c.to_der()
                .map_err(|e| Error::Other(format!("certificado no codificable: {e}")))
        })
        .collect::<Result<Vec<_>>>()?;
    let (certs_refs, new_certs) = add_all(cert_ders, existing_refs(&doc, &old_dss, b"Certs"), &mut out);
    let (crls_refs, new_crls) = add_all(rev.crls.clone(), existing_refs(&doc, &old_dss, b"CRLs"), &mut out);
    let (ocsps_refs, new_ocsps) = add_all(rev.ocsps.clone(), existing_refs(&doc, &old_dss, b"OCSPs"), &mut out);

    let mut dss = old_dss;
    dss.set("Certs", Object::Array(certs_refs));
    if !crls_refs.is_empty() {
        dss.set("CRLs", Object::Array(crls_refs));
    }
    if !ocsps_refs.is_empty() {
        dss.set("OCSPs", Object::Array(ocsps_refs));
    }
    if let Some(sig) = signature {
        let mut vri = match dss.get(b"VRI") {
            Ok(Object::Dictionary(d)) => d.clone(),
            Ok(Object::Reference(r)) => doc.get_dictionary(*r).ok().cloned().unwrap_or_default(),
            _ => Dictionary::new(),
        };
        let mut entry = Dictionary::new();
        entry.set("Type", Object::Name(b"VRI".to_vec()));
        for (key, refs) in [("Cert", new_certs), ("CRL", new_crls), ("OCSP", new_ocsps)] {
            if !refs.is_empty() {
                entry.set(key, Object::Array(refs));
            }
        }
        vri.set(hex::encode_upper(Sha1::digest(sig)), Object::Dictionary(entry));
        dss.set("VRI", Object::Dictionary(vri));
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
