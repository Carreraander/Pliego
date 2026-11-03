use std::path::Path;

use pliego_core::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Qué hacer cuando un archivo falla.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorPolicy {
    /// Anotar el fallo en el informe y seguir con el siguiente archivo.
    #[default]
    Continue,
    /// Parar en el primer fallo.
    Stop,
}

/// Un paso: el tipo (`optimize`, `protect.encrypt`…) y sus opciones, que son las del módulo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Step {
    pub kind: String,
    #[serde(flatten)]
    pub options: Map<String, Value>,
}

impl Step {
    pub fn new(kind: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            options: Map::new(),
        }
    }

    pub fn with(mut self, key: &str, value: impl Into<Value>) -> Self {
        self.options.insert(key.into(), value.into());
        self
    }

    /// Las opciones como un único valor JSON, que es lo que reciben los [`crate::StepRunner`].
    pub fn options_value(&self) -> Value {
        Value::Object(self.options.clone())
    }
}

/// Acción guardada: nombre, descripción y pasos en orden.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Action {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub on_error: ErrorPolicy,
    pub steps: Vec<Step>,
}

impl Action {
    /// Lee una acción de un archivo `.toml` o `.json` (por la extensión; sin extensión conocida se prueba TOML).
    pub fn load(path: &Path) -> Result<Action> {
        let text = std::fs::read_to_string(path).map_err(|e| Error::io_at(path, e))?;
        let is_json = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("json"));
        let action: Action = if is_json {
            serde_json::from_str(&text).map_err(|e| Error::InvalidInput(format!("{}: {e}", path.display())))?
        } else {
            toml::from_str(&text).map_err(|e| Error::InvalidInput(format!("{}: {e}", path.display())))?
        };
        action.validate()?;
        Ok(action)
    }

    pub fn validate(&self) -> Result<()> {
        if self.steps.is_empty() {
            return Err(Error::InvalidInput("la acción no tiene ningún paso".into()));
        }
        if let Some(i) = self.steps.iter().position(|s| s.kind.trim().is_empty()) {
            return Err(Error::InvalidInput(format!("el paso {} no indica su tipo", i + 1)));
        }
        Ok(())
    }

    /// Huella estable de la acción: la reanudación solo vale si la acción no ha cambiado.
    pub fn fingerprint(&self) -> String {
        let text = serde_json::to_string(self).unwrap_or_default();
        let mut h: u64 = 0xcbf29ce484222325;
        for b in text.bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x100000001b3);
        }
        format!("{h:016x}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_toml_with_flattened_options() {
        let a: Action = toml::from_str(
            r#"
name = "Archivar"
[[steps]]
kind = "optimize"
preset = "ebook"
[[steps]]
kind = "protect.encrypt"
user_password = "x"
"#,
        )
        .unwrap();
        assert_eq!(a.steps.len(), 2);
        assert_eq!(a.steps[0].options["preset"], "ebook");
        assert_eq!(a.on_error, ErrorPolicy::Continue);
    }

    #[test]
    fn rejects_empty() {
        let a = Action {
            name: String::new(),
            description: String::new(),
            on_error: ErrorPolicy::Continue,
            steps: vec![],
        };
        assert!(a.validate().is_err());
    }
}
