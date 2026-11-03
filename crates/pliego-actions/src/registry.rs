use std::collections::BTreeMap;
use std::path::Path;

use pliego_core::{Error, JobContext, Report, Result};
use serde_json::Value;

/// Un tipo de paso. Lee `input`, escribe un PDF nuevo en `output` (nunca modifica `input`) y devuelve su informe.
pub trait StepRunner: Send + Sync {
    /// Comprueba las opciones antes de empezar el lote, para fallar a la primera y no en el archivo 500.
    fn validate(&self, _options: &Value) -> Result<()> {
        Ok(())
    }

    fn run(&self, input: &Path, output: &Path, options: &Value, ctx: &JobContext) -> Result<Report>;
}

/// Pasos disponibles por nombre (`optimize`, `protect.encrypt`…).
#[derive(Default)]
pub struct Registry {
    steps: BTreeMap<String, Box<dyn StepRunner>>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, kind: impl Into<String>, runner: impl StepRunner + 'static) -> &mut Self {
        self.steps.insert(kind.into(), Box::new(runner));
        self
    }

    pub fn get(&self, kind: &str) -> Result<&dyn StepRunner> {
        self.steps.get(kind).map(|b| b.as_ref()).ok_or_else(|| {
            let known: Vec<&str> = self.steps.keys().map(String::as_str).collect();
            Error::InvalidInput(format!(
                "paso desconocido «{kind}». Disponibles: {}",
                if known.is_empty() {
                    "ninguno".into()
                } else {
                    known.join(", ")
                }
            ))
        })
    }

    pub fn kinds(&self) -> impl Iterator<Item = &str> {
        self.steps.keys().map(String::as_str)
    }
}

/// Adaptador para registrar una función o cierre como paso.
impl<F> StepRunner for F
where
    F: Fn(&Path, &Path, &Value, &JobContext) -> Result<Report> + Send + Sync,
{
    fn run(&self, input: &Path, output: &Path, options: &Value, ctx: &JobContext) -> Result<Report> {
        self(input, output, options, ctx)
    }
}
