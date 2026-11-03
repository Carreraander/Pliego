//! Registro de auditoría: se reconstruye a partir del manifiesto y de las firmas del propio PDF,
//! de modo que no se puede falsear sin invalidar alguna firma.

use lopdf::Object;
use pliego_core::{Error, Result};
use pliego_sign::{SignatureReport, ValidationOptions, validate_bytes};
use serde::{Deserialize, Serialize};

use crate::manifest::Manifest;
use crate::package::{pdf_err, read_manifest, sha256_hex};

/// Qué ocurrió.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "kebab-case")]
pub enum AuditEvent {
    /// Se preparó el paquete.
    Prepared {
        original_sha256: String,
        signers: Vec<String>,
    },
    /// Se añadió una firma.
    Signed {
        /// Posición del firmante previsto (desde 0); `None` si la firma no pertenece al flujo.
        index: Option<usize>,
        /// Nombre del firmante previsto.
        expected: Option<String>,
        /// Titular del certificado que firmó.
        certificate: String,
        field: Option<String>,
        /// SHA-256 (hex) del documento tal como quedó con esta firma.
        document_sha256: String,
        /// La firma es íntegra.
        integrity: bool,
        /// Firmó en el turno que le correspondía.
        in_order: bool,
    },
}

/// Una línea del registro.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Momento (UTC) o `None` si la firma no declara hora.
    pub time: Option<String>,
    #[serde(flatten)]
    pub event: AuditEvent,
}

/// Firma localizada en el PDF.
pub(crate) struct Located {
    pub field: Option<String>,
    pub revision_sha256: String,
    pub end: usize,
    pub report: Option<SignatureReport>,
}

/// Firmas del documento ordenadas por posición en el archivo, con su validación.
pub(crate) fn located_signatures(data: &[u8]) -> Result<Vec<Located>> {
    let doc = lopdf::Document::load_mem(data).map_err(pdf_err)?;
    let reports = validate_bytes(data, &ValidationOptions::default())?;
    let mut out = Vec::new();
    for (&id, obj) in &doc.objects {
        let Object::Dictionary(d) = obj else { continue };
        if !(d.has(b"ByteRange") && d.has(b"Contents")) {
            continue;
        }
        let br: Vec<usize> = d
            .get(b"ByteRange")
            .and_then(Object::as_array)
            .map_err(pdf_err)?
            .iter()
            .filter_map(|o| o.as_i64().ok().map(|n| n.max(0) as usize))
            .collect();
        let [_, _, start2, len2] = br[..] else {
            return Err(Error::Pdf("ByteRange no válido".into()));
        };
        let end = (start2 + len2).min(data.len());
        // El campo es el widget cuyo /V apunta a este diccionario de firma.
        let field = doc.objects.values().find_map(|o| match o {
            Object::Dictionary(w) if matches!(w.get(b"V"), Ok(Object::Reference(r)) if *r == id) => match w.get(b"T") {
                Ok(Object::String(s, _)) => Some(String::from_utf8_lossy(s).into_owned()),
                _ => None,
            },
            _ => None,
        });
        let report = reports.iter().find(|r| r.field.is_some() && r.field == field).cloned();
        out.push(Located {
            field,
            revision_sha256: sha256_hex(&data[..end]),
            end,
            report,
        });
    }
    out.sort_by_key(|l| l.end);
    Ok(out)
}

/// Registro de auditoría del paquete `doc`, en orden cronológico.
pub fn audit_log(doc: &[u8]) -> Result<Vec<AuditEntry>> {
    let m: Manifest = read_manifest(doc)?;
    let mut log = vec![AuditEntry {
        time: chrono::DateTime::from_timestamp(m.prepared_at, 0).map(|t| t.format("%Y-%m-%d %H:%M:%S UTC").to_string()),
        event: AuditEvent::Prepared {
            original_sha256: m.original_sha256.clone(),
            signers: m.signers.0.iter().map(|s| s.name.clone()).collect(),
        },
    }];
    for (pos, sig) in located_signatures(doc)?.into_iter().enumerate() {
        let index = (0..m.signers.0.len()).find(|&i| sig.field.as_deref() == Some(Manifest::field_name(i).as_str()));
        let r = sig.report.as_ref();
        log.push(AuditEntry {
            time: r.and_then(|r| r.signing_time.clone()),
            event: AuditEvent::Signed {
                index,
                expected: index.map(|i| m.signers.0[i].name.clone()),
                certificate: r.map_or_else(|| "desconocido".into(), |r| r.signer.clone()),
                field: sig.field,
                document_sha256: sig.revision_sha256,
                integrity: r.is_some_and(|r| r.integrity),
                in_order: index == Some(pos),
            },
        });
    }
    Ok(log)
}
