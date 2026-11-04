//! Validación de firmas: integridad (ByteRange + resumen), firma del CMS y cadena de certificados.

use std::path::Path;

use cms::content_info::ContentInfo;
use cms::signed_data::{SignedData, SignerIdentifier};
use der::{Decode, Encode};
use lopdf::Object;
use pliego_core::{Error, Result};
use rsa::Pkcs1v15Sign;
use serde::{Deserialize, Serialize};
use signature::hazmat::PrehashVerifier;
use spki::SubjectPublicKeyInfoOwned;
use x509_cert::Certificate;

use crate::cms::{OID_MESSAGE_DIGEST, OID_TIMESTAMP_TOKEN, digest_from_oid};
use crate::signer::{DigestAlg, cert_der, subject_name};

/// Estado de la cadena de certificados del firmante.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChainStatus {
    /// Llega hasta una raíz de confianza.
    Trusted,
    /// La cadena es coherente pero la raíz no está entre las de confianza (p. ej. autofirmado).
    UntrustedRoot,
    /// Falta algún certificado intermedio.
    Incomplete,
    /// Una firma de la cadena no es correcta.
    Invalid,
}

/// Resultado de validar una firma.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureReport {
    pub field: Option<String>,
    pub signer: String,
    pub subject: String,
    pub signing_time: Option<String>,
    pub reason: Option<String>,
    pub location: Option<String>,
    /// El resumen del documento y la firma son correctos: el contenido firmado no se ha alterado.
    pub integrity: bool,
    /// La firma cubre el documento hasta el final; si no, hay cambios posteriores.
    pub covers_whole_document: bool,
    /// No hay cambios posteriores o son solo de los permitidos (DSS, nuevas firmas).
    pub modifications_allowed: bool,
    /// El documento tiene datos de validación a largo plazo (DSS).
    pub has_dss: bool,
    /// Nivel de certificación (DocMDP 1, 2 o 3) si es una firma de certificación.
    pub certification: Option<u8>,
    /// Nivel PAdES detectado.
    pub level: crate::PadesLevel,
    pub chain: ChainStatus,
    /// La firma lleva sello de tiempo RFC 3161.
    pub has_timestamp: bool,
    /// Detalle del sello de tiempo, si lo hay.
    pub timestamp: Option<crate::timestamp::TimestampInfo>,
    /// Sello de tiempo del documento (B-LTA) que cubre esta firma y su DSS, si lo hay.
    pub document_timestamp: Option<crate::timestamp::TimestampInfo>,
    pub problems: Vec<String>,
}

impl SignatureReport {
    /// Firma íntegra, sin cambios posteriores y con cadena de confianza.
    pub fn is_valid(&self) -> bool {
        self.integrity
            && self.modifications_allowed
            && self.chain == ChainStatus::Trusted
            && self.document_timestamp.as_ref().is_none_or(|t| t.valid)
    }
}

/// Opciones de validación.
#[derive(Debug, Clone, Default)]
pub struct ValidationOptions {
    /// Certificados raíz en los que se confía.
    pub trust_anchors: Vec<Certificate>,
}

pub fn validate(input: &Path) -> Result<Vec<SignatureReport>> {
    validate_with(input, &ValidationOptions::default())
}

pub fn validate_with(input: &Path, opts: &ValidationOptions) -> Result<Vec<SignatureReport>> {
    let data = std::fs::read(input).map_err(|e| Error::io_at(input, e))?;
    validate_bytes(&data, opts)
}

fn text(d: &lopdf::Dictionary, key: &[u8]) -> Option<String> {
    match d.get(key).ok()? {
        Object::String(b, _) if b.starts_with(&[0xFE, 0xFF]) => {
            let u: Vec<u16> = b[2..]
                .chunks(2)
                .filter(|c| c.len() == 2)
                .map(|c| u16::from_be_bytes([c[0], c[1]]))
                .collect();
            Some(String::from_utf16_lossy(&u))
        }
        Object::String(b, _) => Some(String::from_utf8_lossy(b).into_owned()),
        _ => None,
    }
}

