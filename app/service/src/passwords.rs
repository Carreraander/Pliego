//! Comprobar si un PDF pide contraseña y si la que se tiene vale (sin abrirlo en el visor).
//!
//! Lo usan los paneles que reciben **otros** archivos además del abierto (Combinar, Comparar, Insertar páginas de
//! otro PDF): antes de lanzar el trabajo piden la contraseña de cada archivo protegido en vez de fallar al final.

use std::path::Path;

use pliego_core::{Error, Result};
use pliego_pdf::Document;
use serde::Serialize;

/// Estado de un PDF respecto a su contraseña.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PasswordStatus {
    /// Se abre (no está protegido, o la contraseña dada es correcta).
    Open,
    /// Está protegido y no se dio contraseña.
    Required,
    /// Está protegido y la contraseña dada no es correcta.
    Wrong,
}

/// Estado de `path` con `password` (si hay).
pub fn status(path: &Path, password: Option<&str>) -> Result<PasswordStatus> {
    let bytes = std::fs::read(path).map_err(|e| Error::io_at(path, e))?;
    let mut doc = Document::from_bytes(&bytes, "application/pdf")?;
    if !doc.needs_password()? {
        return Ok(PasswordStatus::Open);
    }
    match password.filter(|p| !p.is_empty()) {
        None => Ok(PasswordStatus::Required),
        Some(p) if doc.authenticate(p)? => Ok(PasswordStatus::Open),
        Some(_) => Ok(PasswordStatus::Wrong),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protect::{self, ProtectRequest};
    use pliego_core::JobContext;

    #[test]
    fn tells_open_required_and_wrong() {
        let dir = tempfile::tempdir().unwrap();
        let plain = dir.path().join("a.pdf");
        std::fs::copy(
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/corpus/organize/a.pdf"),
            &plain,
        )
        .unwrap();
        assert_eq!(status(&plain, None).unwrap(), PasswordStatus::Open);

        let req: ProtectRequest = serde_json::from_value(serde_json::json!({
            "path": plain, "userPassword": "abre", "ownerPassword": "manda"
        }))
        .unwrap();
        let enc = protect::protect(&req, &JobContext::silent()).unwrap().outputs.remove(0);
        assert_eq!(status(&enc, None).unwrap(), PasswordStatus::Required);
        assert_eq!(status(&enc, Some("")).unwrap(), PasswordStatus::Required);
        assert_eq!(status(&enc, Some("mal")).unwrap(), PasswordStatus::Wrong);
        assert_eq!(status(&enc, Some("abre")).unwrap(), PasswordStatus::Open);
        assert!(status(&dir.path().join("no-existe.pdf"), None).is_err());
    }
}
