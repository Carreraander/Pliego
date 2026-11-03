//! Registro de trabajos en segundo plano. Un flujo lanza su operación larga con [`JobRegistry::spawn`] y recibe un
//! `JobId`; la interfaz sigue el progreso con los eventos `job-progress` y `job-finished` y puede cancelar con
//! `cancel_job`. El registro no conoce Tauri: emite [`JobEvent`] a un receptor que la capa Tauri traduce a eventos.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use pliego_core::{CancelToken, Error, JobContext, Progress, Report, Result};
use serde::Serialize;

pub type JobId = u64;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", rename_all = "kebab-case")]
pub enum JobEvent {
    /// `job-progress { job_id, done, total, label }`
    #[serde(rename = "job-progress")]
    Progress {
        job_id: JobId,
        done: u64,
        total: u64,
        label: String,
    },
    /// `job-finished { job_id, result | error }`
    #[serde(rename = "job-finished")]
    Finished {
        job_id: JobId,
        result: Option<Report>,
        error: Option<JobError>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JobError {
    pub code: &'static str,
    pub message: String,
    pub cancelled: bool,
}

impl From<&Error> for JobError {
    fn from(e: &Error) -> Self {
        let cancelled = matches!(e, Error::Cancelled);
        JobError {
            code: if cancelled { "cancelled" } else { "other" },
            message: e.to_string(),
            cancelled,
        }
    }
}

type Sink = Arc<dyn Fn(JobEvent) + Send + Sync>;

#[derive(Default)]
struct Inner {
    tokens: Mutex<HashMap<JobId, CancelToken>>,
    next: AtomicU64,
}

#[derive(Clone)]
pub struct JobRegistry {
    inner: Arc<Inner>,
    sink: Sink,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

struct EventProgress {
    job_id: JobId,
    sink: Sink,
}

impl Progress for EventProgress {
    fn report(&self, done: u64, total: u64, message: &str) {
        (self.sink)(JobEvent::Progress {
            job_id: self.job_id,
            done,
            total,
            label: message.to_owned(),
        });
    }
}

impl JobRegistry {
    pub fn new(sink: impl Fn(JobEvent) + Send + Sync + 'static) -> Self {
        Self {
            inner: Arc::new(Inner::default()),
            sink: Arc::new(sink),
        }
    }

    /// Lanza `work` en un hilo y devuelve su identificador de inmediato. `work` recibe el contexto estándar de
    /// `pliego-core` (progreso y cancelación); al acabar se emite `job-finished` con el informe o el error.
    pub fn spawn<F>(&self, work: F) -> JobId
    where
        F: FnOnce(&JobContext) -> Result<Report> + Send + 'static,
    {
        let id = self.inner.next.fetch_add(1, Ordering::Relaxed) + 1;
        let token = CancelToken::new();
        lock(&self.inner.tokens).insert(id, token.clone());
        let registry = self.clone();
        std::thread::spawn(move || {
            let progress = EventProgress {
                job_id: id,
                sink: registry.sink.clone(),
            };
            let ctx = JobContext::new(&progress, token);
            let outcome = work(&ctx);
            lock(&registry.inner.tokens).remove(&id);
            let event = match outcome {
                Ok(report) => JobEvent::Finished {
                    job_id: id,
                    result: Some(report),
                    error: None,
                },
                Err(e) => JobEvent::Finished {
                    job_id: id,
                    result: None,
                    error: Some(JobError::from(&e)),
                },
            };
            (registry.sink)(event);
        });
        id
    }

    /// Pide cancelar un trabajo en curso; `false` si ya terminó o no existe. El trabajo lo nota en su próximo `ctx.step`.
    pub fn cancel(&self, id: JobId) -> bool {
        lock(&self.inner.tokens).get(&id).map(CancelToken::cancel).is_some()
    }

    pub fn running(&self) -> usize {
        lock(&self.inner.tokens).len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    fn registry() -> (JobRegistry, mpsc::Receiver<JobEvent>) {
        let (tx, rx) = mpsc::channel();
        let tx = Mutex::new(tx);
        (
            JobRegistry::new(move |e| {
                let _ = lock(&tx).send(e);
            }),
            rx,
        )
    }

    fn finished(rx: &mpsc::Receiver<JobEvent>) -> JobEvent {
        loop {
            match rx.recv_timeout(Duration::from_secs(5)).expect("el trabajo no terminó") {
                e @ JobEvent::Finished { .. } => return e,
                JobEvent::Progress { .. } => {}
            }
        }
    }

    #[test]
    fn job_reports_progress_and_result() {
        let (reg, rx) = registry();
        let id = reg.spawn(|ctx| {
            ctx.step(1, 2, "mitad")?;
            ctx.step(2, 2, "fin")?;
            Ok(Report::with_output("salida.pdf", "listo"))
        });
        match rx.recv_timeout(Duration::from_secs(5)).unwrap() {
            JobEvent::Progress {
                job_id,
                done,
                total,
                label,
            } => assert_eq!((job_id, done, total, label.as_str()), (id, 1, 2, "mitad")),
            other => panic!("inesperado: {other:?}"),
        }
        match finished(&rx) {
            JobEvent::Finished {
                job_id,
                result: Some(r),
                error: None,
            } => {
                assert_eq!(job_id, id);
                assert_eq!(r.summary, "listo");
            }
            other => panic!("inesperado: {other:?}"),
        }
    }

    #[test]
    fn cancel_is_seen_at_next_step() {
        let (reg, rx) = registry();
        let (go_tx, go_rx) = mpsc::channel::<()>();
        let id = reg.spawn(move |ctx| {
            go_rx.recv().unwrap();
            ctx.step(1, 2, "x")?;
            Ok(Report::default())
        });
        assert!(reg.cancel(id));
        go_tx.send(()).unwrap();
        match finished(&rx) {
            JobEvent::Finished { error: Some(e), .. } => assert!(e.cancelled),
            other => panic!("inesperado: {other:?}"),
        }
        assert!(!reg.cancel(id), "ya terminó");
        assert_eq!(reg.running(), 0);
    }

    #[test]
    fn failure_becomes_error_event() {
        let (reg, rx) = registry();
        reg.spawn(|_| Err(Error::InvalidInput("rango no válido".into())));
        match finished(&rx) {
            JobEvent::Finished {
                error: Some(e),
                result: None,
                ..
            } => assert!(e.message.contains("rango")),
            other => panic!("inesperado: {other:?}"),
        }
    }
}