/// `D:20261103120000+01'00'` → `2026-11-03 11:00:00 UTC`.
fn parse_pdf_date(s: &str) -> Option<String> {
    let s = s.strip_prefix("D:")?;
    let n = |a: usize, b: usize| s.get(a..b)?.parse::<i64>().ok();
    let dt = chrono::NaiveDate::from_ymd_opt(n(0, 4)? as i32, n(4, 6)? as u32, n(6, 8)? as u32)?.and_hms_opt(
        n(8, 10)? as u32,
        n(10, 12)? as u32,
        n(12, 14)? as u32,
    )?;
    let mut secs = dt.and_utc().timestamp();
    if let Some(sign) = s.get(14..15).filter(|c| *c == "+" || *c == "-") {
        let off = n(15, 17).unwrap_or(0) * 3600 + s.get(18..20).and_then(|m| m.parse::<i64>().ok()).unwrap_or(0) * 60;
        secs -= if sign == "+" { off } else { -off };
    }
    Some(
        chrono::DateTime::from_timestamp(secs, 0)?
            .format("%Y-%m-%d %H:%M:%S UTC")
            .to_string(),
    )
}

pub fn validate_bytes(data: &[u8], opts: &ValidationOptions) -> Result<Vec<SignatureReport>> {
    let doc = lopdf::Document::load_mem(data).map_err(|e| Error::Pdf(format!("no se pudo leer el PDF: {e}")))?;
    let has_dss = doc.catalog().is_ok_and(|c| c.has(b"DSS"));
    let mut reports = Vec::new();
    let mut doc_stamps = Vec::new();
    for (&id, obj) in &doc.objects {
        let Object::Dictionary(d) = obj else { continue };
        if !(d.has(b"ByteRange") && d.has(b"Contents")) {
            continue;
        }
        if d.get(b"SubFilter").ok() == Some(&Object::Name(b"ETSI.RFC3161".to_vec())) {
            doc_stamps.push(check_document_timestamp(data, d, opts));
            continue;
        }
        // Nombre del campo: el widget cuyo /V apunta a esta firma.
        let field = doc.objects.values().find_map(|o| match o {
            Object::Dictionary(f) if f.get(b"V").and_then(Object::as_reference).ok() == Some(id) => text(f, b"T"),
            _ => None,
        });
        let mut r = check_signature(data, d, field, opts);
        r.has_dss = has_dss;
        r.level = match (r.has_timestamp, has_dss) {
            (true, true) => crate::PadesLevel::BLt,
            (true, false) => crate::PadesLevel::BT,
            _ => crate::PadesLevel::BB,
        };
        reports.push(r);
    }
    // El sello más reciente (el último) es el que protege la firma a largo plazo.
    if let Some(stamp) = doc_stamps.pop() {
        for r in &mut reports {
            if !stamp.valid {
                r.problems.push("el sello de tiempo del documento no es válido".into());
            } else if r.level == crate::PadesLevel::BLt {
                r.level = crate::PadesLevel::BLta;
            }
            r.document_timestamp = Some(stamp.clone());
        }
    }
    Ok(reports)
}

/// Comprueba un `DocTimeStamp`: el token RFC 3161 de `/Contents` debe sellar los bytes cubiertos por su `ByteRange`.
fn check_document_timestamp(
    data: &[u8],
    d: &lopdf::Dictionary,
    opts: &ValidationOptions,
) -> crate::timestamp::TimestampInfo {
    let broken = |why: &str| crate::timestamp::TimestampInfo {
        time: why.to_string(),
        valid: false,
        signer: String::new(),
        chain: ChainStatus::Invalid,
    };
    let br: Vec<usize> = match d.get(b"ByteRange").and_then(Object::as_array) {
        Ok(a) => a.iter().filter_map(|o| o.as_i64().ok().map(|v| v as usize)).collect(),
        Err(_) => return broken("ByteRange ilegible"),
    };
    let [0, l1, s2, l2] = br[..] else {
        return broken("el sello no cubre el documento desde el principio");
    };
    if s2 < l1 + 2 || s2.checked_add(l2).is_none_or(|e| e > data.len()) {
        return broken("ByteRange fuera del documento");
    }
    let Ok(Object::String(contents, _)) = d.get(b"Contents") else {
        return broken("falta el contenido del sello");
    };
    let gap = &data[l1..s2];
    if gap.first() != Some(&b'<')
        || gap.last() != Some(&b'>')
        || hex::decode(&gap[1..gap.len() - 1]).ok().as_deref() != Some(&contents[..])
    {
        return broken("el sello no ocupa el hueco que deja ByteRange");
    }
    let Some(len) = der_prefix_len(contents).filter(|l| *l <= contents.len()) else {
        return broken("sello ilegible");
    };
    let mut covered = data[..l1].to_vec();
    covered.extend_from_slice(&data[s2..s2 + l2]);
    match crate::timestamp::check_token(&contents[..len], &covered, &opts.trust_anchors) {
        Ok(mut t) => {
            // El sello debe llegar hasta el final del archivo, salvo que solo se hayan añadido datos de validación.
            if s2 + l2 != data.len() && crate::changes::check_updates(data, s2 + l2, None).is_err() {
                t.valid = false;
            }
            t
        }
        Err(e) => broken(&e.to_string()),
    }
}

