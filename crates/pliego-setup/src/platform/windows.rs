//! Implementación para Windows de [`System`] y utilidades del instalador.
//! Todo el código `unsafe` está aquí; cada bloque lleva su `// SAFETY:`.
#![allow(unsafe_code)]

mod reg;
mod shell;
mod webview2;

pub use shell::{delete_self_later, error_box, spawn_cleanup, wait_for_exit};
pub use webview2::ensure_webview2;

use crate::cli::UiRequest;
use crate::model::{Result, Scope, SetupError};
use crate::pathenv;
use crate::system::{System, UninstallEntry};
use reg::{Hive, Key};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub struct WinSystem;

pub fn system() -> Box<dyn System> {
    Box::new(WinSystem)
}

pub fn locale_name() -> Option<String> {
    shell::locale_name()
}

pub fn run_ui(req: UiRequest) -> u32 {
    crate::ui::run(req)
}

fn hive(scope: Scope) -> Hive {
    match scope {
        Scope::User => Hive::CurrentUser,
        Scope::Machine => Hive::LocalMachine,
    }
}

fn io<T>(r: windows::core::Result<T>, what: &str) -> Result<T> {
    r.map_err(|e| {
        if e.code().0 as u32 == 0x8007_0005 {
            SetupError::Permission(format!("{what}: {e}"))
        } else {
            SetupError::Other(format!("{what}: {e}"))
        }
    })
}

const PROG_ID: &str = "Pliego.Document";
const UNINSTALL_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Pliego";

impl System for WinSystem {
    fn is_admin(&self) -> bool {
        shell::is_elevated()
    }

    fn windows_build(&self) -> u32 {
        Key::open(Hive::LocalMachine, r"SOFTWARE\Microsoft\Windows NT\CurrentVersion")
            .and_then(|k| k.get_string("CurrentBuildNumber"))
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0)
    }

    fn free_space(&self, path: &Path) -> Option<u64> {
        shell::free_space(path)
    }

    fn pliego_running(&self, root: &Path) -> bool {
        !shell::pliego_pids(root).is_empty()
    }

    fn close_pliego(&self, root: &Path, timeout_ms: u32) -> bool {
        let deadline = Instant::now() + Duration::from_millis(timeout_ms as u64);
        for pid in shell::pliego_pids(root) {
            shell::post_close(pid);
        }
        while Instant::now() < deadline {
            if shell::pliego_pids(root).is_empty() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        shell::pliego_pids(root).is_empty()
    }

    fn register_uninstall(&self, scope: Scope, e: &UninstallEntry) -> Result<()> {
        let k = io(Key::create(hive(scope), UNINSTALL_KEY), "clave de desinstalación")?;
        let q = |p: &Path| format!("\"{}\"", p.display());
        let un = q(&e.uninstaller);
        let machine = if scope == Scope::Machine { " --per-machine" } else { "" };
        let values = [
            ("DisplayName", "Pliego".to_string()),
            ("DisplayVersion", e.version.clone()),
            ("Publisher", "Proyecto Pliego".into()),
            ("DisplayIcon", format!("{},0", q(&e.launcher))),
            ("InstallLocation", e.root.display().to_string()),
            ("UninstallString", format!("{un} --uninstall{machine}")),
            ("QuietUninstallString", format!("{un} --uninstall --silent{machine}")),
            ("URLInfoAbout", "https://github.com/Carreraander/Pliego".into()),
        ];
        for (n, v) in values {
            io(k.set_string(n, &v), n)?;
        }
        io(k.set_dword("EstimatedSize", e.size_kb), "EstimatedSize")?;
        io(k.set_dword("NoModify", 1), "NoModify")?;
        io(k.set_dword("NoRepair", 1), "NoRepair")
    }

    fn unregister_uninstall(&self, scope: Scope) -> Result<()> {
        io(Key::delete_tree(hive(scope), UNINSTALL_KEY), "clave de desinstalación")
    }

    fn register_file_association(&self, scope: Scope, launcher: &Path) -> Result<()> {
        let h = hive(scope);
        let exe = launcher.display().to_string();
        let open = format!("\"{exe}\" \"%1\"");
        let prog = format!(r"Software\Classes\{PROG_ID}");
        let k = io(Key::create(h, &prog), "ProgID")?;
        io(k.set_string("", "Documento PDF de Pliego"), "ProgID")?;
        io(
            Key::create(h, &format!(r"{prog}\DefaultIcon")).and_then(|k| k.set_string("", &format!("\"{exe}\",0"))),
            "icono",
        )?;
        io(
            Key::create(h, &format!(r"{prog}\shell\open\command")).and_then(|k| k.set_string("", &open)),
            "verbo open",
        )?;
        io(
            Key::create(h, r"Software\Classes\.pdf\OpenWithProgids").and_then(|k| k.set_empty(PROG_ID)),
            "OpenWithProgids",
        )?;
        // «Abrir con» y Aplicaciones predeterminadas.
        let app = r"Software\Classes\Applications\Pliego.exe";
        io(
            Key::create(h, app).and_then(|k| k.set_string("FriendlyAppName", "Pliego")),
            "Applications",
        )?;
        io(
            Key::create(h, &format!(r"{app}\shell\open\command")).and_then(|k| k.set_string("", &open)),
            "Applications",
        )?;
        io(
            Key::create(h, &format!(r"{app}\SupportedTypes")).and_then(|k| k.set_string(".pdf", "")),
            "SupportedTypes",
        )?;
        let cap = r"Software\Pliego\Capabilities";
        let k = io(Key::create(h, cap), "Capabilities")?;
        io(k.set_string("ApplicationName", "Pliego"), "Capabilities")?;
        io(
            k.set_string(
                "ApplicationDescription",
                "Lee, firma y organiza tus PDF. Libre y sin nube.",
            ),
            "Capabilities",
        )?;
        io(
            Key::create(h, &format!(r"{cap}\FileAssociations")).and_then(|k| k.set_string(".pdf", PROG_ID)),
            "FileAssociations",
        )?;
        io(
            Key::create(h, r"Software\RegisteredApplications").and_then(|k| k.set_string("Pliego", cap)),
            "RegisteredApplications",
        )
    }

    fn unregister_file_association(&self, scope: Scope) -> Result<()> {
        let h = hive(scope);
        for key in [
            format!(r"Software\Classes\{PROG_ID}"),
            r"Software\Classes\Applications\Pliego.exe".to_string(),
            r"Software\Pliego\Capabilities".to_string(),
        ] {
            io(Key::delete_tree(h, &key), &key)?;
        }
        if let Some(k) = Key::open_write(h, r"Software\Classes\.pdf\OpenWithProgids") {
            let _ = k.delete_value(PROG_ID);
        }
        if let Some(k) = Key::open_write(h, r"Software\RegisteredApplications") {
            let _ = k.delete_value("Pliego");
        }
        // `Software\Pliego` se borra solo si quedó vacía (la app puede guardar cosas ahí).
        Key::delete_if_empty(h, r"Software\Pliego");
        Ok(())
    }

    fn create_shortcuts(&self, scope: Scope, launcher: &Path, desktop: bool) -> Result<Vec<PathBuf>> {
        shell::create_shortcuts(scope, launcher, desktop)
            .map_err(|e| SetupError::Other(format!("accesos directos: {e}")))
    }

    fn path_add(&self, scope: Scope, dir: &Path) -> Result<()> {
        edit_path(scope, |cur| pathenv::with_dir(cur, &dir.display().to_string()))
    }

    fn path_remove(&self, scope: Scope, dir: &Path) -> Result<()> {
        edit_path(scope, |cur| pathenv::without_dir(cur, &dir.display().to_string()))
    }

    fn shell_extensions(&self, app_dir: &Path, register: bool, scope: Scope) -> Result<()> {
        let exe = app_dir.join("pliego-shell.exe");
        if !exe.is_file() {
            crate::logging::info("pliego-shell.exe no está en esta versión; se omite");
            return Ok(());
        }
        let mut cmd = Command::new(exe);
        cmd.arg(if register { "register" } else { "unregister" });
        if scope == Scope::Machine {
            cmd.arg("--per-machine");
        }
        let status = cmd
            .creation_flags(CREATE_NO_WINDOW)
            .status()
            .map_err(|e| SetupError::Other(e.to_string()))?;
        if status.success() {
            Ok(())
        } else {
            Err(SetupError::Other(format!("pliego-shell terminó con {status}")))
        }
    }

    fn notify_assoc_changed(&self) {
        shell::notify_assoc_changed();
    }

    fn open_default_apps(&self) {
        let url = if self.windows_build() >= 22000 {
            "ms-settings:defaultapps?registeredAppUser=Pliego"
        } else {
            "ms-settings:defaultapps"
        };
        shell::open(url);
    }

    fn open_url(&self, url: &str) {
        shell::open(url);
    }
}

