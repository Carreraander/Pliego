//! Motor de instalación: independiente de la interfaz y del sistema operativo (todo lo de Windows pasa por
//! [`System`]). Lo usan la interfaz, el modo silencioso y las pruebas.

use crate::logging;
use crate::model::{INFO_FILE, InstallInfo, Options, Progress, Result, Scope, SetupError};
use crate::system::{System, UninstallEntry, register_all};
use pliego_install::paq::{self, Manifest};
use pliego_install::{Layout, cmp_versions};
use std::cell::Cell;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Windows 10 versión 2004.
pub const MIN_WINDOWS_BUILD: u32 = 19041;
/// Margen sobre el tamaño descomprimido para temporales y registro.
const SPACE_MARGIN: u64 = 30 * 1024 * 1024;
const CLOSE_TIMEOUT_MS: u32 = 15_000;
const CANCEL_MARK: &str = "__pliego_cancelled__";

/// Dónde está el paquete: algo que se puede abrir más de una vez (primero el manifiesto, luego todo).
pub struct Source {
    open: Box<dyn Fn() -> io::Result<Box<dyn Read>>>,
    /// Longitud del `.paq` comprimido, para calcular el avance.
    pub len: u64,
}

impl Source {
    #[allow(dead_code)] // se usa en pruebas y para instalar desde un `.paq` suelto
    pub fn from_file(path: PathBuf) -> io::Result<Source> {
        let len = fs::metadata(&path)?.len();
        Ok(Source {
            open: Box::new(move || Ok(Box::new(fs::File::open(&path)?))),
            len,
        })
    }

    /// El `.paq` pegado al final de un ejecutable (con su SHA-256 ya comprobado).
    pub fn embedded(exe: PathBuf) -> Result<Option<Source>> {
        let Some(footer) = pliego_install::footer::read_footer(&exe)? else {
            return Ok(None);
        };
        pliego_install::footer::open_embedded(&exe)?; // comprueba la suma una vez
        Ok(Some(Source {
            len: footer.len,
            open: Box::new(move || {
                pliego_install::footer::open_embedded(&exe)
                    .map_err(io::Error::other)?
                    .map(|r| Box::new(r) as Box<dyn Read>)
                    .ok_or_else(|| io::Error::other("sin paquete"))
            }),
        }))
    }

    fn open(&self) -> Result<Box<dyn Read>> {
        Ok((self.open)()?)
    }

    pub fn manifest(&self) -> Result<Manifest> {
        Ok(paq::read_manifest(self.open()?)?)
    }
}

/// Lector que cuenta los bytes comprimidos leídos.
struct Counting<R> {
    inner: R,
    read: Rc<Cell<u64>>,
}

impl<R: Read> Read for Counting<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.read.set(self.read.get() + n as u64);
        Ok(n)
    }
}

/// Relación entre lo instalado y lo que trae este instalador.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Existing {
    None,
    /// Misma versión: reparar o desinstalar.
    Same,
    /// Versión anterior: actualizar.
    Older,
    /// Versión posterior: no se baja de versión.
    Newer,
}

pub fn classify(installed: Option<&str>, packaged: &str) -> Existing {
    match installed {
        None => Existing::None,
        Some(v) => match cmp_versions(v, packaged) {
            std::cmp::Ordering::Equal => Existing::Same,
            std::cmp::Ordering::Less => Existing::Older,
            std::cmp::Ordering::Greater => Existing::Newer,
        },
    }
}

/// Carpetas de datos del usuario que se tratan al desinstalar.
#[derive(Debug, Clone)]
pub struct DataDirs {
    /// `%APPDATA%\Pliego`: ajustes y firmas.
    pub settings: PathBuf,
    /// `%LOCALAPPDATA%\Pliego`: caché y descargas.
    pub cache: PathBuf,
}

/// Subcarpetas de `%LOCALAPPDATA%\Pliego` que se borran siempre al desinstalar (se vuelven a descargar).
const REGENERABLE_CACHE: &[&str] = &["updates"];

/// Marca del tipo de instalación que lee la autoactualización.
pub const KIND_FILE: &str = "install-kind";

