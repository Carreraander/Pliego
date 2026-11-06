//! App de escritorio de Pliego: capa fina de Tauri sobre `pliego-app-service`.

mod commands;

use std::sync::{Arc, Mutex};

use pliego_app_service::DocumentService;
use tauri::http::{Response, StatusCode, header};
use tauri::{Emitter, Manager};

/// Estado compartido de la app, accesible desde todos los comandos con `State<AppState>`.
pub struct AppState {
    pub service: Arc<DocumentService>,
    pub startup_files: Mutex<Vec<String>>,
}

fn tile_response(service: &DocumentService, path: &str) -> Response<Vec<u8>> {
    match service.tile_from_uri(path) {
        Ok(png) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "image/png")
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .header(header::CACHE_CONTROL, "no-store")
            .body(png)
            .unwrap_or_default(),
        Err(e) => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(e.to_string().into_bytes())
            .unwrap_or_default(),
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            // Segunda instancia: pasa sus archivos a la primera y la trae al frente.
            let files = commands::startup::pdf_args(args);
            let _ = app.emit("open-files", files);
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        .register_asynchronous_uri_scheme_protocol("pliego", |ctx, request, responder| {
            // En Windows (WebView2) la URL llega como http://pliego.localhost/tile/...; basta con la ruta.
            let path = request.uri().path().to_owned();
            let service = ctx.app_handle().state::<AppState>().service.clone();
            // El render puede tardar: fuera del hilo de la interfaz.
            std::thread::spawn(move || responder.respond(tile_response(&service, &path)));
        })
        .setup(|app| {
            let handle = app.handle().clone();
            let data_dir = pliego_appdata::data_dir();
            commands::logs::start(&data_dir);
            // El worker (PDFium y lopdf, aislados) va junto al ejecutable; en desarrollo, PLIEGO_WORKER.
            let service = DocumentService::discover()
                .inspect_err(|e| pliego_core::log::error("arranque", format!("no se encontró el worker: {e}")))?
                .on_document_changed(move |ev| {
                    let _ = handle.emit("document-changed", ev);
                });
            let settings = pliego_appdata::settings::SettingsStore::new(&data_dir);
            app.manage(commands::updates::start(app.handle(), &settings));
            app.manage(commands::settings::SettingsState(settings));
            app.manage(commands::recents::RecentsState(
                pliego_appdata::recents::RecentStore::new(&data_dir),
            ));
            // Marca de «en marcha»: si ya estaba, la vez anterior Pliego se cerró de golpe.
            let session = pliego_appdata::session::SessionStore::new(&data_dir);
            let previous = session.start();
            if previous.crashed {
                pliego_core::log::warn(
                    "arranque",
                    format!("la sesión anterior no se cerró bien ({} pestañas)", previous.docs.len()),
                );
            }
            app.manage(commands::session::SessionState {
                store: session,
                previous,
            });
            app.manage(commands::jobs::JobsState::new(app.handle().clone()));
            app.manage(commands::assistant::AssistantState::default());
            app.manage(commands::annot::AnnotEditState::default());
            app.manage(AppState {
                service: Arc::new(service),
                startup_files: Mutex::new(commands::startup::pdf_args(std::env::args())),
            });
            Ok(())
        })
        .invoke_handler(commands::handler())
        .build(tauri::generate_context!())
        .unwrap_or_else(|e| {
            pliego_core::log::error("arranque", format!("no se pudo iniciar Pliego: {e}"));
            panic!("no se pudo iniciar Pliego: {e}");
        })
        .run(|app, event| {
            // Salida normal: se quita la marca de «en marcha» para que el próximo arranque no ofrezca recuperar.
            if let tauri::RunEvent::Exit = event {
                if let Some(s) = app.try_state::<commands::session::SessionState>() {
                    s.store.clean_exit();
                }
                pliego_core::log::info("cierre", "Pliego se cerró bien");
            }
        });
}