fn edit_path(scope: Scope, f: impl Fn(&str) -> Option<String>) -> Result<()> {
    let (h, sub) = match scope {
        Scope::User => (Hive::CurrentUser, "Environment"),
        Scope::Machine => (
            Hive::LocalMachine,
            r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment",
        ),
    };
    let k = io(Key::create(h, sub), "PATH")?;
    let current = k.get_string("Path").unwrap_or_default();
    if let Some(new) = f(&current) {
        io(k.set_expand_string("Path", &new), "PATH")?;
        shell::broadcast_environment_change();
    }
    Ok(())
}

/// Versión instalada (clave de desinstalación) y su carpeta, por usuario o por máquina.
pub fn installed() -> Option<(Scope, String, PathBuf)> {
    for scope in [Scope::User, Scope::Machine] {
        if let Some(k) = Key::open(hive(scope), UNINSTALL_KEY) {
            let version = k.get_string("DisplayVersion")?;
            let dir = k.get_string("InstallLocation")?;
            return Some((scope, version, PathBuf::from(dir)));
        }
    }
    None
}

/// Elige la carpeta con el diálogo de Windows.
pub fn pick_folder(start: &Path) -> Option<PathBuf> {
    shell::pick_folder(start)
}

/// Relanza este mismo programa pidiendo permisos de administrador. `true` si se lanzó.
pub fn relaunch_elevated(args: &[String]) -> bool {
    shell::relaunch_elevated(args)
}

pub fn is_dark_theme() -> bool {
    Key::open(
        Hive::CurrentUser,
        r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
    )
    .and_then(|k| k.get_dword("AppsUseLightTheme"))
    .map(|v| v == 0)
    .unwrap_or(false)
}