fn drive_of(path: &Path) -> String {
    path.components()
        .next()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn nearest_existing(path: &Path) -> &Path {
    let mut p = path;
    while !p.exists() {
        match p.parent() {
            Some(parent) => p = parent,
            None => break,
        }
    }
    p
}

/// Copia `src` sobre `dst` aunque `dst` esté en uso (se aparta el antiguo como `.old`).
pub fn replace_file(src: &Path, dst: &Path) -> io::Result<()> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = dst.with_extension("new");
    fs::copy(src, &tmp)?;
    if dst.exists() {
        let old = dst.with_extension("old");
        let _ = fs::remove_file(&old);
        if fs::rename(&tmp, dst).is_err() {
            // En Windows no se puede sobrescribir un ejecutable en marcha, pero sí renombrarlo.
            fs::rename(dst, &old)?;
            fs::rename(&tmp, dst)?;
        }
        let _ = fs::remove_file(&old);
    } else {
        fs::rename(&tmp, dst)?;
    }
    Ok(())
}

/// Comprueba que la carpeta de destino se puede usar antes de tocar nada.
fn check_dir(dir: &Path) -> Result<()> {
    let shown = dir.display();
    if dir.as_os_str().is_empty() {
        return Err(SetupError::BadPath("la ruta está vacía".into()));
    }
    if !dir.is_absolute() {
        return Err(SetupError::BadPath(format!("{shown} no es una ruta absoluta")));
    }
    if dir.is_file() {
        return Err(SetupError::BadPath(format!("{shown} es un archivo, no una carpeta")));
    }
    // Una unidad que no existe (p. ej. `Z:\Pliego`) no tiene ningún ancestro existente.
    let anchor = nearest_existing(dir);
    if !anchor.exists() {
        return Err(SetupError::BadPath(format!("la unidad de {shown} no existe")));
    }
    if anchor.is_file() {
        return Err(SetupError::BadPath(format!(
            "{} es un archivo y no puede contener {shown}",
            anchor.display()
        )));
    }
    Ok(())
}

fn check_environment(sys: &dyn System, opts: &Options, manifest: &Manifest) -> Result<()> {
    check_dir(&opts.dir)?;
    let build = sys.windows_build();
    if build != 0 && build < MIN_WINDOWS_BUILD {
        return Err(SetupError::OldWindows(build));
    }
    let needed = manifest.files.iter().map(|f| f.size).sum::<u64>() + SPACE_MARGIN;
    if let Some(free) = sys.free_space(nearest_existing(&opts.dir))
        && free < needed
    {
        return Err(SetupError::NoSpace {
            needed,
            free,
            drive: drive_of(&opts.dir),
        });
    }
    Ok(())
}

fn ensure_closed(sys: &dyn System, root: &Path) -> Result<()> {
    if sys.pliego_running(root) && !sys.close_pliego(root, CLOSE_TIMEOUT_MS) {
        return Err(SetupError::Running);
    }
    Ok(())
}

fn size_kb(m: &Manifest) -> u32 {
    (m.files.iter().map(|f| f.size).sum::<u64>() / 1024).min(u32::MAX as u64) as u32
}

pub fn read_info(root: &Path) -> Option<InstallInfo> {
    serde_json::from_slice(&fs::read(root.join(INFO_FILE)).ok()?).ok()
}

/// Versión activa (`current`). Tras una vuelta atrás del lanzador `install.json` y el registro siguen
/// nombrando la versión descartada, así que para decidir entre reparar, actualizar o no hacer nada manda esta.
pub fn active_version(root: &Path) -> Option<String> {
    Layout::new(root)
        .current()
        .ok()
        .flatten()
        .or_else(|| read_info(root).map(|i| i.version))
}

fn write_info(root: &Path, info: &InstallInfo) -> Result<()> {
    let json = serde_json::to_vec_pretty(info).map_err(|e| SetupError::Other(e.to_string()))?;
    fs::write(root.join(INFO_FILE), json)?;
    Ok(())
}

