//! Registro de comandos Tauri: un archivo por área.
//!
//! Para añadir los tuyos: crea `commands/<area>.rs`, declara `pub mod <area>;` aquí y añade tus funciones
//! a la lista de `handler()`. Ver `docs/app-integration.md`.

pub mod actions;
pub mod annot;
pub mod assistant;
pub mod checks;
pub mod convert;
pub mod dialog;
pub mod document;
pub mod index;
pub mod jobs;
pub mod optimize;
pub mod organize;
pub mod portfolio;
pub mod protect;
pub mod recents;
pub mod search;
pub mod settings;
pub mod sign;
pub mod stamp;
pub mod startup;
pub mod text;
pub mod updates;

/// Lista de comandos expuestos a la interfaz.
pub fn handler() -> impl Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        dialog::pick_pdf_files,
        dialog::pick_key_file,
        dialog::pick_folder,
        dialog::pick_action_file,
        dialog::pick_any_files,
        dialog::pick_image_file,
        dialog::pick_source_files,
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
        convert::create_pdf,
        checks::check_accessibility,
        checks::validate_pdfa,
        checks::preflight_pdf,
        annot::annot_summary,
        annot::annot_add,
        annot::annot_delete,
        annot::annot_undo,
        annot::forms_fields,
        annot::forms_fill,
        annot::export_comments_xfdf,
        annot::import_comments_xfdf,
        annot::pick_xfdf_file,
        actions::actions_load,
        actions::actions_run,
        index::index_folder,
        index::index_search,
        portfolio::portfolio_create,
        portfolio::portfolio_list,
        portfolio::portfolio_extract,
        convert::export_pdf,
        convert::pdfa_convert,
        convert::office_export_pdf,
        optimize::compress_pdf,
        optimize::analyze_pdf,
        organize::merge_pdfs,
        organize::split_pdf,
        sign::sign_list_certificates,
        sign::sign_pdf,
        sign::sign_validate,
        sign::signflow_prepare,
        sign::signflow_next,
        sign::signflow_sign,
        sign::signflow_audit,
        protect::protect_encrypt,
        protect::protect_remove,
        protect::protect_sanitize,
        protect::redact_find,
        protect::redact_apply,
        organize::insert_pages,
        organize::number_pages,
        organize::organize_pages,
        stamp::stamp_pdf,
        stamp::stamp_annot_pdf,
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
