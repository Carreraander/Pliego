//! Formato del manifiesto y su sobre firmado.

use std::collections::BTreeMap;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use semver::Version;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::error::{Result, UpdateError};
use crate::keys::PublicKey;

/// Identificador del formato del sobre.
pub const FORMAT: &str = "pliego-update/1";
/// Versión del esquema de la carga útil.
pub const SCHEMA: u32 = 1;

/// Lo que se publica en `v1/<canal>/manifest.json`: la carga útil en base64 (los bytes exactos que se firman,
/// sin canonicalizar JSON) y sus firmas.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    pub format: String,
    pub payload: String,
    pub signatures: Vec<Signature>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signature {
    pub key_id: String,
    pub sig: String,
}

/// Carga útil del manifiesto: la versión más reciente de un canal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub schema: u32,
    pub product: String,
    pub channel: String,
    /// Crece en cada publicación del canal. El cliente rechaza un serial menor que el último visto.
    pub serial: u64,
    /// RFC 3339.
    pub issued_at: String,
    /// RFC 3339. Pasada esta fecha el cliente no confía en el manifiesto (hay que volver a firmarlo).
    pub expires_at: String,
    pub latest: Release,
    /// Las versiones anteriores a esta deben actualizarse (aviso persistente, no se puede posponer indefinidamente).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_supported_version: Option<Version>,
    /// Versiones con fallos graves conocidos: se tratan como obligatorias.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocked_versions: Vec<Version>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Release {
    pub version: Version,
    pub released_at: String,
    /// Notas por idioma (`es`, `en`), en Markdown breve.
    #[serde(default)]
    pub notes: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes_url: Option<String>,
    /// Corrige un fallo de seguridad: se instala aunque el modo sea "solo avisar" (nunca si está desactivado).
    #[serde(default)]
    pub critical: bool,
    /// Porcentaje de instalaciones que la reciben (1–100).
    #[serde(default = "full_rollout")]
    pub rollout_percent: u8,
    /// Compilación mínima de Windows (p. ej. 19041 = Windows 10 2004).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_windows_build: Option<u32>,
    pub artifacts: Vec<Artifact>,
}

fn full_rollout() -> u8 {
    100
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Artifact {
    pub kind: ArtifactKind,
    /// `x86_64` o `aarch64`.
    pub arch: String,
    /// Absoluta (`https://…`) o relativa a la URL base del servidor.
    pub url: String,
    pub size: u64,
    /// SHA-256 en hexadecimal minúsculas.
    pub sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArtifactKind {
    /// Paquete de la aplicación (`.paq`, tar comprimido): lo que usa el actualizador.
    Package,
    /// Instalador completo `Pliego-Setup-x.y.z.exe` (descarga manual, instalaciones nuevas).
    Setup,
    /// MSI para despliegue corporativo (sin autoactualización).
    Msi,
}

impl Manifest {
    pub fn issued(&self) -> Result<OffsetDateTime> {
        parse_time(&self.issued_at)
    }

    pub fn expires(&self) -> Result<OffsetDateTime> {
        parse_time(&self.expires_at)
    }

    /// Serializa la carga útil (los bytes que se firman).
    pub fn to_payload(&self) -> Vec<u8> {
        serde_json::to_vec_pretty(self).expect("el manifiesto siempre se serializa")
    }
}

impl Release {
    pub fn artifact(&self, kind: ArtifactKind, arch: &str) -> Option<&Artifact> {
        self.artifacts.iter().find(|a| a.kind == kind && a.arch == arch)
    }

    /// Notas en `lang`, o en español, o las primeras que haya.
    pub fn notes_for(&self, lang: &str) -> Option<&str> {
        self.notes
            .get(lang)
            .or_else(|| self.notes.get("es"))
            .or_else(|| self.notes.values().next())
            .map(String::as_str)
    }
}

pub(crate) fn parse_time(s: &str) -> Result<OffsetDateTime> {
    OffsetDateTime::parse(s, &Rfc3339).map_err(|_| UpdateError::Malformed(format!("fecha no válida: {s}")))
}

pub(crate) fn format_time(t: OffsetDateTime) -> String {
    t.format(&Rfc3339).expect("RFC 3339 siempre formatea")
}

impl Envelope {
    /// Comprueba que al menos una firma es de una clave de confianza y devuelve la carga útil decodificada.
    /// No comprueba caducidad ni serial: eso lo hace [`crate::evaluate`].
    pub fn open(&self, trusted: &[PublicKey]) -> Result<Manifest> {
        if self.format != FORMAT {
            return Err(UpdateError::Malformed(format!("formato desconocido «{}»", self.format)));
        }
        let payload = B64
            .decode(&self.payload)
            .map_err(|_| UpdateError::Malformed("carga útil no es base64".into()))?;
        let signed = self.signatures.iter().any(|s| {
            trusted
                .iter()
                .any(|k| k.key_id() == s.key_id && k.verify(&payload, &s.sig))
        });
        if !signed {
            return Err(UpdateError::BadSignature);
        }
        let manifest: Manifest = serde_json::from_slice(&payload).map_err(|e| UpdateError::Malformed(e.to_string()))?;
        if manifest.schema != SCHEMA {
            return Err(UpdateError::Malformed(format!(
                "esquema {} no soportado",
                manifest.schema
            )));
        }
        Ok(manifest)
    }

    pub fn from_json(bytes: &[u8]) -> Result<Self> {
        serde_json::from_slice(bytes).map_err(|e| UpdateError::Malformed(e.to_string()))
    }
}