/// Instala (o repara / actualiza encima) el paquete con las opciones dadas.
pub fn install(
    src: &Source,
    opts: &Options,
    sys: &dyn System,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(Progress),
) -> Result<InstallInfo> {
    let mut report = |step: &'static str, percent: u8| progress(Progress { step, percent });
    report("checking", 1);
    let manifest = src.manifest()?;
    let version = manifest.version.clone();
    logging::info(&format!(
        "instalando {version} en {} ({:?})",
        opts.dir.display(),
        opts.scope
    ));
    check_environment(sys, opts, &manifest)?;
    ensure_closed(sys, &opts.dir)?;

    let root = &opts.dir;
    let layout = Layout::new(root);
    let fresh = layout.current().ok().flatten().is_none();
    fs::create_dir_all(root)?;
    let app_dir = layout.app_dir(&version);

    let result = (|| -> Result<InstallInfo> {
        // 1. Extraer y verificar (5 % → 70 %).
        report("copying", 5);
        let read = Rc::new(Cell::new(0u64));
        let reader = Counting {
            inner: src.open()?,
            read: read.clone(),
        };
        let total = src.len.max(1);
        let mut last = 0u8;
        struct Tick<'a, R> {
            inner: Counting<R>,
            read: Rc<Cell<u64>>,
            total: u64,
            cancel: &'a AtomicBool,
            last: &'a mut u8,
            report: &'a mut dyn FnMut(&'static str, u8),
        }
        impl<R: Read> Read for Tick<'_, R> {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                if self.cancel.load(Ordering::Relaxed) {
                    return Err(io::Error::other(CANCEL_MARK));
                }
                let n = self.inner.read(buf)?;
                let pct = 5 + (self.read.get() * 65 / self.total).min(65) as u8;
                if pct != *self.last {
                    *self.last = pct;
                    (self.report)("copying", pct);
                }
                Ok(n)
            }
        }
        paq::unpack_verified(
            Tick {
                inner: reader,
                read,
                total,
                cancel,
                last: &mut last,
                report: &mut report,
            },
            &app_dir,
        )
        .map_err(|e| {
            if e.to_string().contains(CANCEL_MARK) {
                SetupError::Cancelled
            } else {
                e.into()
            }
        })?;
        report("verifying", 72);
        let bad = paq::verify_dir(&app_dir)?;
        if !bad.is_empty() {
            return Err(SetupError::Corrupt(format!(
                "fallan {} archivos tras copiar",
                bad.len()
            )));
        }

        // 2. Lanzadores y desinstalador (sustituibles aunque estén en uso).
        report("registering", 78);
        let launcher = root.join("Pliego.exe");
        replace_file(&app_dir.join("launcher").join("Pliego.exe"), &launcher)
            .map_err(|e| SetupError::Other(format!("lanzador: {e}")))?;
        let cli = app_dir.join("launcher").join("pliego-cli.exe");
        if cli.is_file() {
            replace_file(&cli, &root.join("bin").join("pliego.exe"))?;
        }
        let uninstaller = root.join("Uninstall.exe");
        replace_file(&app_dir.join("pliego-setup.exe"), &uninstaller)
            .map_err(|e| SetupError::Other(format!("desinstalador: {e}")))?;

        // 3. Activar la versión y registrar en Windows (78 % → 95 %).
        layout.activate(&version)?;
        let entry = UninstallEntry {
            version: version.clone(),
            root: root.clone(),
            launcher,
            uninstaller,
            size_kb: size_kb(&manifest),
        };
        report("shortcuts", 88);
        let shortcuts = register_all(sys, opts, &entry, &app_dir)?;
        let info = InstallInfo {
            version: version.clone(),
            options: opts.clone(),
            shortcuts,
        };
        write_info(root, &info)?;
        // Marca para la autoactualización (`pliego-update`): `user` o `machine` (el MSI de W10 escribe `msi`).
        let kind = if opts.scope == Scope::Machine {
            "machine"
        } else {
            "user"
        };
        fs::write(root.join(KIND_FILE), kind)?;

        report("finishing", 96);
        let _ = layout.cleanup(2);
        report("finishing", 100);
        Ok(info)
    })();

    if let Err(e) = &result {
        logging::error(&format!("instalación fallida: {e}"));
        if fresh {
            // Instalación nueva a medias: se retira lo que se haya copiado.
            let _ = remove_installed_files(root);
        }
    }
    result
}

/// Borra solo lo que instala Pliego en `root` (nunca archivos ajenos) y la carpeta si queda vacía.
pub fn remove_installed_files(root: &Path) -> io::Result<()> {
    let layout = Layout::new(root);
    for v in layout.installed_versions() {
        fs::remove_dir_all(layout.app_dir(&v))?;
    }
    if let Ok(rd) = fs::read_dir(root) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with("app-") && name.ends_with(".partial") {
                let _ = fs::remove_dir_all(e.path());
            }
        }
    }
    // `Uninstall.exe` puede estar en marcha (el que desinstala): lo borra el proceso de limpieza.
    for f in ["Uninstall.exe", "Uninstall.old", "Uninstall.new"] {
        let _ = fs::remove_file(root.join(f));
    }
    for f in [
        "Pliego.exe",
        "Pliego.old",
        "Pliego.new",
        "current",
        "current.tmp",
        "pending",
        "pending.tmp",
        "previous",
        "previous.tmp",
        "skipped",
        "skipped.tmp",
        "rolled-back",
        "rolled-back.tmp",
        INFO_FILE,
        KIND_FILE,
    ] {
        match fs::remove_file(root.join(f)) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
    }
    let bin = root.join("bin");
    for f in ["pliego.exe", "pliego.old", "pliego.new"] {
        let _ = fs::remove_file(bin.join(f));
    }
    let _ = fs::remove_dir(&bin);
    match fs::remove_dir(root) {
        Ok(()) => Ok(()),
        // Si quedan archivos del usuario, se respetan.
        Err(_) => Ok(()),
    }
}

