//! Trabajos en segundo plano: el registro que usan todos los flujos para operaciones largas.
//!
//! Un flujo lanza su operación con `state.0.spawn(|ctx| …)` (ver `docs/app-integration.md`, «Herramientas») y devuelve
//! el `JobId` a la interfaz. Los eventos `job-progress {job_id, done, total, label}` y
//! `job-finished {job_id, result | error}` los escucha el centro de tareas.

use pliego_app_service::isolated::{self, JobSpec};
use pliego_appdata::jobs::{JobEvent, JobId, JobRegistry};
use tauri::{AppHandle, Emitter, State};

pub struct JobsState(pub JobRegistry);

impl JobsState {
    /// Lanza una herramienta archivo → archivo como trabajo. Corre en el proceso aislado `pliego __job` (un PDF que
    /// tumbe el motor no cierra la app); ver `pliego_app_service::isolated`.
    pub fn spawn_tool(&self, spec: JobSpec) -> JobId {
        self.0.spawn(move |ctx| isolated::dispatch(spec, ctx))
    }
}

impl JobsState {
    pub fn new(app: AppHandle) -> Self {
        Self(JobRegistry::new(move |event| match &event {
            JobEvent::Progress { .. } => {
                let _ = app.emit("job-progress", &event);
            }
            JobEvent::Finished { .. } => {
                let _ = app.emit("job-finished", &event);
            }
        }))
    }
}

/// Pide cancelar un trabajo; `false` si ya había terminado.
#[tauri::command]
pub fn cancel_job(state: State<'_, JobsState>, job_id: JobId) -> bool {
    state.0.cancel(job_id)
}
