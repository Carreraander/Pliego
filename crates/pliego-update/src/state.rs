//! Estado persistente del actualizador (`%LOCALAPPDATA%\Pliego\update-state.json`).

use std::collections::BTreeMap;
use std::path::Path;

use semver::Version;
use serde::{Deserialize, Serialize};

use crate::error::Result;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateState {
    /// Identificador aleatorio de esta instalación. Solo se usa en local para el despliegue gradual: no se envía.
    pub install_id: String,
    /// Último serial aceptado por canal (protección contra manifiestos reproducidos).
    #[serde(default)]
    pub last_serial: BTreeMap<String, u64>,
    /// Última comprobación correcta (RFC 3339).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_check: Option<String>,
    /// Versión que el usuario decidió omitir (no se vuelve a avisar de ella; sí de las siguientes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skipped_version: Option<Version>,
}

impl UpdateState {
    /// Estado nuevo con un identificador de instalación aleatorio (`seed`: 16 bytes aleatorios del llamador).
    pub fn new(seed: [u8; 16]) -> Self {
        Self {
            install_id: seed.iter().map(|b| format!("{b:02x}")).collect(),
            ..Self::default()
        }
    }

    pub fn load(path: &Path) -> Result<Option<Self>> {
        match std::fs::read(path) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes).ok()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Guarda de forma atómica (temporal + renombrar).
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(
            &tmp,
            serde_json::to_vec_pretty(self).expect("el estado siempre se serializa"),
        )?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }
}
