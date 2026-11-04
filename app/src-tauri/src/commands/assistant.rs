//! Asistente local: estado, descarga de modelos, preguntas, resúmenes y datos clave.
//!
//! Capa fina: la lógica vive en `pliego-assistant`. Los trabajos largos corren fuera del hilo de la
//! interfaz, informan con el evento `assistant-progress` y se cancelan con `assistant_cancel(job_id)`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use pliego_app_service::api::{ApiError, ApiResult};
use pliego_app_service::{DocId, DocumentService};
use pliego_assistant::{
    Answer, Assistant, DocData, DocIndex, DocumentText, KeyDatum, ModelSpec, Session, Summary, catalog, download_model,
    extract_key_data,
};
use pliego_core::{CancelToken, Error, JobContext, Progress};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::AppState;

/// Estado del asistente que gestiona Tauri (`app.manage`).
#[derive(Clone, Default)]
pub struct AssistantState {
    session: Arc<Mutex<Session>>,
    jobs: Arc<Mutex<HashMap<String, CancelToken>>>,
}

impl AssistantState {
    /// Libera el índice y el texto en caché de un documento cerrado.
    pub fn forget(&self, doc: u64) {
        lock(&self.session).forget(doc);
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

#[derive(Serialize)]
pub struct ModelInfo {
    #[serde(flatten)]
    spec: ModelSpec,
    installed: bool,
}

#[derive(Serialize)]
pub struct Status {
    enabled: bool,
    model_id: Option<String>,
    /// La app incluye el motor llama.cpp (si no, el asistente no puede responder).
    engine_available: bool,
    models_dir: String,
    models: Vec<ModelInfo>,
}

#[derive(Clone, Serialize)]
struct ProgressEvent {
    job_id: String,
    done: u64,
    total: u64,
    label: String,
}

struct Emit {
    app: AppHandle,
    job_id: String,
}

impl Progress for Emit {
    fn report(&self, done: u64, total: u64, message: &str) {
        let _ = self.app.emit(
            "assistant-progress",
            ProgressEvent {
                job_id: self.job_id.clone(),
                done,
                total,
                label: message.to_string(),
            },
        );
    }
}

fn status_of(session: &Session) -> Status {
    Status {
        enabled: session.config().enabled,
        model_id: session.config().model_id.clone(),
        engine_available: Session::engine_available(),
        models_dir: session.models_dir().display().to_string(),
        models: catalog()
            .iter()
            .map(|m| ModelInfo {
                spec: m.clone(),
                installed: session.model_path(m).is_some(),
            })
            .collect(),
    }
}

#[tauri::command]
pub fn assistant_status(assistant: State<'_, AssistantState>) -> Status {
    status_of(&lock(&assistant.session))
}

#[tauri::command]
pub fn assistant_set_enabled(assistant: State<'_, AssistantState>, enabled: bool) -> ApiResult<Status> {
    let mut s = lock(&assistant.session);
    s.set_enabled(enabled)?;
    Ok(status_of(&s))
}

#[tauri::command]
pub fn assistant_select_model(assistant: State<'_, AssistantState>, id: String) -> ApiResult<Status> {
    let mut s = lock(&assistant.session);
    s.select_model(&id)?;
    Ok(status_of(&s))
}

/// Cancela un trabajo en curso (descarga, pregunta o resumen).
#[tauri::command]
pub fn assistant_cancel(assistant: State<'_, AssistantState>, job_id: String) {
    if let Some(token) = lock(&assistant.jobs).get(&job_id) {
        token.cancel();
    }
}

/// Descarga un modelo (única operación del asistente con red) y lo deja elegido.
#[tauri::command]
pub async fn assistant_download(
    app: AppHandle,
    assistant: State<'_, AssistantState>,
    id: String,
    job_id: String,
) -> ApiResult<Status> {
    let st = assistant.inner().clone();
    run_job(app, st, job_id, move |st, ctx| {
        let spec = ModelSpec::find(&id).ok_or_else(|| Error::InvalidInput(format!("no existe el modelo «{id}»")))?;
        let dir = lock(&st.session).models_dir().to_path_buf();
        download_model(spec, &dir, ctx)?;
        let mut s = lock(&st.session);
        s.select_model(&id)?;
        Ok(status_of(&s))
    })
    .await
}

#[tauri::command]
pub async fn assistant_ask(
    app: AppHandle,
    state: State<'_, AppState>,
    assistant: State<'_, AssistantState>,
    doc_id: DocId,
    question: String,
    job_id: String,
) -> ApiResult<Answer> {
    let (st, service) = (assistant.inner().clone(), state.service.clone());
    run_job(app, st, job_id, move |st, ctx| {
        let a = lock(&st.session).assistant()?;
        let data = doc_data(&service, st, &a, doc_id, ctx)?;
        a.ask_indexed(&data.index, &question, ctx)
    })
    .await
}

#[tauri::command]
pub async fn assistant_summarize(
    app: AppHandle,
    state: State<'_, AppState>,
    assistant: State<'_, AssistantState>,
    doc_id: DocId,
    job_id: String,
) -> ApiResult<Summary> {
    let (st, service) = (assistant.inner().clone(), state.service.clone());
    run_job(app, st, job_id, move |st, ctx| {
        let a = lock(&st.session).assistant()?;
        let data = doc_data(&service, st, &a, doc_id, ctx)?;
        a.summarize(&data.text, ctx)
    })
    .await
}

/// Fechas e importes del documento. No necesita modelo, pero sí que el asistente esté activado.
#[tauri::command]
pub async fn assistant_keydata(
    app: AppHandle,
    state: State<'_, AppState>,
    assistant: State<'_, AssistantState>,
    doc_id: DocId,
    job_id: String,
) -> ApiResult<Vec<KeyDatum>> {
    let (st, service) = (assistant.inner().clone(), state.service.clone());
    run_job(app, st, job_id, move |st, ctx| {
        if !lock(&st.session).config().enabled {
            return Err(Error::InvalidInput(
                "el asistente está desactivado; actívalo en sus ajustes".into(),
            ));
        }
        Ok(extract_key_data(&document_text(&service, doc_id, ctx)?))
    })
    .await
}

/// Ejecuta `work` en un hilo aparte, con progreso por evento y cancelación por `job_id`.
async fn run_job<T, F>(app: AppHandle, st: AssistantState, job_id: String, work: F) -> ApiResult<T>
where
    T: Send + 'static,
    F: FnOnce(&AssistantState, &JobContext) -> pliego_core::Result<T> + Send + 'static,
{
    let cancel = CancelToken::new();
    lock(&st.jobs).insert(job_id.clone(), cancel.clone());
    let result = tauri::async_runtime::spawn_blocking({
        let (st, job_id) = (st.clone(), job_id.clone());
        move || {
            let progress = Emit { app, job_id };
            work(&st, &JobContext::new(&progress, cancel))
        }
    })
    .await;
    lock(&st.jobs).remove(&job_id);
    match result {
        Ok(r) => Ok(r?),
        Err(e) => Err(ApiError::from(Error::Other(format!(
            "el asistente se interrumpió: {e}"
        )))),
    }
}

/// Texto de todas las páginas, una a una, a través del proceso aislado.
fn document_text(service: &DocumentService, doc: DocId, ctx: &JobContext) -> pliego_core::Result<DocumentText> {
    let total = service.document_info(doc)?.page_count;
    let mut pages = Vec::with_capacity(total as usize);
    for p in 0..total {
        ctx.step(u64::from(p), u64::from(total), "Leyendo el texto del documento")?;
        pages.push(service.page_text(doc, p)?);
    }
    Ok(DocumentText::from_pages(pages))
}

/// Texto e índice del documento para su revisión actual (se reutilizan entre preguntas).
fn doc_data(
    service: &DocumentService,
    st: &AssistantState,
    assistant: &Assistant,
    doc: DocId,
    ctx: &JobContext,
) -> pliego_core::Result<Arc<DocData>> {
    let revision = service.revision(doc)?;
    if let Some(d) = lock(&st.session).doc_data(doc.0, revision) {
        return Ok(d);
    }
    let text = document_text(service, doc, ctx)?;
    let index: DocIndex = assistant.index(&text, ctx)?;
    Ok(lock(&st.session).store_doc_data(doc.0, revision, DocData { text, index }))
}
