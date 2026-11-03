//! Construcción de la firma CMS (CAdES) que va dentro del PDF.

use cms::cert::{CertificateChoices, IssuerAndSerialNumber};
use cms::content_info::{CmsVersion, ContentInfo};
use cms::signed_data::{
    CertificateSet, EncapsulatedContentInfo, SignedAttributes, SignedData, SignerIdentifier, SignerInfo, SignerInfos,
};
use const_oid::ObjectIdentifier;
use der::asn1::{Any, OctetString, SetOfVec};
use der::{Decode, Encode, Sequence};
use pliego_core::{Error, Result};
use spki::AlgorithmIdentifierOwned;
use x509_cert::Certificate;
use x509_cert::attr::Attribute;
use x509_cert::ext::pkix::name::GeneralName;

use crate::signer::{DigestAlg, SignatureScheme, Signer, cert_der};

pub const OID_DATA: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113549.1.7.1");
pub const OID_SIGNED_DATA: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113549.1.7.2");
pub const OID_CONTENT_TYPE: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113549.1.9.3");
pub const OID_MESSAGE_DIGEST: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113549.1.9.4");
pub const OID_SIGNING_CERT_V2: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113549.1.9.16.2.47");
pub const OID_TSTINFO: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113549.1.9.16.1.4");
pub const OID_TIMESTAMP_TOKEN: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113549.1.9.16.2.14");

pub fn digest_oid(alg: DigestAlg) -> ObjectIdentifier {
    match alg {
        DigestAlg::Sha256 => ObjectIdentifier::new_unwrap("2.16.840.1.101.3.4.2.1"),
        DigestAlg::Sha384 => ObjectIdentifier::new_unwrap("2.16.840.1.101.3.4.2.2"),
        DigestAlg::Sha512 => ObjectIdentifier::new_unwrap("2.16.840.1.101.3.4.2.3"),
    }
}

pub fn digest_from_oid(oid: &ObjectIdentifier) -> Option<DigestAlg> {
    [DigestAlg::Sha256, DigestAlg::Sha384, DigestAlg::Sha512]
        .into_iter()
        .find(|a| digest_oid(*a) == *oid)
}

fn alg_id(oid: ObjectIdentifier, null_params: bool) -> AlgorithmIdentifierOwned {
    let parameters = null_params.then(|| Any::from_der(&[0x05, 0x00]).expect("NULL válido"));
    AlgorithmIdentifierOwned { oid, parameters }
}

fn signature_alg(scheme: SignatureScheme, alg: DigestAlg) -> AlgorithmIdentifierOwned {
    match (scheme, alg) {
        (SignatureScheme::RsaPkcs1v15, DigestAlg::Sha256) => alg_id("1.2.840.113549.1.1.11".parse().unwrap(), true),
        (SignatureScheme::RsaPkcs1v15, DigestAlg::Sha384) => alg_id("1.2.840.113549.1.1.12".parse().unwrap(), true),
        (SignatureScheme::RsaPkcs1v15, DigestAlg::Sha512) => alg_id("1.2.840.113549.1.1.13".parse().unwrap(), true),
        (SignatureScheme::Ecdsa, DigestAlg::Sha256) => alg_id("1.2.840.10045.4.3.2".parse().unwrap(), false),
        (SignatureScheme::Ecdsa, DigestAlg::Sha384) => alg_id("1.2.840.10045.4.3.3".parse().unwrap(), false),
        (SignatureScheme::Ecdsa, DigestAlg::Sha512) => alg_id("1.2.840.10045.4.3.4".parse().unwrap(), false),
    }
}

#[derive(Sequence)]
struct IssuerSerial {
    issuer: Vec<GeneralName>,
    serial: x509_cert::serial_number::SerialNumber,
}

#[derive(Sequence)]
struct EssCertIdV2 {
    #[asn1(optional = "true")]
    hash_algorithm: Option<AlgorithmIdentifierOwned>,
    cert_hash: OctetString,
    issuer_serial: IssuerSerial,
}

#[derive(Sequence)]
struct SigningCertificateV2 {
    certs: Vec<EssCertIdV2>,
}

fn der_err(e: der::Error) -> Error {
    Error::Other(format!("error al codificar la firma: {e}"))
}

fn attribute(oid: ObjectIdentifier, value_der: &[u8]) -> Result<Attribute> {
    let mut values = SetOfVec::new();
    values
        .insert(Any::from_der(value_der).map_err(der_err)?)
        .map_err(der_err)?;
    Ok(Attribute { oid, values })
}

/// Atributos firmados de PAdES B-B: tipo de contenido, resumen del documento y certificado firmante (ESS v2).
fn signed_attributes(
    leaf: &Certificate,
    alg: DigestAlg,
    content_type: ObjectIdentifier,
    content_digest: &[u8],
) -> Result<SignedAttributes> {
    let ess = SigningCertificateV2 {
        certs: vec![EssCertIdV2 {
            // SHA-256 es el valor por defecto de ESSCertIDv2 y en DER no se codifica.
            hash_algorithm: (alg != DigestAlg::Sha256).then(|| alg_id(digest_oid(alg), false)),
            cert_hash: OctetString::new(alg.digest(&cert_der(leaf)?)).map_err(der_err)?,
            issuer_serial: IssuerSerial {
                issuer: vec![GeneralName::DirectoryName(leaf.tbs_certificate.issuer.clone())],
                serial: leaf.tbs_certificate.serial_number.clone(),
            },
        }],
    };
    let mut attrs = SetOfVec::new();
    let items = [
        attribute(OID_CONTENT_TYPE, &content_type.to_der().map_err(der_err)?)?,
        attribute(
            OID_MESSAGE_DIGEST,
            &OctetString::new(content_digest)
                .map_err(der_err)?
                .to_der()
                .map_err(der_err)?,
        )?,
        attribute(OID_SIGNING_CERT_V2, &ess.to_der().map_err(der_err)?)?,
    ];
    for a in items {
        attrs.insert(a).map_err(der_err)?;
    }
    Ok(attrs)
}

