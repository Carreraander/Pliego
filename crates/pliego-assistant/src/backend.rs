use pliego_core::{Error, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
        }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
        }
    }
}

/// Petición de generación. El motor aplica la plantilla de chat propia del modelo.
#[derive(Debug, Clone, PartialEq)]
pub struct GenerateRequest {
    pub messages: Vec<Message>,
    pub max_tokens: u32,
    pub temperature: f32,
}

/// Motor de lenguaje local. Lo implementa llama.cpp (característica `llama`); en las pruebas, un doble.
pub trait LlmBackend: Send + Sync {
    /// Genera texto. `on_token` recibe cada trozo según se produce; devolver `false` detiene la generación.
    fn generate(&self, req: &GenerateRequest, on_token: &mut dyn FnMut(&str) -> bool) -> Result<String>;

    /// Ventana de contexto del modelo, en tokens.
    fn context_tokens(&self) -> usize;

    /// Vector de un texto. `None` si el modelo no calcula vectores: entonces solo se usa la búsqueda léxica.
    fn embed(&self, _text: &str) -> Result<Option<Vec<f32>>> {
        Ok(None)
    }

    /// Descripción de una imagen PNG (solo modelos multimodales).
    fn describe_image(&self, _png: &[u8], _instruction: &str) -> Result<String> {
        Err(Error::Unsupported("el modelo elegido no entiende imágenes".into()))
    }
}
