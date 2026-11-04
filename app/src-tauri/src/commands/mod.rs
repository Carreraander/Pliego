//! Registro de comandos Tauri: un archivo por área.
//!
//! Para añadir los tuyos: crea `commands/<area>.rs`, declara `pub mod <area>;` aquí y añade tus funciones
//! a la lista de `handler()`. Ver `docs/app-integration.md`.

pub mod assistant;
pub mod dialog;
pub mod document;
pub mod jobs;
pub mod organize;
pub mod recents;
pub mod search;
pub mod settings;
pub mod stamp;
pub mod startup;
pub mod text;
pub mod updates;

/// Lista de comandos expuestos a la interfaz.
pub fn handler() -> impl Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        dialog::pick_pdf_files,
        dialog::pick_image_file,
        document::open_document,
        document::close_document,
        document::document_info,
        document::page_sizes,
        document::bump_revision,
        search::search,
        settings::get_settings,
        settings::set_settings,
        recents::recents_list,
        recents::recents_add,
        recents::recents_remove,
        recents::recents_pin,
        recents::recent_thumbnail,
        recents::reveal_path,
        recents::about_info,
        jobs::cancel_job,
        organize::merge_pdfs,
        organize::split_pdf,
        stamp::stamp_pdf,
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
        updates::update_status,
        updates::update_check_now,
        updates::update_download,
        updates::update_restart_now,
        updates::update_set_settings,
        updates::update_skip_version,
        updates::update_dismiss_notices,
        updates::update_dismiss_rollback,
    ]
}
