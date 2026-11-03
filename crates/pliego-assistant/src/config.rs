use serde::{Deserialize, Serialize};

/// Ajustes del asistente. Por defecto está **desactivado**: nada se carga ni se descarga hasta que la
/// persona lo active en Ajustes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssistantConfig {
    pub enabled: bool,
    /// Identificador del modelo elegido (ver [`crate::catalog`]).
    pub model_id: Option<String>,
    /// Pasajes que se entregan al modelo en cada pregunta.
    pub top_k: usize,
    /// Tamaño máximo de respuesta, en tokens.
    pub max_answer_tokens: u32,
}

impl Default for AssistantConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            model_id: None,
            top_k: 5,
            max_answer_tokens: 400,
        }
    }
}

impl AssistantConfig {
    /// Lee los ajustes de `path`; si no existen o están dañados, devuelve los de por defecto (desactivado).
    pub fn load_or_default(path: &std::path::Path) -> Self {
        std::fs::read(path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    /// Guarda los ajustes de forma atómica (temporal + renombrado).
    pub fn save(&self, path: &std::path::Path) -> pliego_core::Result<()> {
        use pliego_core::Error;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| Error::io_at(dir, e))?;
        }
        let tmp = path.with_extension("tmp");
        let json = serde_json::to_vec_pretty(self).map_err(|e| Error::Other(e.to_string()))?;
        std::fs::write(&tmp, json).map_err(|e| Error::io_at(&tmp, e))?;
        std::fs::rename(&tmp, path).map_err(|e| Error::io_at(path, e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub").join("assistant.json");
        assert!(!AssistantConfig::load_or_default(&path).enabled);
        let cfg = AssistantConfig {
            enabled: true,
            model_id: Some("x".into()),
            ..Default::default()
        };
        cfg.save(&path).unwrap();
        assert_eq!(AssistantConfig::load_or_default(&path), cfg);
        std::fs::write(&path, b"{ roto").unwrap();
        assert!(!AssistantConfig::load_or_default(&path).enabled);
    }
}