fn check_signature(
    data: &[u8],
    d: &lopdf::Dictionary,
    field: Option<String>,
    opts: &ValidationOptions,
) -> SignatureReport {
    let mut r = SignatureReport {
        field,
        signer: String::new(),
        subject: String::new(),
        signing_time: text(d, b"M").and_then(|m| parse_pdf_date(&m)),
        reason: text(d, b"Reason"),
        location: text(d, b"Location"),
        integrity: false,
        covers_whole_document: false,
        modifications_allowed: false,
        has_dss: false,
        certification: docmdp_level(d),
        level: crate::PadesLevel::BB,
        chain: ChainStatus::Incomplete,
        has_timestamp: false,
        timestamp: None,
        document_timestamp: None,
        problems: Vec::new(),
    };
    if let Err(msg) = check_inner(data, d, opts, &mut r) {
        r.problems.push(msg);
    }
    r
}

fn der_prefix_len(b: &[u8]) -> Option<usize> {
    // Longitud total de un TLV DER al inicio de `b` (el resto es relleno de ceros).
    let first = *b.get(1)?;
    if first < 0x80 {
        Some(2 + first as usize)
    } else {
        let n = (first & 0x7F) as usize;
        let len = b.get(2..2 + n)?.iter().fold(0usize, |acc, &x| (acc << 8) | x as usize);
        Some(2 + n + len)
    }
}

fn check_inner(
    data: &[u8],
    d: &lopdf::Dictionary,
    opts: &ValidationOptions,
    r: &mut SignatureReport,
) -> std::result::Result<(), String> {
    let br: Vec<usize> = d
        .get(b"ByteRange")
        .and_then(Object::as_array)
        .map_err(|_| "ByteRange ilegible")?
        .iter()
        .map(|o| o.as_i64().map(|v| v as usize))
        .collect::<std::result::Result<_, _>>()
        .map_err(|_| "ByteRange ilegible")?;
    let [0, l1, s2, l2] = br[..] else {
        return Err("la firma no cubre el documento desde el principio".into());
    };
    if s2 < l1 + 2 || s2.checked_add(l2).is_none_or(|e| e > data.len()) {
        return Err("ByteRange fuera del documento".into());
    }
    r.covers_whole_document = s2 + l2 == data.len();
    r.modifications_allowed = true;
    if !r.covers_whole_document {
        // Cambios posteriores: permitidos si solo añaden DSS, firmas o anotaciones de campos.
        if let Err(m) = crate::changes::check_updates(data, s2 + l2, r.certification) {
            r.modifications_allowed = false;
            r.problems.push(format!("cambios no permitidos tras la firma: {m}"));
        }
    }
    let contents = match d.get(b"Contents") {
        Ok(Object::String(b, _)) => b.clone(),
        _ => return Err("falta el contenido de la firma".into()),
    };
    // La zona sin firmar debe ser exactamente la cadena /Contents (evita ataques de envoltura).
    let gap = &data[l1..s2];
    if gap.first() != Some(&b'<')
        || gap.last() != Some(&b'>')
        || hex::decode(&gap[1..gap.len() - 1]).ok().as_deref() != Some(&contents[..])
    {
        return Err("la firma no ocupa el hueco que deja ByteRange".into());
    }
    let len = der_prefix_len(&contents)
        .filter(|l| *l <= contents.len())
        .ok_or("firma CMS ilegible")?;
    let info = ContentInfo::from_der(&contents[..len]).map_err(|e| format!("firma CMS ilegible: {e}"))?;
    let sd: SignedData = info
        .content
        .decode_as()
        .map_err(|e| format!("firma CMS ilegible: {e}"))?;
    let si = sd.signer_infos.0.iter().next().ok_or("la firma no contiene firmante")?;
    let mut covered = data[..l1].to_vec();
    covered.extend_from_slice(&data[s2..s2 + l2]);
    let (leaf, certs) = verify_cms(&sd, &covered)?;
    let leaf = &leaf;
    r.signer = subject_name(leaf);
    r.subject = leaf.tbs_certificate.subject.to_string();
    r.integrity = true;
    if let Some(a) = si
        .unsigned_attrs
        .as_ref()
        .and_then(|u| u.iter().find(|a| a.oid == OID_TIMESTAMP_TOKEN))
    {
        r.has_timestamp = true;
        match a.values.iter().next().and_then(|v| v.to_der().ok()) {
            Some(token) => match crate::timestamp::check_token(&token, si.signature.as_bytes(), &opts.trust_anchors) {
                Ok(t) => {
                    if !t.valid {
                        r.problems.push("el sello de tiempo no corresponde a esta firma".into());
                    }
                    r.timestamp = Some(t);
                }
                Err(e) => r.problems.push(format!("el sello de tiempo no es válido: {e}")),
            },
            None => r.problems.push("el sello de tiempo es ilegible".into()),
        }
    }

    r.chain = check_chain(leaf, &certs, &opts.trust_anchors);
    match r.chain {
        ChainStatus::Trusted => {}
        ChainStatus::UntrustedRoot => r
            .problems
            .push("el certificado no procede de una entidad de confianza".into()),
        ChainStatus::Incomplete => r.problems.push("la cadena de certificados está incompleta".into()),
        ChainStatus::Invalid => r.problems.push("la cadena de certificados no es válida".into()),
    }
    Ok(())
}

