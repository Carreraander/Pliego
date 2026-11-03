//! Lo que el instalador necesita del sistema operativo. En Windows lo implementa `platform::windows`; en las
//! pruebas, un doble que lo registra todo.

use crate::model::{Options, Result, Scope};
use std::path::{Path, PathBuf};

/// Datos para la entrada de «Aplicaciones instaladas».
#[derive(Debug, Clone)]
pub struct UninstallEntry {
    pub version: String,
    pub root: PathBuf,
    pub launcher: PathBuf,
    pub uninstaller: PathBuf,
    pub size_kb: u32,
}

pub trait System {
    fn is_admin(&self) -> bool;
    /// Compilación de Windows (p. ej. 19045); 0 si no se sabe.
    fn windows_build(&self) -> u32;
    /// Bytes libres en la unidad que contiene `path` (o su primer ancestro existente).
    fn free_space(&self, path: &Path) -> Option<u64>;

    /// ¿Hay un `pliego-app.exe` de esta instalación en marcha?
    fn pliego_running(&self, root: &Path) -> bool;
    /// Pide a Pliego que se cierre (ofrece guardar) y espera hasta `timeout_ms`. `true` si terminó.
    fn close_pliego(&self, root: &Path, timeout_ms: u32) -> bool;

    fn register_uninstall(&self, scope: Scope, entry: &UninstallEntry) -> Result<()>;
    fn unregister_uninstall(&self, scope: Scope) -> Result<()>;
    fn register_file_association(&self, scope: Scope, launcher: &Path) -> Result<()>;
    fn unregister_file_association(&self, scope: Scope) -> Result<()>;
    /// Crea los accesos directos (menú Inicio siempre; escritorio opcional) y devuelve sus rutas.
    fn create_shortcuts(&self, scope: Scope, launcher: &Path, desktop: bool) -> Result<Vec<PathBuf>>;
    fn path_add(&self, scope: Scope, dir: &Path) -> Result<()>;
    fn path_remove(&self, scope: Scope, dir: &Path) -> Result<()>;
    /// Llama a `pliego-shell register|unregister` de la versión `app_dir` si existe.
    fn shell_extensions(&self, app_dir: &Path, register: bool, scope: Scope) -> Result<()>;
    /// `SHChangeNotify(SHCNE_ASSOCCHANGED)`.
    fn notify_assoc_changed(&self);
    /// Abre la página de aplicaciones predeterminadas de Windows.
    fn open_default_apps(&self);
    fn open_url(&self, url: &str);
}

/// Opciones efectivas de registro en el sistema para unas `Options`.
pub fn register_all(sys: &dyn System, opts: &Options, entry: &UninstallEntry, app_dir: &Path) -> Result<Vec<PathBuf>> {
    sys.register_uninstall(opts.scope, entry)?;
    if opts.assoc {
        sys.register_file_association(opts.scope, &entry.launcher)?;
    }
    if opts.context_menu {
        // Un fallo del shell (W10) no impide usar Pliego: se registra y se sigue.
        if let Err(e) = sys.shell_extensions(app_dir, true, opts.scope) {
            crate::logging::warn(&format!("pliego-shell register: {e}"));
        }
    }
    if opts.path {
        sys.path_add(opts.scope, &entry.root.join("bin"))?;
    }
    let shortcuts = sys.create_shortcuts(opts.scope, &entry.launcher, opts.desktop_shortcut)?;
    sys.notify_assoc_changed();
    Ok(shortcuts)
}

#[cfg(test)]
pub mod fake {
    use super::*;
    use std::cell::RefCell;

    /// Doble de pruebas: registra las llamadas y simula Pliego abierto, poco espacio, etc.
    #[derive(Default)]
    pub struct FakeSystem {
        pub calls: RefCell<Vec<String>>,
        pub running: RefCell<bool>,
        pub closable: bool,
        pub free: Option<u64>,
        pub build: u32,
        pub admin: bool,
    }

    impl FakeSystem {
        pub fn new() -> Self {
            FakeSystem {
                closable: true,
                free: Some(1 << 40),
                build: 22631,
                admin: false,
                ..Default::default()
            }
        }
        fn log(&self, s: impl Into<String>) {
            self.calls.borrow_mut().push(s.into());
        }
        pub fn calls(&self) -> Vec<String> {
            self.calls.borrow().clone()
        }
    }

    impl System for FakeSystem {
        fn is_admin(&self) -> bool {
            self.admin
        }
        fn windows_build(&self) -> u32 {
            self.build
        }
        fn free_space(&self, _: &Path) -> Option<u64> {
            self.free
        }
        fn pliego_running(&self, _: &Path) -> bool {
            *self.running.borrow()
        }
        fn close_pliego(&self, _: &Path, _: u32) -> bool {
            self.log("close_pliego");
            if self.closable {
                *self.running.borrow_mut() = false;
            }
            self.closable
        }
        fn register_uninstall(&self, s: Scope, e: &UninstallEntry) -> Result<()> {
            self.log(format!("register_uninstall {s:?} {}", e.version));
            Ok(())
        }
        fn unregister_uninstall(&self, s: Scope) -> Result<()> {
            self.log(format!("unregister_uninstall {s:?}"));
            Ok(())
        }
        fn register_file_association(&self, s: Scope, _: &Path) -> Result<()> {
            self.log(format!("assoc {s:?}"));
            Ok(())
        }
        fn unregister_file_association(&self, s: Scope) -> Result<()> {
            self.log(format!("unassoc {s:?}"));
            Ok(())
        }
        fn create_shortcuts(&self, _: Scope, _: &Path, desktop: bool) -> Result<Vec<PathBuf>> {
            self.log(format!("shortcuts desktop={desktop}"));
            Ok(vec![PathBuf::from("startmenu/Pliego.lnk")])
        }
        fn path_add(&self, s: Scope, d: &Path) -> Result<()> {
            self.log(format!("path_add {s:?} {}", d.display()));
            Ok(())
        }
        fn path_remove(&self, s: Scope, d: &Path) -> Result<()> {
            self.log(format!("path_remove {s:?} {}", d.display()));
            Ok(())
        }
        fn shell_extensions(&self, _: &Path, register: bool, _: Scope) -> Result<()> {
            self.log(format!("shell register={register}"));
            Ok(())
        }
        fn notify_assoc_changed(&self) {
            self.log("notify");
        }
        fn open_default_apps(&self) {
            self.log("open_default_apps");
        }
        fn open_url(&self, url: &str) {
            self.log(format!("open_url {url}"));
        }
    }
}
