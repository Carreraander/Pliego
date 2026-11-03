//! Registro de comandos Tauri: un archivo por área.
//!
//! Para añadir los tuyos: crea `commands/<area>.rs`, declara `pub mod <area>;` aquí y añade tus funciones
//! a la lista de `handler()`. Ver `docs/app-integration.md`.

pub mod assistant;
pub mod dialog;
pub mod document;
pub mod search;
pub mod startup;
pub mod text;

/// Lista de comandos expuestos a la interfaz.
pub fn handler() -> impl Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        dialog::pick_pdf_files,
        document::open_document,
        document::close_document,
        document::document_info,
        document::page_sizes,
        document::bump_revision,
        search::search,
        text::outline,
        text::page_text,
        startup::startup_files,
        assistant::assistant_status,
        assistant::assistant_set_enabled,
        assistant::assistant_select_model,
        assistant::assistant_cancel,
        assistant::assistant_download,
        assistant::assistant_ask,
        assistant::assistant_summarize,
        assistant::assistant_keydata,
    ]
}