/// Algoritmos por OID de firma (RSA/ECDSA con SHA-2); si el OID no fija el resumen se usa el del CMS.
fn hash_for_sig_oid(oid: &str, fallback: DigestAlg) -> DigestAlg {
    match oid {
        "1.2.840.113549.1.1.12" | "1.2.840.10045.4.3.3" => DigestAlg::Sha384,
        "1.2.840.113549.1.1.13" | "1.2.840.10045.4.3.4" => DigestAlg::Sha512,
        "1.2.840.113549.1.1.11" | "1.2.840.10045.4.3.2" => DigestAlg::Sha256,
        _ => fallback,
    }
}

fn verify_signature(
    spki: &SubjectPublicKeyInfoOwned,
    sig_oid: &const_oid::ObjectIdentifier,
    fallback: DigestAlg,
    message: &[u8],
    sig: &[u8],
) -> bool {
    use rsa::pkcs8::DecodePublicKey;
    let alg = hash_for_sig_oid(&sig_oid.to_string(), fallback);
    let hashed = alg.digest(message);
    let Ok(spki_der) = spki.to_der() else { return false };
    match spki.algorithm.oid.to_string().as_str() {
        "1.2.840.113549.1.1.1" => {
            let Ok(key) = rsa::RsaPublicKey::from_public_key_der(&spki_der) else {
                return false;
            };
            let scheme = match alg {
                DigestAlg::Sha256 => Pkcs1v15Sign::new::<sha2::Sha256>(),
                DigestAlg::Sha384 => Pkcs1v15Sign::new::<sha2::Sha384>(),
                DigestAlg::Sha512 => Pkcs1v15Sign::new::<sha2::Sha512>(),
            };
            key.verify(scheme, &hashed, sig).is_ok()
        }
        "1.2.840.10045.2.1" => {
            let curve = spki
                .algorithm
                .parameters
                .as_ref()
                .and_then(|p| p.decode_as::<const_oid::ObjectIdentifier>().ok());
            match curve.map(|c| c.to_string()).as_deref() {
                Some("1.2.840.10045.3.1.7") => p256::ecdsa::VerifyingKey::from_public_key_der(&spki_der)
                    .ok()
                    .is_some_and(|k| {
                        p256::ecdsa::Signature::from_der(sig)
                            .ok()
                            .is_some_and(|s| k.verify_prehash(&hashed, &s).is_ok())
                    }),
                Some("1.3.132.0.34") => p384::ecdsa::VerifyingKey::from_public_key_der(&spki_der)
                    .ok()
                    .is_some_and(|k| {
                        p384::ecdsa::Signature::from_der(sig)
                            .ok()
                            .is_some_and(|s| k.verify_prehash(&hashed, &s).is_ok())
                    }),
                _ => false,
            }
        }
        _ => false,
    }
}

fn verify_cert_signed_by(cert: &Certificate, issuer: &Certificate) -> bool {
    let Ok(tbs) = cert.tbs_certificate.to_der() else {
        return false;
    };
    verify_signature(
        &issuer.tbs_certificate.subject_public_key_info,
        &cert.signature_algorithm.oid,
        DigestAlg::Sha256,
        &tbs,
        cert.signature.raw_bytes(),
    )
}

