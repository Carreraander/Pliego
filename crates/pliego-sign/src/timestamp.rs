//! Sellos de tiempo RFC 3161 para PAdES B-T.

use cms::content_info::ContentInfo;
use cms::signed_data::SignedData;
use const_oid::ObjectIdentifier;
use der::asn1::{Any, GeneralizedTime, OctetString};
use der::{Decode, Encode, Sequence, SliceReader};
use pliego_core::{Error, Result};
use serde::{Deserialize, Serialize};
use spki::AlgorithmIdentifierOwned;
use x509_cert::Certificate;

use crate::cms::{OID_TSTINFO, build_signed_data, digest_oid};
use crate::signer::{DigestAlg, Signer};
use crate::validate::{ChainStatus, check_chain, verify_cms};

/// Autoridad de sellado de tiempo: recibe una `TimeStampReq` DER y devuelve la `TimeStampResp` DER.
pub trait TimestampAuthority {
    fn timestamp(&self, request: &[u8]) -> Result<Vec<u8>>;
}

/// Cliente HTTP (RFC 3161 sobre HTTP). Los servicios https necesitan una versión con TLS.
pub struct HttpTsa {
    pub url: String,
}

impl TimestampAuthority for HttpTsa {
    fn timestamp(&self, request: &[u8]) -> Result<Vec<u8>> {
        let fail = |e: &dyn std::fmt::Display| {
            Error::Other(format!(
                "no se pudo contactar con el servidor de sellado de tiempo: {e}"
            ))
        };
        if self.url.starts_with("https://") {
            return Err(Error::Unsupported(
                "los servidores de sellado de tiempo https todavía no están soportados; use una dirección http".into(),
            ));
        }
        let mut resp = ureq::post(&self.url)
            .header("Content-Type", "application/timestamp-query")
            .send(request)
            .map_err(|e| fail(&e))?;
        resp.body_mut()
            .with_config()
            .limit(4 << 20)
            .read_to_vec()
            .map_err(|e| fail(&e))
    }
}

#[derive(Sequence)]
struct MessageImprint {
    hash_algorithm: AlgorithmIdentifierOwned,
    hashed_message: OctetString,
}

#[derive(Sequence)]
struct TimeStampReq {
    version: u8,
    message_imprint: MessageImprint,
    #[asn1(optional = "true")]
    nonce: Option<u64>,
    cert_req: bool,
}

#[derive(Sequence)]
struct TstInfo {
    version: u8,
    policy: ObjectIdentifier,
    message_imprint: MessageImprint,
    serial_number: u64,
    gen_time: GeneralizedTime,
    #[asn1(optional = "true")]
    nonce: Option<u64>,
}

#[derive(Sequence)]
struct StatusInfo {
    status: u8,
}

#[derive(Sequence)]
struct TimeStampRespOut {
    status: StatusInfo,
    token: ContentInfo,
}

fn der_err(e: der::Error) -> Error {
    Error::Other(format!("sello de tiempo no válido: {e}"))
}

fn imprint(alg: DigestAlg, data: &[u8]) -> Result<MessageImprint> {
    Ok(MessageImprint {
        hash_algorithm: AlgorithmIdentifierOwned {
            oid: digest_oid(alg),
            parameters: None,
        },
        hashed_message: OctetString::new(alg.digest(data)).map_err(der_err)?,
    })
}

/// Solicitud de sello sobre `data` (el valor de la firma), con el certificado de la TSA incluido en la respuesta.
pub fn build_request(alg: DigestAlg, data: &[u8], nonce: u64) -> Result<Vec<u8>> {
    TimeStampReq {
        version: 1,
        message_imprint: imprint(alg, data)?,
        nonce: Some(nonce),
        cert_req: true,
    }
    .to_der()
    .map_err(der_err)
}

/// Extrae el token (ContentInfo DER) de una `TimeStampResp`, comprobando que la TSA lo concedió.
pub fn parse_response(resp: &[u8]) -> Result<Vec<u8>> {
    #[derive(Sequence)]
    struct Resp {
        status: Any,
        token: Option<ContentInfo>,
    }
    let r = Resp::from_der(resp)
        .map_err(|_| Error::Other("respuesta del servidor de sellado de tiempo ilegible".into()))?;
    let mut reader = SliceReader::new(r.status.value()).map_err(der_err)?;
    let status = u32::decode(&mut reader).map_err(der_err)?;
    if status > 1 {
        return Err(Error::Other(format!(
            "el servidor de sellado de tiempo rechazó la solicitud (estado {status})"
        )));
    }
    r.token
        .ok_or_else(|| Error::Other("la respuesta no incluye el sello de tiempo".into()))?
        .to_der()
        .map_err(der_err)
}

