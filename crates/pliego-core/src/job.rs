use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::{Error, Result};

/// Token de cancelación compartido entre la interfaz y el trabajo en segundo plano.
#[derive(Clone, Debug, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// Receptor de progreso. La app lo traduce a eventos de la interfaz; la CLI a una barra.
pub trait Progress: Send + Sync {
    fn report(&self, done: u64, total: u64, message: &str);
}

/// Progreso que no hace nada (tests, uso como librería).
pub struct NoProgress;

impl Progress for NoProgress {
    fn report(&self, _done: u64, _total: u64, _message: &str) {}
}

/// Contexto que reciben todas las operaciones largas.
pub struct JobContext<'a> {
    pub progress: &'a dyn Progress,
    pub cancel: CancelToken,
}

impl<'a> JobContext<'a> {
    pub fn new(progress: &'a dyn Progress, cancel: CancelToken) -> Self {
        Self { progress, cancel }
    }

    /// Contexto sin progreso ni cancelación externa.
    pub fn silent() -> JobContext<'static> {
        JobContext {
            progress: &NoProgress,
            cancel: CancelToken::new(),
        }
    }

    /// Llamar entre pasos (por ejemplo, por página). Devuelve `Err(Cancelled)` si se canceló.
    pub fn step(&self, done: u64, total: u64, message: &str) -> Result<()> {
        if self.cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        self.progress.report(done, total, message);
        Ok(())
    }
}

/// Resultado de una operación: archivos producidos y avisos legibles por la persona usuaria.
#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct Report {
    pub outputs: Vec<PathBuf>,
    pub warnings: Vec<String>,
    pub summary: String,
}

impl Report {
    pub fn with_output(output: impl Into<PathBuf>, summary: impl Into<String>) -> Self {
        Self {
            outputs: vec![output.into()],
            warnings: Vec::new(),
            summary: summary.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_stops_step() {
        let ctx = JobContext::silent();
        assert!(ctx.step(0, 1, "a").is_ok());
        ctx.cancel.cancel();
        assert!(matches!(ctx.step(1, 1, "b"), Err(Error::Cancelled)));
    }
}