pub(crate) fn check_chain(leaf: &Certificate, embedded: &[Certificate], anchors: &[Certificate]) -> ChainStatus {
    let mut cur = leaf.clone();
    for _ in 0..10 {
        if let Ok(der) = cert_der(&cur)
            && anchors.iter().any(|a| cert_der(a).ok().as_deref() == Some(&der[..]))
        {
            return ChainStatus::Trusted;
        }
        let self_issued = cur.tbs_certificate.issuer == cur.tbs_certificate.subject;
        let issuer = anchors
            .iter()
            .chain(embedded.iter())
            .find(|c| c.tbs_certificate.subject == cur.tbs_certificate.issuer && verify_cert_signed_by(&cur, c));
        match issuer {
            Some(i) if self_issued => {
                let _ = i;
                return ChainStatus::UntrustedRoot;
            }
            Some(i) => {
                if anchors.iter().any(|a| {
                    a.tbs_certificate.subject == i.tbs_certificate.subject
                        && a.tbs_certificate.subject_public_key_info == i.tbs_certificate.subject_public_key_info
                }) {
                    return ChainStatus::Trusted;
                }
                cur = i.clone();
            }
            None if self_issued => return ChainStatus::Invalid,
            None => {
                let has_name = anchors
                    .iter()
                    .chain(embedded.iter())
                    .any(|c| c.tbs_certificate.subject == cur.tbs_certificate.issuer);
                return if has_name {
                    ChainStatus::Invalid
                } else {
                    ChainStatus::Incomplete
                };
            }
        }
    }
    ChainStatus::Invalid
}

/// Comprueba un SignedData sobre `content` (los bytes firmados): resumen y firma del firmante.
/// Devuelve el certificado del firmante y los certificados incluidos.
pub(crate) fn verify_cms(
    sd: &SignedData,
    content: &[u8],
) -> std::result::Result<(Certificate, Vec<Certificate>), String> {
    let si = sd.signer_infos.0.iter().next().ok_or("la firma no contiene firmante")?;
    let certs: Vec<Certificate> = sd
        .certificates
        .iter()
        .flat_map(|s| s.0.iter())
        .filter_map(|c| match c {
            cms::cert::CertificateChoices::Certificate(c) => Some(c.clone()),
            _ => None,
        })
        .collect();
    let leaf = certs
        .iter()
        .find(|c| match &si.sid {
            SignerIdentifier::IssuerAndSerialNumber(i) => {
                c.tbs_certificate.issuer == i.issuer && c.tbs_certificate.serial_number == i.serial_number
            }
            SignerIdentifier::SubjectKeyIdentifier(_) => false,
        })
        .ok_or("la firma no incluye el certificado del firmante")?
        .clone();
    let alg = digest_from_oid(&si.digest_alg.oid).ok_or("algoritmo de resumen no soportado")?;
    let attrs = si.signed_attrs.as_ref().ok_or("la firma no tiene atributos firmados")?;
    let expected = attrs
        .iter()
        .find(|a| a.oid == OID_MESSAGE_DIGEST)
        .and_then(|a| a.values.iter().next())
        .and_then(|v| v.decode_as::<der::asn1::OctetString>().ok())
        .ok_or("falta el resumen firmado")?;
    if expected.as_bytes() != alg.digest(content).as_slice() {
        return Err("el contenido del documento ha sido modificado desde que se firmó".into());
    }
    let signed = attrs.to_der().map_err(|e| e.to_string())?;
    if !verify_signature(
        &leaf.tbs_certificate.subject_public_key_info,
        &si.signature_algorithm.oid,
        alg,
        &signed,
        si.signature.as_bytes(),
    ) {
        return Err("la firma digital no corresponde al certificado (firma rota)".into());
    }
    Ok((leaf, certs))
}

/// Nivel P de la referencia DocMDP de una firma de certificación.
fn docmdp_level(sig: &lopdf::Dictionary) -> Option<u8> {
    let refs = sig.get(b"Reference").ok()?.as_array().ok()?;
    refs.iter().find_map(|r| {
        let d = r.as_dict().ok()?;
        if d.get(b"TransformMethod").ok()? != &Object::Name(b"DocMDP".to_vec()) {
            return None;
        }
        let p = d
            .get(b"TransformParams")
            .ok()?
            .as_dict()
            .ok()
            .and_then(|t| t.get(b"P").ok()?.as_i64().ok())
            .unwrap_or(2);
        Some(p.clamp(1, 3) as u8)
    })
}
