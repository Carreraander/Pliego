//! Interfaz del instalador: ventana sin marco (`tao`) con WebView2 (`wry`) que carga `installer/setup/ui/`.
//! La lógica está en `engine`; aquí solo se traducen los mensajes de la página en llamadas al motor.
#![allow(unsafe_code)]

use crate::cli::{InstallArgs, UiRequest};
use crate::engine::{self, DataDirs, Existing, Source};
use crate::i18n::Lang;
use crate::logging;
use crate::model::{Options, Scope, SetupError, exit};
use crate::platform::{self, ensure_webview2, installed, is_dark_theme, pick_folder, relaunch_elevated};
use serde::Deserialize;
use serde_json::{Value, json};
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use tao::dpi::{LogicalSize, PhysicalPosition};
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::platform::windows::{WindowBuilderExtWindows, WindowExtWindows};
use tao::window::WindowBuilder;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::DwmSetWindowAttribute;
use wry::WebViewBuilder;
use wry::http::{Request, Response};

const ASSETS: &[(&str, &str, &[u8])] = &[
    (
        "/index.html",
        "text/html; charset=utf-8",
        include_bytes!("../../../installer/setup/ui/index.html"),
    ),
    (
        "/tokens.css",
        "text/css; charset=utf-8",
        include_bytes!("../../../installer/setup/ui/tokens.css"),
    ),
    (
        "/setup.css",
        "text/css; charset=utf-8",
        include_bytes!("../../../installer/setup/ui/setup.css"),
    ),
    (
        "/setup.js",
        "text/javascript; charset=utf-8",
        include_bytes!("../../../installer/setup/ui/setup.js"),
    ),
    (
        "/license.txt",
        "text/plain; charset=utf-8",
        include_bytes!("../../../LICENSE"),
    ),
];

const MICA_MIN_BUILD: u32 = 22621;
const ALLOWED_URL: &str = "https://github.com/Carreraander/Pliego";

enum Ev {
    Eval(String),
    Drag,
    Minimize,
    Quit,
}

struct Shared {
    cancel: AtomicBool,
    code: AtomicU32,
    root: Mutex<PathBuf>,
    scope: Mutex<Scope>,
    purge_data: DataDirs,
}

