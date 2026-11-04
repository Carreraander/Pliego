//! Firma con el almacén de certificados de Windows (CurrentUser\My) mediante CNG/NCrypt.
//!
//! La clave privada nunca sale del proveedor: puede ser el KSP de software, una tarjeta inteligente (DNIe, FNMT
//! en tarjeta) o un TPM. Si el proveedor necesita PIN, Windows muestra su propio diálogo al firmar.
#![allow(unsafe_code)]

use std::mem::{size_of, zeroed};
use std::ptr::{null, null_mut};

use der::Decode;
use pliego_core::{Error, Result};
use serde::{Deserialize, Serialize};
use windows_sys::Win32::Foundation::{NTE_USER_CANCELLED, SCARD_W_CANCELLED_BY_USER};
use windows_sys::Win32::Security::Cryptography::{
    BCRYPT_PKCS1_PADDING_INFO, BCRYPT_SHA256_ALGORITHM, BCRYPT_SHA384_ALGORITHM, BCRYPT_SHA512_ALGORITHM,
    CERT_CHAIN_CONTEXT, CERT_CHAIN_PARA, CERT_CONTEXT, CERT_FIND_SHA1_HASH, CERT_KEY_PROV_INFO_PROP_ID,
    CERT_SHA1_HASH_PROP_ID, CRYPT_ACQUIRE_ONLY_NCRYPT_KEY_FLAG, CRYPT_INTEGER_BLOB, CertCloseStore,
    CertEnumCertificatesInStore, CertFindCertificateInStore, CertFreeCertificateChain, CertFreeCertificateContext,
    CertGetCertificateChain, CertGetCertificateContextProperty, CertOpenSystemStoreW,
    CryptAcquireCertificatePrivateKey, HCERTSTORE, NCRYPT_KEY_HANDLE, NCRYPT_PAD_PKCS1_FLAG, NCryptFreeObject,
    NCryptSignHash, PKCS_7_ASN_ENCODING, X509_ASN_ENCODING,
};
use x509_cert::Certificate;

use crate::signer::{DigestAlg, SignatureScheme, Signer, name_text, subject_name};

const RSA_ENCRYPTION: der::asn1::ObjectIdentifier = const_oid::db::rfc5912::RSA_ENCRYPTION;
const EC_PUBLIC_KEY: der::asn1::ObjectIdentifier = const_oid::db::rfc5912::ID_EC_PUBLIC_KEY;

/// Certificado del almacén personal del usuario, para mostrar en un selector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowsCertificate {
    /// Huella SHA-1 en hexadecimal (mayúsculas, sin separadores), la que admite [`WindowsStoreSigner::open`].
    pub thumbprint: String,
    /// Titular (CN si existe).
    pub subject: String,
    /// Emisor (CN si existe).
    pub issuer: String,
    /// Fin de validez en RFC 3339 (UTC).
    pub not_after: String,
    /// Si Windows tiene asociada una clave privada (en disco, tarjeta o TPM).
    pub has_private_key: bool,
}

/// Almacén `My` del usuario actual, cerrado al soltarlo.
struct Store(HCERTSTORE);

impl Store {
    fn open_my() -> Result<Self> {
        let name: Vec<u16> = "MY\0".encode_utf16().collect();
        // SAFETY: `name` es una cadena UTF-16 terminada en nulo que vive durante la llamada; el proveedor 0 indica
        // el predeterminado.
        let h = unsafe { CertOpenSystemStoreW(0, name.as_ptr()) };
        if h.is_null() {
            return Err(Error::Other(format!(
                "no se pudo abrir el almacén de certificados de Windows: {}",
                std::io::Error::last_os_error()
            )));
        }
        Ok(Self(h))
    }
}

impl Drop for Store {
    fn drop(&mut self) {
        // SAFETY: el handle se obtuvo de CertOpenSystemStoreW y se cierra una sola vez.
        unsafe { CertCloseStore(self.0, 0) };
    }
}

/// Copia el DER de un contexto de certificado.
///
/// # Safety
/// `ctx` debe apuntar a un `CERT_CONTEXT` válido durante la llamada.
unsafe fn context_der(ctx: *const CERT_CONTEXT) -> Vec<u8> {
    // SAFETY: según el contrato, `ctx` es válido; `pbCertEncoded` apunta a `cbCertEncoded` bytes que pertenecen al
    // contexto y se copian antes de devolver.
    unsafe {
        let c = &*ctx;
        std::slice::from_raw_parts(c.pbCertEncoded, c.cbCertEncoded as usize).to_vec()
    }
}