/// Desinstala. `purge` borra también los ajustes; la caché siempre se borra.
pub fn uninstall(
    root: &Path,
    scope: Scope,
    purge: bool,
    data: &DataDirs,
    sys: &dyn System,
    progress: &mut dyn FnMut(Progress),
) -> Result<()> {
    let mut report = |step: &'static str, percent: u8| progress(Progress { step, percent });
    logging::info(&format!("desinstalando {} (purge={purge})", root.display()));
    report("checking", 2);
    ensure_closed(sys, root)?;

    let layout = Layout::new(root);
    let info = read_info(root);
    report("removing", 15);
    if let Some(v) = layout.current().ok().flatten()
        && let Err(e) = sys.shell_extensions(&layout.app_dir(&v), false, scope)
    {
        logging::warn(&format!("pliego-shell unregister: {e}"));
    }
    report("removing", 30);
    sys.unregister_file_association(scope)?;
    sys.path_remove(scope, &root.join("bin"))?;
    for lnk in info.as_ref().map(|i| i.shortcuts.clone()).unwrap_or_default() {
        let _ = fs::remove_file(lnk);
    }
    sys.unregister_uninstall(scope)?;
    sys.notify_assoc_changed();
    report("removing", 55);

    remove_installed_files(root)
        .map_err(|e| SetupError::Other(format!("no se pudo borrar {}: {e}", root.display())))?;
    report("removing", 80);
    // Sin `purge` solo se borra lo regenerable (descargas de actualizaciones). `%LOCALAPPDATA%\Pliego` también guarda
    // `tessdata` (modelos de OCR), `models`, `update-state.json`…: eso solo desaparece con `--purge`.
    for sub in REGENERABLE_CACHE {
        let _ = fs::remove_dir_all(data.cache.join(sub));
    }
    if purge {
        let _ = fs::remove_dir_all(&data.cache);
        let _ = fs::remove_dir_all(&data.settings);
    }
    report("removing", 100);
    Ok(())
}

/// Tareas tras cambiar de versión (lo llama el lanzador): renueva registro, lanzadores y desinstalador.
pub fn post_update(root: &Path, sys: &dyn System) -> Result<()> {
    let layout = Layout::new(root);
    let version = layout
        .current()?
        .ok_or_else(|| SetupError::Other("no hay versión activa".into()))?;
    let app_dir = layout.app_dir(&version);
    let mut info = read_info(root).ok_or_else(|| SetupError::Other("falta install.json".into()))?;
    logging::info(&format!("post-update a {version}"));
    refresh_files(root, &app_dir);
    let manifest = read_manifest_json(&app_dir)?;
    let entry = UninstallEntry {
        version: version.clone(),
        root: root.to_path_buf(),
        launcher: root.join("Pliego.exe"),
        uninstaller: root.join("Uninstall.exe"),
        size_kb: size_kb(&manifest),
    };
    info.version = version;
    info.shortcuts = register_all(sys, &info.options, &entry, &app_dir)?;
    write_info(root, &info)?;
    Ok(())
}

/// `files.json` extraído en disco es JSON plano (no un `.paq`).
fn read_manifest_json(app_dir: &Path) -> Result<Manifest> {
    let raw = fs::read(app_dir.join("files.json"))?;
    serde_json::from_slice(&raw).map_err(|e| SetupError::Corrupt(e.to_string()))
}

/// Sustituye lanzadores y desinstalador por los de la versión activa si cambiaron.
fn refresh_files(root: &Path, app_dir: &Path) {
    let pairs = [
        (app_dir.join("launcher").join("Pliego.exe"), root.join("Pliego.exe")),
        (
            app_dir.join("launcher").join("pliego-cli.exe"),
            root.join("bin").join("pliego.exe"),
        ),
        (app_dir.join("pliego-setup.exe"), root.join("Uninstall.exe")),
    ];
    for (src, dst) in pairs {
        if src.is_file()
            && !same_content(&src, &dst)
            && let Err(e) = replace_file(&src, &dst)
        {
            logging::warn(&format!("no se pudo renovar {}: {e}", dst.display()));
        }
    }
}

fn same_content(a: &Path, b: &Path) -> bool {
    match (fs::metadata(a), fs::metadata(b)) {
        (Ok(x), Ok(y)) if x.len() == y.len() => fs::read(a).ok() == fs::read(b).ok(),
        _ => false,
    }
}