fn asset(req: Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> {
    let path = req.uri().path();
    let path = if path == "/" { "/index.html" } else { path };
    match ASSETS.iter().find(|(p, _, _)| *p == path) {
        Some((_, mime, body)) => Response::builder()
            .header("Content-Type", *mime)
            .header("Cache-Control", "no-store")
            .body(Cow::Borrowed(*body))
            .unwrap_or_default(),
        None => Response::builder()
            .status(404)
            .body(Cow::Borrowed(&b""[..]))
            .unwrap_or_default(),
    }
}

fn js_call(name: &str, args: &[Value]) -> String {
    let list: Vec<String> = args.iter().map(Value::to_string).collect();
    format!("window.__pl.{name}({})", list.join(","))
}

fn options_json(o: &Options) -> Value {
    serde_json::to_value(o).unwrap_or(Value::Null)
}

fn error_json(e: &SetupError) -> Value {
    let (need, drive) = match e {
        SetupError::NoSpace { needed, drive, .. } => (needed.div_ceil(1024 * 1024), drive.clone()),
        _ => (0, String::new()),
    };
    json!({ "kind": e.kind(), "detail": e.to_string(), "need": need, "drive": drive })
}

/// Argumentos para relanzar elevado y empezar a instalar sin volver a preguntar.
fn elevated_args(o: &Options, lang: Lang) -> Vec<String> {
    let mut a = vec![
        "--per-machine".into(),
        "--begin".into(),
        "--dir".into(),
        o.dir.display().to_string(),
        "--lang".into(),
        lang.code().into(),
    ];
    a.push(
        if o.desktop_shortcut {
            "--desktop-shortcut"
        } else {
            "--no-desktop-shortcut"
        }
        .into(),
    );
    a.push(if o.path { "--path" } else { "--no-path" }.into());
    for (on, flag) in [
        (o.assoc, "--no-assoc"),
        (o.context_menu, "--no-context-menu"),
        (o.updates, "--no-updates"),
    ] {
        if !on {
            a.push(flag.into());
        }
    }
    a
}

fn center(window: &tao::window::Window) {
    if let Some(m) = window.primary_monitor() {
        let (ms, ws) = (m.size(), window.outer_size());
        let pos = m.position();
        window.set_outer_position(PhysicalPosition::new(
            pos.x + (ms.width.saturating_sub(ws.width) / 2) as i32,
            pos.y + (ms.height.saturating_sub(ws.height) / 2) as i32,
        ));
    }
}

fn apply_dwm(window: &tao::window::Window, dark: bool, mica: bool) {
    let hwnd = HWND(window.hwnd() as *mut _);
    let set = |attr: i32, value: i32| {
        // SAFETY: `hwnd` es la ventana viva de este hilo y `value` un i32 que vive durante la llamada.
        unsafe {
            let _ = DwmSetWindowAttribute(
                hwnd,
                windows::Win32::Graphics::Dwm::DWMWINDOWATTRIBUTE(attr),
                &value as *const i32 as *const _,
                4,
            );
        }
    };
    set(33, 2); // DWMWA_WINDOW_CORNER_PREFERENCE = redondeadas
    set(20, dark as i32); // DWMWA_USE_IMMERSIVE_DARK_MODE
    if mica {
        set(38, 2); // DWMWA_SYSTEMBACKDROP_TYPE = Mica
    }
}

pub fn run(req: UiRequest) -> u32 {
    let (lang, args, uninstall) = match &req {
        UiRequest::Install { args, lang } => (*lang, args.clone(), None),
        UiRequest::Uninstall { root, scope, lang } => (*lang, InstallArgs::default(), Some((root.clone(), *scope))),
    };
    if !ensure_webview2(lang) {
        return exit::FAILED;
    }

    let sys = platform::system();
    let exe = std::env::current_exe().unwrap_or_default();
    let dark = is_dark_theme();
    let mica = sys.windows_build() >= MICA_MIN_BUILD;
    let env = crate::cli::Env::real();

    // Estado inicial que recibe la página.
    let existing = installed();
    let mut init = json!({
        "lang": lang.code(), "dark": dark, "mica": mica, "version": crate::model::SETUP_VERSION,
        "log": logging::path().display().to_string(), "begin": args.begin,
        "defaults": { "user_dir": env.default_user_dir, "machine_dir": env.default_machine_dir },
    });
    let scope0 = if args.per_machine { Scope::Machine } else { Scope::User };
    let mut options = Options::defaults_for(
        scope0,
        args.dir.clone().unwrap_or_else(|| platform::default_dir(scope0)),
    );
    options.desktop_shortcut = args.desktop_shortcut.unwrap_or(false);
    options.path = args.path.unwrap_or(false);
    options.assoc = args.assoc.unwrap_or(true);
    options.context_menu = args.context_menu.unwrap_or(true);
    options.updates = args.updates.unwrap_or(true);
    let mut watch_root = options.dir.clone();

    let shared_root;
    let shared_scope;
    if let Some((root, scope)) = &uninstall {
        let version = engine::read_info(root).map(|i| i.version).unwrap_or_default();
        init["mode"] = json!("uninstall");
        init["root"] = json!(root);
        init["version"] = json!(version);
        init["size_mb"] = json!(0);
        watch_root = root.clone();
        shared_root = root.clone();
        shared_scope = *scope;
    } else {
        let source = match Source::embedded(exe.clone()) {
            Ok(Some(s)) => s,
            Ok(None) => return exit::FAILED,
            Err(e) => return fatal_before_ui(&e, lang),
        };
        let manifest = match source.manifest() {
            Ok(m) => m,
            Err(e) => return fatal_before_ui(&e, lang),
        };
        init["mode"] = json!("install");
        init["version"] = json!(manifest.version);
        init["size_mb"] = json!(std::fs::metadata(&exe).map(|m| m.len() / (1024 * 1024)).unwrap_or(0));
        if let Some((scope, ver, dir)) = &existing {
            let info = engine::read_info(dir);
            let relation = match engine::classify(Some(ver), &manifest.version) {
                Existing::None | Existing::Same => "same",
                Existing::Older => "older",
                Existing::Newer => "newer",
            };
            init["existing"] = json!({
                "scope": scope, "version": ver, "dir": dir, "relation": relation,
                "options": info.map(|i| options_json(&i.options)),
            });
            watch_root = dir.to_path_buf();
        }
        shared_root = options.dir.clone();
        shared_scope = scope0;
    }
    init["options"] = options_json(&options);
    init["running"] = json!(sys.pliego_running(&watch_root));

    let shared = Arc::new(Shared {
        cancel: AtomicBool::new(false),
        code: AtomicU32::new(exit::CANCELLED),
        root: Mutex::new(shared_root),
        scope: Mutex::new(shared_scope),
        purge_data: env.data.clone(),
    });

    let event_loop = EventLoopBuilder::<Ev>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let window = match WindowBuilder::new()
        .with_title("Pliego")
        .with_inner_size(LogicalSize::new(680.0, 440.0))
        .with_resizable(false)
        .with_decorations(false)
        .with_undecorated_shadow(true)
        .with_transparent(mica)
        .with_visible(false)
        .build(&event_loop)
    {
        Ok(w) => w,
        Err(e) => return fatal_before_ui(&SetupError::Other(e.to_string()), lang),
    };
    center(&window);
    apply_dwm(&window, dark, mica);

    let mut ctx = wry::WebContext::new(Some(std::env::temp_dir().join("pliego-setup-webview2")));
    let handler_shared = shared.clone();
    let handler_proxy = proxy.clone();
    let init_json = init.clone();
    let lang_h = lang;
    let webview = {
        use wry::WebViewBuilderExtWindows;
        WebViewBuilder::new_with_web_context(&mut ctx)
            .with_transparent(mica)
            .with_background_color((0, 0, 0, 0))
            .with_browser_accelerator_keys(false)
            .with_default_context_menus(false)
            .with_devtools(false)
            .with_custom_protocol("pliego".into(), |_, req| asset(req))
            .with_navigation_handler(|url| url.starts_with("http://pliego.localhost"))
            .with_new_window_req_handler(|_, _| wry::NewWindowResponse::Deny)
            .with_ipc_handler(move |req: Request<String>| {
                handle_message(req.body(), &handler_shared, &handler_proxy, &init_json, lang_h);
            })
            .with_url("http://pliego.localhost/index.html")
            .build(&window)
    };
    let webview = match webview {
        Ok(w) => w,
        Err(e) => return fatal_before_ui(&SetupError::Other(format!("WebView2: {e}")), lang),
    };
    window.set_visible(true);
    window.set_focus();

    event_loop.run(move |event, _, flow| {
        *flow = ControlFlow::Wait;
        match event {
            Event::UserEvent(Ev::Eval(js)) => {
                let _ = webview.evaluate_script(&js);
            }
            Event::UserEvent(Ev::Drag) => {
                let _ = window.drag_window();
            }
            Event::UserEvent(Ev::Minimize) => window.set_minimized(true),
            Event::UserEvent(Ev::Quit)
            | Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *flow = ControlFlow::ExitWithCode(shared.code.load(Ordering::Relaxed) as i32),
            _ => {}
        }
    })
}

fn fatal_before_ui(e: &SetupError, lang: Lang) -> u32 {
    logging::error(&e.to_string());
    platform::error_box(&crate::i18n::fatal(lang, &e.to_string()));
    e.exit_code()
}

#[derive(Deserialize)]
struct Msg {
    c: String,
    #[serde(default)]
    opts: Option<Options>,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    purge: bool,
    #[serde(default)]
    set_default: bool,
    #[serde(default)]
    current: Option<PathBuf>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    text: Option<String>,
}

fn eval(proxy: &EventLoopProxy<Ev>, name: &str, args: &[Value]) {
    let _ = proxy.send_event(Ev::Eval(js_call(name, args)));
}

fn handle_message(body: &str, shared: &Arc<Shared>, proxy: &EventLoopProxy<Ev>, init: &Value, lang: Lang) {
    let Ok(msg) = serde_json::from_str::<Msg>(body) else {
        logging::warn(&format!("mensaje de la interfaz no válido: {body}"));
        return;
    };
    match msg.c.as_str() {
        "ready" => eval(proxy, "init", std::slice::from_ref(init)),
        "drag" => {
            let _ = proxy.send_event(Ev::Drag);
        }
        "minimize" => {
            let _ = proxy.send_event(Ev::Minimize);
        }
        "quit" => {
            let _ = proxy.send_event(Ev::Quit);
        }
        "cancel" => {
            shared.cancel.store(true, Ordering::Relaxed);
            // Si no hay instalación en curso, salir; si la hay, el motor devuelve «cancelado» y se sale.
            if !INSTALLING.load(Ordering::Relaxed) {
                let _ = proxy.send_event(Ev::Quit);
            }
        }
        "open_url" => {
            if let Some(u) = msg.url.filter(|u| u.starts_with(ALLOWED_URL)) {
                platform::system().open_url(&u);
            }
        }
        "copy" => {
            if let Some(t) = msg.text {
                copy_to_clipboard(&t);
            }
        }
        "pick_folder" => {
            let start = msg.current.unwrap_or_default();
            let mut picked = pick_folder(&start);
            if let Some(p) = &mut picked
                && p.file_name().is_some_and(|n| !n.eq_ignore_ascii_case("Pliego"))
            {
                p.push("Pliego");
            }
            eval(proxy, "folder", &[json!(picked)]);
        }
        "close_pliego" => {
            let (shared, proxy) = (shared.clone(), proxy.clone());
            let root = shared.root.lock().map(|r| r.clone()).unwrap_or_default();
            std::thread::spawn(move || {
                let sys = platform::system();
                sys.close_pliego(&root, 15_000);
                eval(&proxy, "running", &[json!(sys.pliego_running(&root))]);
            });
        }
        "install" => {
            if let Some(opts) = msg.opts {
                start_install(opts, msg.kind.unwrap_or_else(|| "install".into()), shared, proxy, lang);
            }
        }
        "uninstall" => start_uninstall(msg.purge, shared, proxy),
        "open_pliego" => {
            let root = shared.root.lock().map(|r| r.clone()).unwrap_or_default();
            open_pliego(&root, msg.set_default);
            let _ = proxy.send_event(Ev::Quit);
        }
        other => logging::warn(&format!("orden desconocida: {other}")),
    }
}

static INSTALLING: AtomicBool = AtomicBool::new(false);

fn start_install(opts: Options, kind: String, shared: &Arc<Shared>, proxy: &EventLoopProxy<Ev>, lang: Lang) {
    if opts.scope == Scope::Machine && !platform::system().is_admin() {
        // Hace falta UAC: se relanza elevado y se empieza directamente.
        if relaunch_elevated(&elevated_args(&opts, lang)) {
            shared.code.store(exit::OK, Ordering::Relaxed);
            let _ = proxy.send_event(Ev::Quit);
        } else {
            eval(
                proxy,
                "error",
                &[error_json(&SetupError::Permission(
                    "se canceló la petición de administrador".into(),
                ))],
            );
        }
        return;
    }
    if let Ok(mut r) = shared.root.lock() {
        *r = opts.dir.clone();
    }
    shared.cancel.store(false, Ordering::Relaxed);
    INSTALLING.store(true, Ordering::Relaxed);
    let (shared, proxy) = (shared.clone(), proxy.clone());
    std::thread::spawn(move || {
        let sys = platform::system();
        let result = Source::embedded(std::env::current_exe().unwrap_or_default())
            .and_then(|s| s.ok_or_else(|| SetupError::Corrupt("el instalador no contiene el paquete".into())))
            .and_then(|src| {
                engine::install(&src, &opts, sys.as_ref(), &shared.cancel, &mut |p| {
                    eval(&proxy, "progress", &[json!(p.step), json!(p.percent)]);
                })
            });
        INSTALLING.store(false, Ordering::Relaxed);
        match result {
            Ok(_) => {
                shared.code.store(exit::OK, Ordering::Relaxed);
                eval(&proxy, "done", &[json!(kind)]);
            }
            Err(SetupError::Cancelled) => {
                shared.code.store(exit::CANCELLED, Ordering::Relaxed);
                let _ = proxy.send_event(Ev::Quit);
            }
            Err(e) => {
                logging::error(&format!("instalación: {e}"));
                shared.code.store(e.exit_code(), Ordering::Relaxed);
                eval(&proxy, "error", &[error_json(&e)]);
            }
        }
    });
}

fn start_uninstall(purge: bool, shared: &Arc<Shared>, proxy: &EventLoopProxy<Ev>) {
    let (shared, proxy) = (shared.clone(), proxy.clone());
    std::thread::spawn(move || {
        let root = shared.root.lock().map(|r| r.clone()).unwrap_or_default();
        let scope = shared.scope.lock().map(|s| *s).unwrap_or(Scope::User);
        let sys = platform::system();
        if scope == Scope::Machine && !sys.is_admin() {
            let args = vec!["--uninstall".to_string(), "--root".into(), root.display().to_string()];
            if relaunch_elevated(&args) {
                shared.code.store(exit::OK, Ordering::Relaxed);
                let _ = proxy.send_event(Ev::Quit);
            } else {
                eval(
                    &proxy,
                    "error",
                    &[error_json(&SetupError::Permission(
                        "se canceló la petición de administrador".into(),
                    ))],
                );
            }
            return;
        }
        if std::env::current_exe().is_ok_and(|e| e.starts_with(&root)) {
            platform::spawn_cleanup(&root);
        }
        let r = engine::uninstall(&root, scope, purge, &shared.purge_data, sys.as_ref(), &mut |p| {
            eval(&proxy, "progress", &[json!(p.step), json!(p.percent)]);
        });
        match r {
            Ok(()) => {
                shared.code.store(exit::OK, Ordering::Relaxed);
                eval(&proxy, "done", &[json!("uninstall")]);
            }
            Err(e) => {
                logging::error(&format!("desinstalación: {e}"));
                shared.code.store(e.exit_code(), Ordering::Relaxed);
                eval(&proxy, "error", &[error_json(&e)]);
            }
        }
    });
}

fn open_pliego(root: &Path, set_default: bool) {
    use std::os::windows::process::CommandExt;
    let launcher = root.join("Pliego.exe");
    let _ = std::process::Command::new(launcher)
        .current_dir(root)
        .creation_flags(0x0000_0008)
        .spawn();
    if set_default {
        platform::system().open_default_apps();
    }
}

fn copy_to_clipboard(text: &str) {
    use std::io::Write;
    use std::os::windows::process::CommandExt;
    // `clip.exe` lee UTF-16 con BOM cuando recibe bytes UTF-16LE.
    if let Ok(mut child) = std::process::Command::new("clip")
        .stdin(std::process::Stdio::piped())
        .creation_flags(0x0800_0000)
        .spawn()
    {
        if let Some(mut stdin) = child.stdin.take() {
            let mut bytes = vec![0xFF, 0xFE];
            bytes.extend(text.encode_utf16().flat_map(|u| u.to_le_bytes()));
            let _ = stdin.write_all(&bytes);
        }
        let _ = child.wait();
    }
}