/// Firma CMS desvinculada (`ETSI.CAdES.detached`) sobre `content_digest`, el resumen de los bytes del PDF.
pub fn build_cms(signer: &dyn Signer, alg: DigestAlg, content_digest: &[u8]) -> Result<Vec<u8>> {
    build_signed_data(signer, alg, OID_DATA, None, content_digest)
}

/// SignedData genérico: contenido desvinculado (`econtent` = `None`) o incrustado (p. ej. TSTInfo de un sello).
pub fn build_signed_data(
    signer: &dyn Signer,
    alg: DigestAlg,
    content_type: ObjectIdentifier,
    econtent: Option<&[u8]>,
    content_digest: &[u8],
) -> Result<Vec<u8>> {
    let chain = signer.certificate_chain();
    let leaf = chain
        .first()
        .ok_or_else(|| Error::InvalidInput("el firmante no tiene certificado".into()))?;
    let attrs = signed_attributes(leaf, alg, content_type, content_digest)?;
    // La firma se calcula sobre la codificación DER del conjunto de atributos (etiqueta SET).
    let to_sign = alg.digest(&attrs.to_der().map_err(der_err)?);
    let signature = signer.sign_digest(alg, &to_sign)?;
    let scheme = signer.signature_scheme();

    let signer_info = SignerInfo {
        version: CmsVersion::V1,
        sid: SignerIdentifier::IssuerAndSerialNumber(IssuerAndSerialNumber {
            issuer: leaf.tbs_certificate.issuer.clone(),
            serial_number: leaf.tbs_certificate.serial_number.clone(),
        }),
        digest_alg: alg_id(digest_oid(alg), false),
        signed_attrs: Some(attrs),
        signature_algorithm: signature_alg(scheme, alg),
        signature: OctetString::new(signature).map_err(der_err)?,
        unsigned_attrs: None,
    };
    let mut certs = SetOfVec::new();
    for c in &chain {
        certs
            .insert(CertificateChoices::Certificate(c.clone()))
            .map_err(der_err)?;
    }
    let mut digest_algs = SetOfVec::new();
    digest_algs.insert(alg_id(digest_oid(alg), false)).map_err(der_err)?;
    let mut signer_infos = SetOfVec::new();
    signer_infos.insert(signer_info).map_err(der_err)?;
    let signed = SignedData {
        version: if content_type == OID_DATA {
            CmsVersion::V1
        } else {
            CmsVersion::V3
        },
        digest_algorithms: digest_algs,
        encap_content_info: EncapsulatedContentInfo {
            econtent_type: content_type,
            econtent: match econtent {
                // El campo del crate ya es el elemento [0] EXPLICIT completo: se construye a mano.
                Some(c) => Some(
                    Any::from_der(&OctetString::new(c).map_err(der_err)?.to_der().map_err(der_err)?)
                        .map_err(der_err)?,
                ),
                None => None,
            },
        },
        certificates: Some(CertificateSet(certs)),
        crls: None,
        signer_infos: SignerInfos(signer_infos),
    };
    let info = ContentInfo {
        content_type: OID_SIGNED_DATA,
        content: Any::from_der(&signed.to_der().map_err(der_err)?).map_err(der_err)?,
    };
    info.to_der().map_err(der_err)
}

/// Añade un atributo no firmado (p. ej. el sello de tiempo) al único firmante de una firma CMS.
pub fn add_unsigned_attribute(cms_der: &[u8], oid: ObjectIdentifier, value_der: &[u8]) -> Result<Vec<u8>> {
    let info = ContentInfo::from_der(cms_der).map_err(der_err)?;
    let mut sd: SignedData = info.content.decode_as().map_err(der_err)?;
    let mut si = sd
        .signer_infos
        .0
        .iter()
        .next()
        .cloned()
        .ok_or_else(|| Error::Other("la firma no tiene firmante".into()))?;
    let mut attrs = si.unsigned_attrs.take().unwrap_or_default();
    attrs.insert(attribute(oid, value_der)?).map_err(der_err)?;
    si.unsigned_attrs = Some(attrs);
    let mut infos = SetOfVec::new();
    infos.insert(si).map_err(der_err)?;
    sd.signer_infos = SignerInfos(infos);
    ContentInfo {
        content_type: OID_SIGNED_DATA,
        content: Any::from_der(&sd.to_der().map_err(der_err)?).map_err(der_err)?,
    }
    .to_der()
    .map_err(der_err)
}

/// Valor de la firma del (primer) firmante de una firma CMS.
pub fn signature_value(cms_der: &[u8]) -> Result<Vec<u8>> {
    let info = ContentInfo::from_der(cms_der).map_err(der_err)?;
    let sd: SignedData = info.content.decode_as().map_err(der_err)?;
    let si = sd
        .signer_infos
        .0
        .iter()
        .next()
        .ok_or_else(|| Error::Other("la firma no tiene firmante".into()))?;
    Ok(si.signature.as_bytes().to_vec())
}
