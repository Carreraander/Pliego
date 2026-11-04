use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;

use pliego_core::{Error, JobContext, Report, Result};
use serde_json::Value;

/// Contadores compartidos entre los archivos de un lote, por nombre. Sirven para pasos que continúan de un
/// archivo al siguiente (p. ej. la numeración Bates de un expediente). El ejecutor los guarda en el diario tras
/// cada archivo, de modo que `--resume` los retoma, y los restaura si un archivo falla (sin huecos).
#[derive(Debug, Default)]
pub struct Batch {
    counters: Mutex<BTreeMap<String, u64>>,
}

impl Batch {
    pub fn get(&self, key: &str) -> u64 {
        self.lock().get(key).copied().unwrap_or(0)
    }

    pub fn add(&self, key: &str, n: u64) {
        *self.lock().entry(key.to_string()).or_insert(0) += n;
    }

    pub(crate) fn snapshot(&self) -> BTreeMap<String, u64> {
        self.lock().clone()
    }

    pub(crate) fn restore(&self, counters: BTreeMap<String, u64>) {
        *self.lock() = counters;
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, u64>> {
        self.counters.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Un tipo de paso. Lee `input`, escribe un PDF nuevo en `output` (nunca modifica `input`) y devuelve su informe.
pub trait StepRunner: Send + Sync {
    /// Comprueba las opciones antes de empezar el lote, para fallar a la primera y no en el archivo 500.
    fn validate(&self, _options: &Value) -> Result<()> {
        Ok(())
    }

    fn run(&self, input: &Path, output: &Path, options: &Value, ctx: &JobContext) -> Result<Report>;

    /// Como [`StepRunner::run`], con acceso a los contadores del lote. Por defecto los ignora.
    fn run_in_batch(
        &self,
        input: &Path,
        output: &Path,
        options: &Value,
        ctx: &JobContext,
        _batch: &Batch,
    ) -> Result<Report> {
        self.run(input, output, options, ctx)
    }
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
