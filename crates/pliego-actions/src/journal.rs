//! Diario de un lote en `<salida>/.pliego-run.json`, para reanudar tras cancelar o fallar.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use pliego_core::{Error, Result};
use serde::{Deserialize, Serialize};

pub const FILE_NAME: &str = ".pliego-run.json";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Outcome {
    Done { output: PathBuf },
    Failed { error: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub size: u64,
    pub mtime_secs: u64,
    #[serde(flatten)]
    pub outcome: Outcome,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Journal {
    pub action: String,
    pub entries: BTreeMap<String, Entry>,
}

/// Tamaño y fecha de modificación de la entrada: si cambian, el resultado guardado ya no vale.
pub fn stamp_of(path: &Path) -> Result<(u64, u64)> {
    let md = std::fs::metadata(path).map_err(|e| Error::io_at(path, e))?;
    let secs = md
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs());
    Ok((md.len(), secs))
}

impl Journal {
    pub fn load(dir: &Path) -> Option<Journal> {
        let text = std::fs::read_to_string(dir.join(FILE_NAME)).ok()?;
        serde_json::from_str(&text).ok()
    }

    pub fn save(&self, dir: &Path) -> Result<()> {
        let tmp = dir.join(format!("{FILE_NAME}.tmp"));
        let text = serde_json::to_string_pretty(self).map_err(|e| Error::Other(e.to_string()))?;
        std::fs::write(&tmp, text).map_err(|e| Error::io_at(&tmp, e))?;
        std::fs::rename(&tmp, dir.join(FILE_NAME)).map_err(|e| Error::io_at(dir, e))
    }

    /// ¿Ya se procesó este archivo con éxito y no ha cambiado?
    pub fn is_done(&self, rel: &str, size: u64, mtime_secs: u64) -> Option<&Path> {
        match self.entries.get(rel) {
            Some(Entry {
                size: s,
                mtime_secs: m,
                outcome: Outcome::Done { output },
            }) if *s == size && *m == mtime_secs && output.exists() => Some(output),
            _ => None,
        }
    }
}
