//! Claves públicas de confianza.

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use ed25519_dalek::{Signature as DalekSignature, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};

use crate::error::{Result, UpdateError};

/// Contexto que se antepone a la carga útil antes de firmar: una firma de manifiesto no sirve para otra cosa.
pub const SIGNING_CONTEXT: &[u8] = b"pliego-update-manifest-v1\n";

/// Claves de publicación de producción, en formato `ed25519:<base64>`.
///
/// Claves reales generadas con `pliego-release keygen` (las privadas viven solo en acr12 y en la copia sin conexión).
/// Se recomiendan dos: la activa y una de reserva guardada sin conexión para poder rotar.
/// También se pueden añadir en compilación con la variable `PLIEGO_UPDATE_PUBKEYS` (separadas por comas).
const BUILTIN_RELEASE_KEYS: &[&str] = &[
    // Activa (id ab6fa8bae6005ac0), generada en acr12 el 2026-11-04.
    "ed25519:mWq30UarbH3bZHlR4NEJafc5NVkq+X3bmFdSZn93+XQ=",
    // Reserva (id 9d886dc4f92cdbde): se guarda solo sin conexión; sirve para rotar.
    "ed25519:H8pykJe7e8W2PnOonRlJ8d2zcuTEy/Fj9R3axSp/XJY=",
];

/// Claves de producción compiladas en este binario.
pub fn release_keys() -> Vec<PublicKey> {
    let extra = option_env!("PLIEGO_UPDATE_PUBKEYS").unwrap_or("");
    BUILTIN_RELEASE_KEYS
        .iter()
        .copied()
        .chain(extra.split(',').map(str::trim).filter(|s| !s.is_empty()))
        .filter_map(|s| PublicKey::parse(s).ok())
        .collect()
}

/// Clave pública Ed25519 con su identificador (los 8 primeros bytes de su SHA-256, en hexadecimal).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicKey {
    key: VerifyingKey,
}

impl PublicKey {
    pub fn from_bytes(bytes: &[u8; 32]) -> Result<Self> {
        VerifyingKey::from_bytes(bytes)
            .map(|key| Self { key })
            .map_err(|_| UpdateError::Malformed("clave pública no válida".into()))
    }

    /// Lee `ed25519:<base64>` (o el base64 solo).
    pub fn parse(text: &str) -> Result<Self> {
        let b64 = text.trim().strip_prefix("ed25519:").unwrap_or(text.trim());
        let raw = B64
            .decode(b64)
            .map_err(|_| UpdateError::Malformed("clave pública no válida".into()))?;
        let bytes: [u8; 32] = raw
            .try_into()
            .map_err(|_| UpdateError::Malformed("clave pública de longitud incorrecta".into()))?;
        Self::from_bytes(&bytes)
    }

    pub fn to_text(&self) -> String {
        format!("ed25519:{}", B64.encode(self.key.as_bytes()))
    }

    pub fn key_id(&self) -> String {
        let digest = Sha256::digest(self.key.as_bytes());
        digest[..8].iter().map(|b| format!("{b:02x}")).collect()
    }

    /// Verifica una firma (base64) sobre `SIGNING_CONTEXT || payload`.
    pub fn verify(&self, payload: &[u8], sig_b64: &str) -> bool {
        let Ok(raw) = B64.decode(sig_b64) else { return false };
        let Ok(bytes) = <[u8; 64]>::try_from(raw.as_slice()) else {
            return false;
        };
        let sig = DalekSignature::from_bytes(&bytes);
        let mut msg = Vec::with_capacity(SIGNING_CONTEXT.len() + payload.len());
        msg.extend_from_slice(SIGNING_CONTEXT);
        msg.extend_from_slice(payload);
        self.key.verify(&msg, &sig).is_ok()
    }
}