/// Actualización por máquina (`--update --paq --manifest`, elevado): copia el `.paq` a una carpeta protegida,
/// verifica la firma Ed25519 del manifiesto y el SHA-256 de la copia, y solo entonces la extrae y la deja como
/// `pending`. El registro (versión mostrada, lanzadores) lo renueva `post_update` cuando el lanzador la activa.
pub fn update_from_paq(
    root: &Path,
    paq_file: &Path,
    manifest: &Path,
    trusted: &[pliego_update::PublicKey],
) -> Result<String> {
    let layout = Layout::new(root);
    ensure_layout_exists(&layout)?;
    let verified = crate::verify::copy_and_verify(root, paq_file, manifest, trusted)?;
    let result = (|| {
        if let Some(current) = active_version(root)
            && cmp_versions(&verified.version, &current) != std::cmp::Ordering::Greater
        {
            return Err(SetupError::Corrupt(format!(
                "la versión {} no es posterior a la instalada ({current})",
                verified.version
            )));
        }
        let version = layout.stage(&verified.copy)?;
        if version != verified.version {
            return Err(SetupError::Corrupt(format!(
                "el paquete trae la versión {version} y el manifiesto firmado {}",
                verified.version
            )));
        }
        Ok(version)
    })();
    crate::verify::clean(root);
    if let Ok(v) = &result {
        logging::info(&format!("actualización preparada: {v}"));
    }
    result
}

