//! Firma de manifiestos (solo con la función `sign`; la usa `pliego-release`, nunca la app).

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use ed25519_dalek::{Signer, SigningKey};

use crate::error::{Result, UpdateError};
use crate::keys::{PublicKey, SIGNING_CONTEXT};
use crate::manifest::{Envelope, FORMAT, Manifest, Signature};

/// Cabecera del archivo de clave secreta, para reconocerlo y no confundirlo con la pública.
const SECRET_HEADER: &str = "pliego-update-secret-key-v1";

/// Clave de firma de publicaciones.
pub struct ReleaseKey {
    key: SigningKey,
}

impl ReleaseKey {
    /// Genera una clave nueva con el generador aleatorio del sistema.
    pub fn generate() -> Result<Self> {
        let mut seed = [0u8; 32];
        getrandom::fill(&mut seed).map_err(|e| UpdateError::Io(std::io::Error::other(e.to_string())))?;
        Ok(Self::from_seed(&seed))
    }

    pub fn from_seed(seed: &[u8; 32]) -> Self {
        Self {
            key: SigningKey::from_bytes(seed),
        }
    }

    pub fn public(&self) -> PublicKey {
        PublicKey::from_bytes(self.key.verifying_key().as_bytes()).expect("la clave derivada siempre es válida")
    }

    /// Texto del archivo de clave secreta.
    pub fn to_secret_text(&self) -> String {
        format!("{SECRET_HEADER}\n{}\n", B64.encode(self.key.to_bytes()))
    }

    pub fn from_secret_text(text: &str) -> Result<Self> {
        let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
        if lines.next() != Some(SECRET_HEADER) {
            return Err(UpdateError::Malformed(
                "no es un archivo de clave secreta de Pliego".into(),
            ));
        }
        let raw = lines
            .next()
            .and_then(|l| B64.decode(l).ok())
            .ok_or_else(|| UpdateError::Malformed("clave secreta no válida".into()))?;
        let seed: [u8; 32] = raw
            .try_into()
            .map_err(|_| UpdateError::Malformed("clave secreta de longitud incorrecta".into()))?;
        Ok(Self::from_seed(&seed))
    }

    pub fn sign_payload(&self, payload: &[u8]) -> Signature {
        let mut msg = Vec::with_capacity(SIGNING_CONTEXT.len() + payload.len());
        msg.extend_from_slice(SIGNING_CONTEXT);
        msg.extend_from_slice(payload);
        Signature {
            key_id: self.public().key_id(),
            sig: B64.encode(self.key.sign(&msg).to_bytes()),
        }
    }
}

/// Firma un manifiesto con una o varias claves (p. ej. durante una rotación, la saliente y la entrante).
pub fn seal(manifest: &Manifest, keys: &[&ReleaseKey]) -> Envelope {
    let payload = manifest.to_payload();
    Envelope {
        format: FORMAT.to_owned(),
        payload: B64.encode(&payload),
        signatures: keys.iter().map(|k| k.sign_payload(&payload)).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_text_roundtrip_and_bad_header() {
        let k = ReleaseKey::from_seed(&[3; 32]);
        let back = ReleaseKey::from_secret_text(&k.to_secret_text()).unwrap();
        assert_eq!(k.public(), back.public());
        assert!(ReleaseKey::from_secret_text(&k.public().to_text()).is_err());
        assert_eq!(PublicKey::parse(&k.public().to_text()).unwrap(), k.public());
    }
}
