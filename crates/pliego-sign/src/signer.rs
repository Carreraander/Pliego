//! Abstracción de claves de firma: PKCS#12 ahora; almacén de Windows y tarjetas más adelante.

use der::{Decode, DecodePem, Encode};
use pkcs8::DecodePrivateKey;
use pliego_core::{Error, Result};
use rsa::Pkcs1v15Sign;
use serde::{Deserialize, Serialize};
use signature::hazmat::PrehashSigner;
use x509_cert::Certificate;

/// Algoritmo de resumen usado en la firma.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum DigestAlg {
    #[default]
    Sha256,
    Sha384,
    Sha512,
}

impl DigestAlg {
    pub fn digest(self, data: &[u8]) -> Vec<u8> {
        use sha2::{Digest, Sha256, Sha384, Sha512};
        match self {
            DigestAlg::Sha256 => Sha256::digest(data).to_vec(),
            DigestAlg::Sha384 => Sha384::digest(data).to_vec(),
            DigestAlg::Sha512 => Sha512::digest(data).to_vec(),
        }
    }
}

/// Esquema de firma de la clave privada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureScheme {
    /// RSASSA-PKCS1-v1_5.
    RsaPkcs1v15,
    /// ECDSA con la firma codificada en DER.
    Ecdsa,
}

/// Quien sabe firmar un resumen. La clave privada nunca sale del implementador.
pub trait Signer {
    /// Cadena de certificados: primero el del firmante, después los intermedios.
    fn certificate_chain(&self) -> Vec<Certificate>;
    /// Esquema de firma de la clave (necesario para declarar el algoritmo en la firma CMS).
    fn signature_scheme(&self) -> SignatureScheme;
    /// Firma un resumen ya calculado con `alg`.
    fn sign_digest(&self, alg: DigestAlg, digest: &[u8]) -> Result<Vec<u8>>;
}

enum Key {
    Rsa(rsa::RsaPrivateKey),
    P256(p256::ecdsa::SigningKey),
    P384(p384::ecdsa::SigningKey),
}

/// Firmante con la clave de un archivo PKCS#12 (.pfx/.p12).
pub struct Pkcs12Signer {
    key: Key,
    chain: Vec<Certificate>,
}

impl Pkcs12Signer {
    pub fn from_bytes(data: &[u8], password: &str) -> Result<Self> {
        use p12_keystore::{KeyStore, Pkcs12ImportPolicy};
        let store = KeyStore::from_pkcs12(data, password, Pkcs12ImportPolicy::Relaxed).map_err(|_| {
            Error::InvalidInput("no se pudo abrir el archivo de certificado: contraseña o formato no válidos".into())
        })?;
        let (_, chain) = store
            .private_key_chain()
            .ok_or_else(|| Error::InvalidInput("el archivo no contiene una clave privada con certificado".into()))?;
        let certs = chain
            .certs()
            .iter()
            .map(|c| Certificate::from_der(c.as_der()).map_err(|e| Error::Other(format!("certificado ilegible: {e}"))))
            .collect::<Result<Vec<_>>>()?;
        if certs.is_empty() {
            return Err(Error::InvalidInput("el archivo no contiene certificados".into()));
        }
        let der = chain.key().as_der();
        let key = if let Ok(k) = rsa::RsaPrivateKey::from_pkcs8_der(der) {
            Key::Rsa(k)
        } else if let Ok(k) = p256::ecdsa::SigningKey::from_pkcs8_der(der) {
            Key::P256(k)
        } else if let Ok(k) = p384::ecdsa::SigningKey::from_pkcs8_der(der) {
            Key::P384(k)
        } else {
            return Err(Error::Unsupported(
                "tipo de clave privada no soportado (se admiten RSA, P-256 y P-384)".into(),
            ));
        };
        Ok(Self { key, chain: certs })
    }

    pub fn from_file(path: impl AsRef<std::path::Path>, password: &str) -> Result<Self> {
        let path = path.as_ref();
        let data = std::fs::read(path).map_err(|e| Error::io_at(path, e))?;
        Self::from_bytes(&data, password)
    }
}

impl Signer for Pkcs12Signer {
    fn certificate_chain(&self) -> Vec<Certificate> {
        self.chain.clone()
    }

    fn signature_scheme(&self) -> SignatureScheme {
        match self.key {
            Key::Rsa(_) => SignatureScheme::RsaPkcs1v15,
            _ => SignatureScheme::Ecdsa,
        }
    }

    fn sign_digest(&self, alg: DigestAlg, digest: &[u8]) -> Result<Vec<u8>> {
        let fail = |e: &dyn std::fmt::Display| Error::Other(format!("falló la firma: {e}"));
        match &self.key {
            Key::Rsa(k) => {
                let scheme = match alg {
                    DigestAlg::Sha256 => Pkcs1v15Sign::new::<sha2::Sha256>(),
                    DigestAlg::Sha384 => Pkcs1v15Sign::new::<sha2::Sha384>(),
                    DigestAlg::Sha512 => Pkcs1v15Sign::new::<sha2::Sha512>(),
                };
                k.sign(scheme, digest).map_err(|e| fail(&e))
            }
            Key::P256(k) => {
                let s: p256::ecdsa::Signature = k.sign_prehash(digest).map_err(|e| fail(&e))?;
                Ok(s.to_der().to_bytes().to_vec())
            }
            Key::P384(k) => {
                let s: p384::ecdsa::Signature = k.sign_prehash(digest).map_err(|e| fail(&e))?;
                Ok(s.to_der().to_bytes().to_vec())
            }
        }
    }
}

/// Nombre legible del titular de un certificado (CN si existe, si no el sujeto completo).
pub fn subject_name(cert: &Certificate) -> String {
    use const_oid::db::rfc4519::CN;
    for rdn in cert.tbs_certificate.subject.0.iter() {
        for atv in rdn.0.iter() {
            if atv.oid == CN {
                if let Ok(s) = atv.value.decode_as::<der::asn1::Utf8StringRef>() {
                    return s.as_str().to_string();
                }
                if let Ok(s) = atv.value.decode_as::<der::asn1::PrintableStringRef>() {
                    return s.as_str().to_string();
                }
            }
        }
    }
    cert.tbs_certificate.subject.to_string()
}

pub(crate) fn cert_der(cert: &Certificate) -> Result<Vec<u8>> {
    cert.to_der()
        .map_err(|e| Error::Other(format!("certificado no codificable: {e}")))
}

/// Lee un certificado de un archivo, en PEM o DER.
pub fn load_certificate(path: impl AsRef<std::path::Path>) -> Result<Certificate> {
    let path = path.as_ref();
    let data = std::fs::read(path).map_err(|e| Error::io_at(path, e))?;
    let parsed = if data.starts_with(b"-----BEGIN") {
        Certificate::from_pem(&data)
    } else {
        Certificate::from_der(&data)
    };
    parsed.map_err(|e| Error::InvalidInput(format!("{} no es un certificado válido: {e}", path.display())))
}
