//! Guía del flujo: quién firma ahora y cómo añadir su firma.

use pliego_core::{Error, Result};
use pliego_sign::{SignOptions, Signer, sign_bytes, subject_name};
use serde::{Deserialize, Serialize};

use crate::audit::located_signatures;
use crate::manifest::{Manifest, SignerSpec};
use crate::package::read_manifest;

/// El siguiente firmante del paquete.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingSigner {
    /// Posición (desde 0).
    pub index: usize,
    /// Total de firmantes del flujo.
    pub total: usize,
    pub spec: SignerSpec,
    /// Campo de firma que se le asigna.
    pub field_name: String,
}

/// Comprueba la coherencia de las firmas existentes y devuelve a quién le toca; `None` si ya firmaron todos.
pub fn next_signer(doc: &[u8]) -> Result<Option<PendingSigner>> {
    let m = read_manifest(doc)?;
    let sigs = located_signatures(doc)?;
    for (pos, s) in sigs.iter().enumerate() {
        if s.field.as_deref() != Some(Manifest::field_name(pos).as_str()) {
            return Err(Error::InvalidInput(format!(
                "la firma {} del documento no corresponde al orden previsto del flujo",
                pos + 1
            )));
        }
        if !s.report.as_ref().is_some_and(|r| r.integrity) {
            return Err(Error::InvalidInput(format!(
                "la firma de «{}» no es válida: el documento se ha modificado",
                m.signers.0[pos.min(m.signers.0.len() - 1)].name
            )));
        }
    }
    let total = m.signers.0.len();
    if sigs.len() > total {
        return Err(Error::InvalidInput(
            "el documento tiene más firmas de las previstas".into(),
        ));
    }
    Ok(m.signers.0.get(sigs.len()).map(|spec| PendingSigner {
        index: sigs.len(),
        total,
        spec: spec.clone(),
        field_name: Manifest::field_name(sigs.len()),
    }))
}

/// Añade la firma del siguiente firmante. `opts.field_name` se fuerza al campo asignado.
/// Devuelve el PDF con la nueva firma y a quién le toca después.
pub fn sign_next(doc: &[u8], signer: &dyn Signer, mut opts: SignOptions) -> Result<(Vec<u8>, Option<PendingSigner>)> {
    let pending = next_signer(doc)?.ok_or_else(|| Error::InvalidInput("ya han firmado todos los firmantes".into()))?;
    if let Some(want) = &pending.spec.certificate_contains {
        let subject = signer.certificate_chain().first().map(subject_name).unwrap_or_default();
        if !subject.to_lowercase().contains(&want.to_lowercase()) {
            return Err(Error::InvalidInput(format!(
                "el certificado ({subject}) no corresponde al firmante previsto «{}»",
                pending.spec.name
            )));
        }
    }
    opts.field_name = Some(pending.field_name.clone());
    if opts.reason.is_none() {
        opts.reason = Some(format!(
            "Firma {} de {}: {}",
            pending.index + 1,
            pending.total,
            pending.spec.name
        ));
    }
    let out = sign_bytes(doc, signer, &opts)?;
    let after = next_signer(&out)?;
    Ok((out, after))
}
