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