/// Obtiene el token para la firma `signature` desde `tsa` y comprueba que corresponde a ella.
pub fn request_token(tsa: &dyn TimestampAuthority, alg: DigestAlg, signature: &[u8]) -> Result<Vec<u8>> {
    let nonce = u64::from_be_bytes(
        alg.digest(&chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0).to_be_bytes())[..8]
            .try_into()
            .expect("8 bytes"),
    ) >> 1;
    let resp = tsa.timestamp(&build_request(alg, signature, nonce)?)?;
    let token = parse_response(&resp)?;
    let info = check_token(&token, signature, &[])?;
    if !info.valid {
        return Err(Error::Other(
            "el sello de tiempo recibido no es válido para esta firma".into(),
        ));
    }
    Ok(token)
}

/// Resultado de comprobar un sello de tiempo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimestampInfo {
    pub time: String,
    /// Corresponde a la firma y su propia firma es correcta.
    pub valid: bool,
    pub signer: String,
    pub chain: ChainStatus,
}

/// Valida un token: huella de `signature`, firma de la TSA y cadena de su certificado.
pub fn check_token(token: &[u8], signature: &[u8], anchors: &[Certificate]) -> Result<TimestampInfo> {
    let info = ContentInfo::from_der(token).map_err(der_err)?;
    let sd: SignedData = info.content.decode_as().map_err(der_err)?;
    let tst_der = sd
        .encap_content_info
        .econtent
        .as_ref()
        .and_then(|c| c.decode_as::<OctetString>().ok())
        .ok_or_else(|| Error::Other("el sello no contiene TSTInfo".into()))?;
    let tst = TstInfo::from_der(tst_der.as_bytes()).map_err(der_err)?;
    let alg = crate::cms::digest_from_oid(&tst.message_imprint.hash_algorithm.oid)
        .ok_or_else(|| Error::Unsupported("algoritmo de resumen del sello no soportado".into()))?;
    let matches = tst.message_imprint.hashed_message.as_bytes() == alg.digest(signature).as_slice();
    let (leaf, certs) = verify_cms(&sd, tst_der.as_bytes()).map_err(Error::Other)?;
    let secs = tst.gen_time.to_unix_duration().as_secs() as i64;
    let time = chrono::DateTime::from_timestamp(secs, 0)
        .unwrap_or_default()
        .format("%Y-%m-%d %H:%M:%S UTC")
        .to_string();
    Ok(TimestampInfo {
        time,
        valid: matches,
        signer: crate::signer::subject_name(&leaf),
        chain: check_chain(&leaf, &certs, anchors),
    })
}

/// Autoridad local que sella con una clave propia. Solo para pruebas y uso sin conexión: su sello
/// no tiene valor legal.
pub struct LocalTsa<S: Signer> {
    pub signer: S,
    pub digest: DigestAlg,
}

impl<S: Signer> TimestampAuthority for LocalTsa<S> {
    fn timestamp(&self, request: &[u8]) -> Result<Vec<u8>> {
        let req = TimeStampReq::from_der(request).map_err(der_err)?;
        let now = std::time::Duration::from_secs(chrono::Utc::now().timestamp() as u64);
        let tst = TstInfo {
            version: 1,
            policy: ObjectIdentifier::new_unwrap("1.2.3.4.1"),
            message_imprint: req.message_imprint,
            serial_number: now.as_nanos() as u64 >> 1,
            gen_time: GeneralizedTime::from_unix_duration(now).map_err(der_err)?,
            nonce: req.nonce,
        }
        .to_der()
        .map_err(der_err)?;
        let token = build_signed_data(
            &self.signer,
            self.digest,
            OID_TSTINFO,
            Some(&tst),
            &self.digest.digest(&tst),
        )?;
        TimeStampRespOut {
            status: StatusInfo { status: 0 },
            token: ContentInfo::from_der(&token).map_err(der_err)?,
        }
        .to_der()
        .map_err(der_err)
    }
}

/// Certificados incluidos en un token de sello de tiempo (para el DSS).
pub fn token_certificates(token: &[u8]) -> Result<Vec<Certificate>> {
    let info = ContentInfo::from_der(token).map_err(der_err)?;
    let sd: SignedData = info.content.decode_as().map_err(der_err)?;
    Ok(sd
        .certificates
        .iter()
        .flat_map(|s| s.0.iter())
        .filter_map(|c| match c {
            cms::cert::CertificateChoices::Certificate(c) => Some(c.clone()),
            _ => None,
        })
        .collect())
}
