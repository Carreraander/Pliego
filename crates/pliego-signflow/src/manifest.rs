use pliego_core::{Error, Result};
use serde::{Deserialize, Serialize};

/// Nombre del adjunto que lleva el manifiesto dentro del PDF.
pub const ATTACHMENT_NAME: &str = "pliego-signflow.json";

/// Un firmante previsto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignerSpec {
    /// Nombre para mostrar.
    pub name: String,
    /// Correo, solo informativo (para enviarle el paquete).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// Texto que debe aparecer en el sujeto de su certificado; si se indica, se rechaza otro certificado.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub certificate_contains: Option<String>,
}

impl SignerSpec {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            email: None,
            certificate_contains: None,
        }
    }
}

/// Firmantes en el orden en que deben firmar.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Signers(pub Vec<SignerSpec>);

impl Signers {
    pub fn new(list: Vec<SignerSpec>) -> Self {
        Self(list)
    }

    pub(crate) fn validate(&self) -> Result<()> {
        if self.0.is_empty() {
            return Err(Error::InvalidInput("hace falta al menos un firmante".into()));
        }
        if self.0.iter().any(|s| s.name.trim().is_empty()) {
            return Err(Error::InvalidInput("todos los firmantes necesitan un nombre".into()));
        }
        Ok(())
    }
}

/// Contenido del adjunto del paquete.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub version: u32,
    /// SHA-256 (hex) del PDF original, antes de preparar el paquete.
    pub original_sha256: String,
    /// Momento de la preparación (segundos Unix).
    pub prepared_at: i64,
    pub signers: Signers,
}

impl Manifest {
    /// Nombre del campo de firma asignado al firmante `index` (desde 0).
    pub fn field_name(index: usize) -> String {
        format!("pliego-firma-{}", index + 1)
    }
}