/// Lee una propiedad binaria de un certificado; `None` si no existe.
///
/// # Safety
/// `ctx` debe apuntar a un `CERT_CONTEXT` válido durante la llamada.
unsafe fn context_property(ctx: *const CERT_CONTEXT, prop: u32) -> Option<Vec<u8>> {
    let mut len = 0u32;
    // SAFETY: primera llamada con búfer nulo solo para conocer el tamaño; `ctx` es válido según el contrato.
    if unsafe { CertGetCertificateContextProperty(ctx, prop, null_mut(), &mut len) } == 0 {
        return None;
    }
    let mut buf = vec![0u8; len as usize];
    // SAFETY: `buf` tiene exactamente `len` bytes escribibles, el tamaño que pidió la llamada anterior.
    if unsafe { CertGetCertificateContextProperty(ctx, prop, buf.as_mut_ptr().cast(), &mut len) } == 0 {
        return None;
    }
    buf.truncate(len as usize);
    Some(buf)
}

fn parse_cert(der: &[u8]) -> Result<Certificate> {
    Certificate::from_der(der).map_err(|e| Error::Other(format!("certificado del almacén ilegible: {e}")))
}

fn time_rfc3339(t: &x509_cert::time::Time) -> String {
    let secs = t.to_unix_duration().as_secs() as i64;
    chrono::DateTime::from_timestamp(secs, 0)
        .map(|d| d.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
        .unwrap_or_default()
}

/// Normaliza una huella SHA-1: admite espacios, dos puntos y minúsculas. Devuelve los 20 bytes.
pub(crate) fn parse_thumbprint(s: &str) -> Result<[u8; 20]> {
    let clean: String = s
        .chars()
        .filter(|c| !c.is_whitespace() && *c != ':' && *c != '\u{200e}')
        .collect();
    let mut out = [0u8; 20];
    if clean.len() != 40 || hex::decode_to_slice(&clean, &mut out).is_err() {
        return Err(Error::InvalidInput(format!(
            "«{s}» no es una huella SHA-1 válida (40 cifras hexadecimales)"
        )));
    }
    Ok(out)
}

/// Lista los certificados del almacén personal del usuario (CurrentUser\My).
pub fn list_windows_certificates() -> Result<Vec<WindowsCertificate>> {
    let store = Store::open_my()?;
    let mut out = Vec::new();
    let mut ctx: *mut CERT_CONTEXT = null_mut();
    loop {
        // SAFETY: el almacén está abierto; `ctx` es nulo o el contexto devuelto por la iteración anterior, que esta
        // llamada libera por sí misma (no se libera a mano dentro del bucle).
        ctx = unsafe { CertEnumCertificatesInStore(store.0, ctx) };
        if ctx.is_null() {
            break;
        }
        // SAFETY: `ctx` no es nulo y sigue vivo hasta la siguiente llamada a CertEnumCertificatesInStore.
        let (der, sha1, key_info) = unsafe {
            (
                context_der(ctx),
                context_property(ctx, CERT_SHA1_HASH_PROP_ID),
                context_property(ctx, CERT_KEY_PROV_INFO_PROP_ID),
            )
        };
        let Ok(cert) = Certificate::from_der(&der) else {
            continue;
        };
        out.push(WindowsCertificate {
            thumbprint: sha1.map(hex::encode_upper).unwrap_or_default(),
            subject: subject_name(&cert),
            issuer: name_text(&cert.tbs_certificate.issuer),
            not_after: time_rfc3339(&cert.tbs_certificate.validity.not_after),
            has_private_key: key_info.is_some(),
        });
    }
    Ok(out)
}

/// Firmante con un certificado del almacén de Windows y su clave en CNG (software, tarjeta o TPM).
pub struct WindowsStoreSigner {
    key: NCRYPT_KEY_HANDLE,
    free_key: bool,
    ctx: *const CERT_CONTEXT,
    _store: Store,
    chain: Vec<Certificate>,
    scheme: SignatureScheme,
}

impl WindowsStoreSigner {
    /// Abre el certificado con huella SHA-1 `thumbprint` de CurrentUser\My y su clave privada.
    pub fn open(thumbprint: &str) -> Result<Self> {
        let hash = parse_thumbprint(thumbprint)?;
        let store = Store::open_my()?;
        let blob = CRYPT_INTEGER_BLOB {
            cbData: hash.len() as u32,
            pbData: hash.as_ptr().cast_mut(),
        };
        // SAFETY: el almacén está abierto y `blob` apunta a `hash`, que vive durante la llamada (solo se lee).
        let ctx = unsafe {
            CertFindCertificateInStore(
                store.0,
                X509_ASN_ENCODING | PKCS_7_ASN_ENCODING,
                0,
                CERT_FIND_SHA1_HASH,
                (&raw const blob).cast(),
                null(),
            )
        };
        if ctx.is_null() {
            return Err(Error::InvalidInput(format!(
                "no hay ningún certificado con huella {} en el almacén personal de Windows",
                hex::encode_upper(hash)
            )));
        }
        // A partir de aquí `signer` es dueño de `ctx` y del almacén: si algo falla, Drop los libera.
        let mut signer = Self {
            key: 0,
            free_key: false,
            ctx,
            _store: store,
            chain: Vec::new(),
            scheme: SignatureScheme::RsaPkcs1v15,
        };

        // SAFETY: `ctx` es un contexto válido devuelto por CertFindCertificateInStore.
        let leaf = parse_cert(&unsafe { context_der(ctx) })?;
        let alg = leaf.tbs_certificate.subject_public_key_info.algorithm.oid;
        signer.scheme = if alg == RSA_ENCRYPTION {
            SignatureScheme::RsaPkcs1v15
        } else if alg == EC_PUBLIC_KEY {
            SignatureScheme::Ecdsa
        } else {
            return Err(Error::Unsupported(
                "el certificado usa un tipo de clave no soportado (se admiten RSA y ECDSA)".into(),
            ));
        };

        let mut key = 0;
        let mut spec = 0;
        let mut free = 0;
        // SAFETY: `ctx` es válido y los punteros de salida apuntan a variables locales. Sin
        // CRYPT_ACQUIRE_SILENT_FLAG, para que las tarjetas puedan pedir el PIN.
        let ok = unsafe {
            CryptAcquireCertificatePrivateKey(
                ctx,
                CRYPT_ACQUIRE_ONLY_NCRYPT_KEY_FLAG,
                null(),
                &mut key,
                &mut spec,
                &mut free,
            )
        };
        if ok == 0 {
            return Err(Error::InvalidInput(format!(
                "no se pudo acceder a la clave privada del certificado «{}»: {}",
                subject_name(&leaf),
                std::io::Error::last_os_error()
            )));
        }
        signer.key = key;
        signer.free_key = free != 0;
        signer.chain = signer.build_chain().unwrap_or_else(|| vec![leaf]);
        Ok(signer)
    }

    /// Cadena desde el certificado del firmante con el motor de Windows; `None` si no se puede construir.
    fn build_chain(&self) -> Option<Vec<Certificate>> {
        // SAFETY: estructura de datos planos; cero es válido y se rellena `cbSize`.
        let mut para: CERT_CHAIN_PARA = unsafe { zeroed() };
        para.cbSize = size_of::<CERT_CHAIN_PARA>() as u32;
        let mut chain: *mut CERT_CHAIN_CONTEXT = null_mut();
        // SAFETY: motor predeterminado (nulo), `self.ctx` válido, sin almacén adicional y `chain` como salida.
        let ok =
            unsafe { CertGetCertificateChain(null_mut(), self.ctx, null(), null_mut(), &para, 0, null(), &mut chain) };
        if ok == 0 || chain.is_null() {
            return None;
        }
        let mut certs = Vec::new();
        // SAFETY: `chain` lo devolvió CertGetCertificateChain y sigue vivo hasta CertFreeCertificateChain; se leen
        // `cChain` cadenas simples y `cElement` elementos, cada uno con un contexto de certificado válido.
        unsafe {
            let c = &*chain;
            if c.cChain > 0 {
                let simple = &**c.rgpChain;
                for i in 0..simple.cElement as usize {
                    let elem = &**simple.rgpElement.add(i);
                    if let Ok(cert) = Certificate::from_der(&context_der(elem.pCertContext)) {
                        certs.push(cert);
                    }
                }
            }
            CertFreeCertificateChain(chain);
        }
        (!certs.is_empty()).then_some(certs)
    }
}

impl Drop for WindowsStoreSigner {
    fn drop(&mut self) {
        // SAFETY: la clave solo se libera si CryptAcquireCertificatePrivateKey indicó que es nuestra; el contexto
        // se obtuvo de CertFindCertificateInStore y se libera una vez, antes de cerrar el almacén (campo `_store`).
        unsafe {
            if self.free_key && self.key != 0 {
                NCryptFreeObject(self.key);
            }
            CertFreeCertificateContext(self.ctx);
        }
    }
}

impl Signer for WindowsStoreSigner {
    fn certificate_chain(&self) -> Vec<Certificate> {
        self.chain.clone()
    }

    fn signature_scheme(&self) -> SignatureScheme {
        self.scheme
    }

    fn sign_digest(&self, alg: DigestAlg, digest: &[u8]) -> Result<Vec<u8>> {
        let padding = BCRYPT_PKCS1_PADDING_INFO {
            pszAlgId: match alg {
                DigestAlg::Sha256 => BCRYPT_SHA256_ALGORITHM,
                DigestAlg::Sha384 => BCRYPT_SHA384_ALGORITHM,
                DigestAlg::Sha512 => BCRYPT_SHA512_ALGORITHM,
            },
        };
        // RSA: CNG añade el DigestInfo del algoritmo indicado. ECDSA: sin relleno, firma r||s.
        let (pad_ptr, flags) = match self.scheme {
            SignatureScheme::RsaPkcs1v15 => ((&raw const padding).cast(), NCRYPT_PAD_PKCS1_FLAG),
            SignatureScheme::Ecdsa => (null(), 0),
        };
        let sign = |out: *mut u8, cap: u32, len: &mut u32| {
            // SAFETY: `self.key` es una clave NCrypt abierta; `pad_ptr` es nulo o apunta a `padding`, vivo durante la
            // llamada; `digest` se lee entero; `out` es nulo (consulta de tamaño) o un búfer de `cap` bytes.
            unsafe {
                NCryptSignHash(
                    self.key,
                    pad_ptr,
                    digest.as_ptr(),
                    digest.len() as u32,
                    out,
                    cap,
                    len,
                    flags,
                )
            }
        };
        let mut len = 0u32;
        let hr = sign(null_mut(), 0, &mut len);
        if hr < 0 {
            return Err(sign_error(hr));
        }
        let mut sig = vec![0u8; len as usize];
        let hr = sign(sig.as_mut_ptr(), len, &mut len);
        if hr < 0 {
            return Err(sign_error(hr));
        }
        sig.truncate(len as usize);
        match self.scheme {
            SignatureScheme::RsaPkcs1v15 => Ok(sig),
            SignatureScheme::Ecdsa => ecdsa_raw_to_der(&sig),
        }
    }
}

fn sign_error(hr: i32) -> Error {
    if hr == NTE_USER_CANCELLED || hr == SCARD_W_CANCELLED_BY_USER {
        return Error::Cancelled;
    }
    Error::Other(format!(
        "Windows no pudo firmar con la clave del certificado (código 0x{:08X})",
        hr as u32
    ))
}

/// CNG devuelve ECDSA como `r||s` de tamaño fijo; CMS espera la estructura DER `ECDSA-Sig-Value`.
fn ecdsa_raw_to_der(raw: &[u8]) -> Result<Vec<u8>> {
    let bad = |e: &dyn std::fmt::Display| Error::Other(format!("firma ECDSA no válida devuelta por Windows: {e}"));
    match raw.len() {
        64 => Ok(p256::ecdsa::Signature::from_slice(raw)
            .map_err(|e| bad(&e))?
            .to_der()
            .to_bytes()
            .to_vec()),
        96 => Ok(p384::ecdsa::Signature::from_slice(raw)
            .map_err(|e| bad(&e))?
            .to_der()
            .to_bytes()
            .to_vec()),
        n => Err(Error::Unsupported(format!(
            "curva ECDSA no soportada (firma de {n} bytes; se admiten P-256 y P-384)"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thumbprint_accepts_common_formats() {
        let t = "a1 b2 c3 d4 e5 f6 07 18 29 3a 4b 5c 6d 7e 8f 90 01 12 23 34";
        let b = parse_thumbprint(t).unwrap();
        assert_eq!(b[0], 0xA1);
        assert_eq!(parse_thumbprint(&t.replace(' ', ":")).unwrap(), b);
        assert_eq!(parse_thumbprint(&t.replace(' ', "").to_uppercase()).unwrap(), b);
    }

    #[test]
    fn thumbprint_rejects_bad_input() {
        assert!(parse_thumbprint("abc").is_err());
        assert!(parse_thumbprint(&"zz".repeat(20)).is_err());
    }

    #[test]
    fn unknown_thumbprint_is_reported() {
        let err = WindowsStoreSigner::open(&"00".repeat(20)).err().unwrap();
        assert!(matches!(err, Error::InvalidInput(_)), "{err}");
    }

    #[test]
    fn listing_does_not_fail() {
        let certs = list_windows_certificates().unwrap();
        assert!(
            certs
                .iter()
                .all(|c| c.thumbprint.is_empty() || c.thumbprint.len() == 40)
        );
    }

    #[test]
    fn ecdsa_raw_signature_becomes_der() {
        let mut raw = [0u8; 64];
        raw[31] = 1;
        raw[63] = 2;
        let der = ecdsa_raw_to_der(&raw).unwrap();
        assert_eq!(der[0], 0x30);
        assert!(ecdsa_raw_to_der(&[1u8; 10]).is_err());
    }
}
