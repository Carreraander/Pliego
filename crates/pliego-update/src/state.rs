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
    /// Versión ya descargada, verificada y preparada para activarse en el próximo arranque.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending: Option<PendingUpdate>,
    /// Versiones instaladas en este equipo (más reciente al final), para Ajustes > Actualizaciones.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub history: Vec<HistoryEntry>,
    /// Aviso pendiente de mostrar: una versión no arrancó y se volvió a la anterior.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rollback_notice: Option<RollbackNotice>,
    /// Versión cuyas novedades se mostrarán una vez en Inicio.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub whats_new: Option<Version>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PendingUpdate {
    pub version: Version,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(default)]
    pub mandatory: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub version: Version,
    /// Momento en que se vio por primera vez en marcha (RFC 3339).
    pub at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RollbackNotice {
    /// Versión que no arrancó (no se volverá a instalar).
    pub failed: Version,
    /// Versión a la que se volvió.
    pub restored: Version,
}

impl UpdateState {
    /// Estado nuevo con un identificador de instalación aleatorio (`seed`: 16 bytes aleatorios del llamador).
    pub fn new(seed: [u8; 16]) -> Self {
        Self {
            install_id: seed.iter().map(|b| format!("{b:02x}")).collect(),
            ..Self::default()
        }
    }

    /// Estado guardado, o uno nuevo con identificador aleatorio si no existe o está dañado.
    pub fn load_or_create(path: &Path) -> Result<Self> {
        if let Some(state) = Self::load(path)?
            && !state.install_id.is_empty()
        {
            return Ok(state);
        }
        let mut seed = [0u8; 16];
        getrandom::fill(&mut seed).map_err(|e| std::io::Error::other(e.to_string()))?;
        let state = Self::new(seed);
        state.save(path)?;
        Ok(state)
    }

    /// Se llama al arrancar con la versión en marcha. Cierra la actualización pendiente si ya es esta versión,
    /// anota el historial y, si el lanzador volvió atrás (`rolled_back_from`), prepara el aviso y omite la versión
    /// fallida. Devuelve `true` si el estado cambió.
    pub fn reconcile_launch(&mut self, running: &Version, now: &str, rolled_back_from: Option<&Version>) -> bool {
        let before = self.clone();
        if let Some(failed) = rolled_back_from {
            self.rollback_notice = Some(RollbackNotice {
                failed: failed.clone(),
                restored: running.clone(),
            });
            self.skipped_version = Some(failed.clone());
            if self.pending.as_ref().is_some_and(|p| &p.version == failed) {
                self.pending = None;
            }
        }
        if self.pending.as_ref().is_some_and(|p| &p.version <= running) {
            self.pending = None;
        }
        if self.history.last().map(|h| &h.version) != Some(running) {
            if !self.history.is_empty() && rolled_back_from.is_none() {
                self.whats_new = Some(running.clone());
            }
            self.history.push(HistoryEntry {
                version: running.clone(),
                at: now.to_owned(),
            });
            let excess = self.history.len().saturating_sub(20);
            self.history.drain(..excess);
        }
        *self != before
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

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Version {
        s.parse().unwrap()
    }

    #[test]
    fn install_id_is_created_once() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("update-state.json");
        let a = UpdateState::load_or_create(&path).unwrap();
        assert_eq!(a.install_id.len(), 32);
        assert_eq!(UpdateState::load_or_create(&path).unwrap().install_id, a.install_id);
    }

    #[test]
    fn launch_after_update_records_history_and_whats_new() {
        let mut s = UpdateState::new([1; 16]);
        assert!(s.reconcile_launch(&v("0.1.0"), "t0", None));
        assert!(s.whats_new.is_none(), "la primera instalación no tiene novedades");
        s.pending = Some(PendingUpdate {
            version: v("0.1.1"),
            notes: None,
            mandatory: false,
        });
        assert!(s.reconcile_launch(&v("0.1.1"), "t1", None));
        assert_eq!(s.whats_new, Some(v("0.1.1")));
        assert!(s.pending.is_none());
        assert_eq!(s.history.len(), 2);
        assert!(!s.reconcile_launch(&v("0.1.1"), "t2", None), "sin cambios al reabrir");
    }

    #[test]
    fn rollback_skips_failed_version_and_leaves_notice() {
        let mut s = UpdateState::new([1; 16]);
        s.reconcile_launch(&v("0.1.1"), "t0", None);
        s.pending = Some(PendingUpdate {
            version: v("0.1.2"),
            notes: None,
            mandatory: false,
        });
        s.reconcile_launch(&v("0.1.1"), "t1", Some(&v("0.1.2")));
        assert_eq!(s.skipped_version, Some(v("0.1.2")));
        assert_eq!(s.rollback_notice.as_ref().unwrap().restored, v("0.1.1"));
        assert!(s.pending.is_none());
        assert!(s.whats_new.is_none());
    }
}