fn ensure_layout_exists(layout: &Layout) -> Result<()> {
    if layout.current()?.is_none() {
        return Err(SetupError::Other(
            "no hay una instalación de Pliego en esa carpeta".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system::fake::FakeSystem;
    use crate::verify::tests::{manifest_for, write_env};
    use pliego_update::sign::ReleaseKey;

    /// Crea un `.paq` de una app falsa con lanzadores y setup.
    fn make_paq(dir: &Path, version: &str, app_marker: &str) -> PathBuf {
        let src = dir.join(format!("src-{version}"));
        fs::create_dir_all(src.join("launcher")).unwrap();
        fs::write(src.join("launcher/Pliego.exe"), "launcher-app").unwrap();
        fs::write(src.join("launcher/pliego-cli.exe"), "launcher-cli").unwrap();
        fs::write(src.join("pliego-setup.exe"), format!("setup {version}")).unwrap();
        fs::write(src.join("pliego-app.exe"), app_marker).unwrap();
        fs::write(src.join("pliego.exe"), "cli").unwrap();
        let out = dir.join(format!("pliego-{version}.paq"));
        paq::pack(&src, version, &out).unwrap();
        out
    }

    fn opts(dir: &Path) -> Options {
        Options::defaults_for(Scope::User, dir.to_path_buf())
    }

    fn data(d: &Path) -> DataDirs {
        let (s, c) = (d.join("roaming/Pliego"), d.join("local/Pliego"));
        fs::create_dir_all(&s).unwrap();
        fs::create_dir_all(&c).unwrap();
        fs::write(s.join("settings.json"), "{}").unwrap();
        fs::write(c.join("update-state.json"), "{}").unwrap();
        fs::create_dir_all(c.join("tessdata")).unwrap();
        fs::write(c.join("tessdata/spa.traineddata"), "modelo").unwrap();
        fs::create_dir_all(c.join("updates")).unwrap();
        fs::write(c.join("updates/pliego.paq"), "x").unwrap();
        DataDirs { settings: s, cache: c }
    }

    #[test]
    fn full_install_layout_and_registration() {
        let d = tempfile::tempdir().unwrap();
        let paq_file = make_paq(d.path(), "0.4.0", "app");
        let root = d.path().join("Programs/Pliego");
        let sys = FakeSystem::new();
        let mut steps = Vec::new();
        let info = install(
            &Source::from_file(paq_file).unwrap(),
            &opts(&root),
            &sys,
            &AtomicBool::new(false),
            &mut |p| steps.push(p),
        )
        .unwrap();

        assert_eq!(info.version, "0.4.0");
        assert_eq!(fs::read_to_string(root.join("current")).unwrap(), "0.4.0");
        assert_eq!(fs::read_to_string(root.join("Pliego.exe")).unwrap(), "launcher-app");
        assert_eq!(fs::read_to_string(root.join("bin/pliego.exe")).unwrap(), "launcher-cli");
        assert_eq!(fs::read_to_string(root.join("Uninstall.exe")).unwrap(), "setup 0.4.0");
        assert!(root.join("app-0.4.0/pliego-app.exe").is_file());
        assert!(root.join(INFO_FILE).is_file());
        let calls = sys.calls();
        assert!(
            calls.contains(&"register_uninstall User 0.4.0".to_string()),
            "{calls:?}"
        );
        assert!(calls.contains(&"assoc User".to_string()));
        assert!(calls.contains(&"shell register=true".to_string()));
        assert!(!calls.iter().any(|c| c.starts_with("path_add")), "PATH es opcional");
        // Progreso monótono y termina en 100.
        assert!(steps.windows(2).all(|w| w[0].percent <= w[1].percent), "{steps:?}");
        assert_eq!(steps.last().unwrap().percent, 100);
    }

    #[test]
    fn options_are_honoured() {
        let d = tempfile::tempdir().unwrap();
        let paq_file = make_paq(d.path(), "0.4.0", "app");
        let root = d.path().join("P");
        let sys = FakeSystem::new();
        let mut o = opts(&root);
        (o.assoc, o.context_menu, o.path, o.desktop_shortcut) = (false, false, true, true);
        install(
            &Source::from_file(paq_file).unwrap(),
            &o,
            &sys,
            &AtomicBool::new(false),
            &mut |_| {},
        )
        .unwrap();
        let calls = sys.calls();
        assert!(!calls.contains(&"assoc User".to_string()));
        assert!(!calls.contains(&"shell register=true".to_string()));
        assert!(
            calls
                .iter()
                .any(|c| c.starts_with("path_add User") && c.ends_with("bin"))
        );
        assert!(calls.contains(&"shortcuts desktop=true".to_string()));
    }

    #[test]
    fn update_keeps_previous_and_repair_reinstalls() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().join("P");
        let sys = FakeSystem::new();
        let p4 = Source::from_file(make_paq(d.path(), "0.4.0", "a")).unwrap();
        let p5 = Source::from_file(make_paq(d.path(), "0.5.0", "b")).unwrap();
        install(&p4, &opts(&root), &sys, &AtomicBool::new(false), &mut |_| {}).unwrap();
        install(&p5, &opts(&root), &sys, &AtomicBool::new(false), &mut |_| {}).unwrap();
        let l = Layout::new(&root);
        assert_eq!(
            (l.current().unwrap().as_deref(), l.previous().unwrap().as_deref()),
            (Some("0.5.0"), Some("0.4.0"))
        );
        // Reparar: estropear y reinstalar la misma versión.
        fs::write(root.join("app-0.5.0/pliego-app.exe"), "roto").unwrap();
        assert!(!paq::verify_dir(&root.join("app-0.5.0")).unwrap().is_empty());
        install(&p5, &opts(&root), &sys, &AtomicBool::new(false), &mut |_| {}).unwrap();
        assert!(paq::verify_dir(&root.join("app-0.5.0")).unwrap().is_empty());
        assert_eq!(l.previous().unwrap().as_deref(), Some("0.4.0"));
    }

    #[test]
    fn cancelling_stops_and_cleans_up() {
        let d = tempfile::tempdir().unwrap();
        let src = Source::from_file(make_paq(d.path(), "0.4.0", "a")).unwrap();
        let root = d.path().join("P");
        let sys = FakeSystem::new();
        let e = install(&src, &opts(&root), &sys, &AtomicBool::new(true), &mut |_| {}).unwrap_err();
        assert!(matches!(e, SetupError::Cancelled), "{e}");
        assert_eq!(e.exit_code(), crate::model::exit::CANCELLED);
        assert!(!root.exists());
    }

    #[test]
    fn classify_versions() {
        assert_eq!(classify(None, "0.5.0"), Existing::None);
        assert_eq!(classify(Some("0.5.0"), "0.5.0"), Existing::Same);
        assert_eq!(classify(Some("0.4.0"), "0.5.0"), Existing::Older);
        assert_eq!(classify(Some("0.10.0"), "0.9.0"), Existing::Newer);
    }

    #[test]
    fn active_version_follows_rollback() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().join("P");
        let sys = FakeSystem::new();
        for v in ["0.4.0", "0.5.0"] {
            let paq_file = make_paq(d.path(), v, "app");
            install(
                &Source::from_file(paq_file).unwrap(),
                &opts(&root),
                &sys,
                &AtomicBool::new(false),
                &mut |_| {},
            )
            .unwrap();
        }
        Layout::new(&root).rollback().unwrap();
        assert_eq!(read_info(&root).unwrap().version, "0.5.0");
        assert_eq!(active_version(&root).as_deref(), Some("0.4.0"));
    }

    #[test]
    fn refuses_when_no_space_old_windows_or_running() {
        let d = tempfile::tempdir().unwrap();
        let src = Source::from_file(make_paq(d.path(), "0.4.0", "a")).unwrap();
        let root = d.path().join("P");

        let mut sys = FakeSystem::new();
        sys.free = Some(1000);
        let e = install(&src, &opts(&root), &sys, &AtomicBool::new(false), &mut |_| {}).unwrap_err();
        assert!(matches!(e, SetupError::NoSpace { .. }), "{e}");
        assert!(!root.join("current").exists());

        // Rutas no válidas: vacía, relativa y bajo un archivo.
        let sys = FakeSystem::new();
        let file = d.path().join("archivo");
        fs::write(&file, b"x").unwrap();
        for bad in [
            PathBuf::new(),
            PathBuf::from("relativa/Pliego"),
            file.clone(),
            file.join("Pliego"),
        ] {
            let e = install(&src, &opts(&bad), &sys, &AtomicBool::new(false), &mut |_| {}).unwrap_err();
            assert!(matches!(e, SetupError::BadPath(_)), "{bad:?}: {e}");
        }

        let mut sys = FakeSystem::new();
        sys.build = 18363;
        assert!(matches!(
            install(&src, &opts(&root), &sys, &AtomicBool::new(false), &mut |_| {}),
            Err(SetupError::OldWindows(18363))
        ));

        let mut sys = FakeSystem::new();
        sys.closable = false;
        *sys.running.borrow_mut() = true;
        assert!(matches!(
            install(&src, &opts(&root), &sys, &AtomicBool::new(false), &mut |_| {}),
            Err(SetupError::Running)
        ));

        // Si se deja cerrar, cierra y sigue.
        let sys = FakeSystem::new();
        *sys.running.borrow_mut() = true;
        install(&src, &opts(&root), &sys, &AtomicBool::new(false), &mut |_| {}).unwrap();
        assert!(sys.calls().contains(&"close_pliego".to_string()));
    }

    #[test]
    fn corrupt_package_leaves_nothing_behind() {
        let d = tempfile::tempdir().unwrap();
        let good = make_paq(d.path(), "0.4.0", "a");
        let mut bytes = fs::read(&good).unwrap();
        let n = bytes.len();
        bytes.truncate(n - 20);
        let bad = d.path().join("bad.paq");
        fs::write(&bad, bytes).unwrap();
        let root = d.path().join("P");
        let sys = FakeSystem::new();
        let e = install(
            &Source::from_file(bad).unwrap(),
            &opts(&root),
            &sys,
            &AtomicBool::new(false),
            &mut |_| {},
        )
        .unwrap_err();
        assert!(matches!(e, SetupError::Corrupt(_)), "{e}");
        assert!(
            !root.exists(),
            "no deben quedar restos de una instalación nueva fallida"
        );
        assert!(!sys.calls().iter().any(|c| c.starts_with("register")));
    }

    #[test]
    fn uninstall_removes_everything_but_kept_settings() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().join("P");
        let sys = FakeSystem::new();
        install(
            &Source::from_file(make_paq(d.path(), "0.4.0", "a")).unwrap(),
            &opts(&root),
            &sys,
            &AtomicBool::new(false),
            &mut |_| {},
        )
        .unwrap();
        fs::write(root.join("mis-notas.txt"), "ajeno").unwrap();
        let dd = data(d.path());
        uninstall(&root, Scope::User, false, &dd, &sys, &mut |_| {}).unwrap();
        assert!(root.join("mis-notas.txt").is_file(), "no se borran archivos ajenos");
        assert!(!root.join("Pliego.exe").exists() && !root.join("app-0.4.0").exists() && !root.join("bin").exists());
        assert!(dd.settings.join("settings.json").is_file(), "los ajustes se conservan");
        assert!(
            !dd.cache.join("updates").exists(),
            "las descargas regenerables se borran"
        );
        assert!(
            dd.cache.join("tessdata/spa.traineddata").is_file(),
            "los modelos de OCR se conservan"
        );
        assert!(
            dd.cache.join("update-state.json").is_file(),
            "el estado del actualizador se conserva"
        );
        let c = sys.calls();
        assert!(
            c.contains(&"unregister_uninstall User".to_string()) && c.contains(&"shell register=false".to_string())
        );
    }

    #[test]
    fn registry_version_follows_rollback_via_post_update() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().join("P");
        let sys = FakeSystem::new();
        for v in ["0.4.0", "0.5.0"] {
            let src = Source::from_file(make_paq(d.path(), v, "a")).unwrap();
            install(&src, &opts(&root), &sys, &AtomicBool::new(false), &mut |_| {}).unwrap();
        }
        assert_eq!(read_info(&root).unwrap().version, "0.5.0");
        Layout::new(&root).rollback().unwrap();
        post_update(&root, &sys).unwrap();
        assert_eq!(read_info(&root).unwrap().version, "0.4.0");
        assert!(sys.calls().contains(&"register_uninstall User 0.4.0".to_string()));
    }

    #[test]
    fn uninstall_purge_removes_settings_and_empty_root() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().join("P");
        let sys = FakeSystem::new();
        install(
            &Source::from_file(make_paq(d.path(), "0.4.0", "a")).unwrap(),
            &opts(&root),
            &sys,
            &AtomicBool::new(false),
            &mut |_| {},
        )
        .unwrap();
        let dd = data(d.path());
        uninstall(&root, Scope::User, true, &dd, &sys, &mut |_| {}).unwrap();
        assert!(!root.exists());
        assert!(!dd.settings.exists());
    }

    #[test]
    fn post_update_renews_launcher_and_registration() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().join("P");
        let sys = FakeSystem::new();
        install(
            &Source::from_file(make_paq(d.path(), "0.4.0", "a")).unwrap(),
            &opts(&root),
            &sys,
            &AtomicBool::new(false),
            &mut |_| {},
        )
        .unwrap();
        // El actualizador deja 0.5.0 preparada (con un setup nuevo) y el lanzador la activa.
        let l = Layout::new(&root);
        l.stage(&make_paq(d.path(), "0.5.0", "b")).unwrap();
        l.activate_pending().unwrap();
        post_update(&root, &sys).unwrap();
        assert_eq!(fs::read_to_string(root.join("Uninstall.exe")).unwrap(), "setup 0.5.0");
        assert_eq!(read_info(&root).unwrap().version, "0.5.0");
        assert!(sys.calls().contains(&"register_uninstall User 0.5.0".to_string()));
    }

    #[test]
    fn update_from_paq_stages_pending() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().join("P");
        let sys = FakeSystem::new();
        install(
            &Source::from_file(make_paq(d.path(), "0.4.0", "a")).unwrap(),
            &opts(&root),
            &sys,
            &AtomicBool::new(false),
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(fs::read_to_string(root.join(KIND_FILE)).unwrap(), "user");

        let key = ReleaseKey::generate().unwrap();
        let trusted = [key.public()];
        let signed = |paq: &Path, version: &str, key: &ReleaseKey| {
            let m = manifest_for(&fs::read(paq).unwrap(), version);
            let dir = paq
                .parent()
                .unwrap()
                .join(format!("m-{version}-{}", key.public().key_id()));
            fs::create_dir_all(&dir).unwrap();
            write_env(&dir, &m, key)
        };
        let paq = make_paq(d.path(), "0.5.0", "b");

        // Firmado por una clave desconocida, o sin claves: no se extrae nada.
        let other = signed(&paq, "0.5.0", &ReleaseKey::generate().unwrap());
        assert!(update_from_paq(&root, &paq, &other, &trusted).is_err());
        assert!(update_from_paq(&root, &paq, &other, &[]).is_err());
        assert_eq!(Layout::new(&root).pending().unwrap(), None);

        let man = signed(&paq, "0.5.0", &key);
        let v = update_from_paq(&root, &paq, &man, &trusted).unwrap();
        assert_eq!(v, "0.5.0");
        assert_eq!(Layout::new(&root).pending().unwrap().as_deref(), Some("0.5.0"));
        assert!(
            !root.join(crate::verify::STAGING_DIR).exists(),
            "limpia la copia protegida"
        );

        // La versión del manifiesto firmado debe coincidir con la del paquete y ser posterior a la instalada.
        let liar = signed(&paq, "0.9.0", &key);
        assert!(update_from_paq(&root, &paq, &liar, &trusted).is_err());
        let old = make_paq(d.path(), "0.3.0", "z");
        let man_old = signed(&old, "0.3.0", &key);
        assert!(update_from_paq(&root, &old, &man_old, &trusted).is_err());
        assert!(update_from_paq(&d.path().join("vacia"), &paq, &man, &trusted).is_err());
    }

    #[test]
    fn replace_file_overwrites_and_creates() {
        let d = tempfile::tempdir().unwrap();
        let (a, b) = (d.path().join("a"), d.path().join("sub/b"));
        fs::write(&a, "1").unwrap();
        replace_file(&a, &b).unwrap();
        fs::write(&a, "2").unwrap();
        replace_file(&a, &b).unwrap();
        assert_eq!(fs::read_to_string(&b).unwrap(), "2");
        assert!(!d.path().join("sub/b.old").exists() && !d.path().join("sub/b.new").exists());
    }
}
