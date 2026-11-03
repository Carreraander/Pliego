//! Disposición en disco: `current`, `pending`, `previous`, `app-<v>\` y `healthy`.
//!
//! ```text
//! <raíz>\
//!   current        "0.4.0"          versión activa
//!   pending        "0.5.0"          versión preparada, se activa en el siguiente arranque
//!   previous       "0.3.2"          versión a la que volver si la actual no arranca
//!   skipped        una versión por línea: no se vuelven a ofrecer
//!   rolled-back    "0.5.0"          última versión descartada (la app se lo cuenta al usuario)
//!   app-0.4.0\     healthy, launch-failures, pliego-app.exe, files.json…
//! ```

use crate::error::{Error, Result};
use crate::version::{cmp_versions, is_valid_version};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// La app escribe `healthy` tras este tiempo funcionando; si termina antes, el arranque cuenta como fallido.
pub const HEALTHY_AFTER: Duration = Duration::from_secs(10);
/// Arranques fallidos seguidos de una versión sin `healthy` tras los cuales se vuelve a la anterior.
pub const MAX_FAILED_STARTS: u32 = 2;

/// Qué debe ejecutar el lanzador.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchPlan {
    pub version: String,
    pub dir: PathBuf,
    /// Versión que se acaba de activar desde `pending`.
    pub activated: Option<String>,
    /// Versión descartada en este arranque por no haber arrancado.
    pub rolled_back_from: Option<String>,
}

/// Carpeta de instalación de Pliego.
#[derive(Debug, Clone)]
pub struct Layout {
    root: PathBuf,
}

impl Layout {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Layout { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn app_dir(&self, version: &str) -> PathBuf {
        self.root.join(format!("app-{version}"))
    }

    fn read_version_file(&self, name: &str) -> Result<Option<String>> {
        match fs::read_to_string(self.root.join(name)) {
            Ok(s) => {
                let v = s.trim().to_string();
                Ok(if is_valid_version(&v) { Some(v) } else { None })
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(Error::io(format!("no se pudo leer «{name}»"), e)),
        }
    }

    /// Escribe un archivo de texto de forma atómica (temporal + renombrar).
    pub fn write_atomic(&self, name: &str, content: &str) -> Result<()> {
        fs::create_dir_all(&self.root).map_err(|e| Error::io("no se pudo crear la carpeta de instalación", e))?;
        let tmp = self.root.join(format!("{name}.tmp"));
        fs::write(&tmp, content).map_err(|e| Error::io(format!("no se pudo escribir «{name}»"), e))?;
        fs::rename(&tmp, self.root.join(name)).map_err(|e| Error::io(format!("no se pudo actualizar «{name}»"), e))
    }

    fn remove_file(&self, name: &str) -> Result<()> {
        match fs::remove_file(self.root.join(name)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(Error::io(format!("no se pudo borrar «{name}»"), e)),
        }
    }

    pub fn current(&self) -> Result<Option<String>> {
        self.read_version_file("current")
    }
    pub fn pending(&self) -> Result<Option<String>> {
        self.read_version_file("pending")
    }
    pub fn previous(&self) -> Result<Option<String>> {
        self.read_version_file("previous")
    }
    /// Última versión descartada por no arrancar.
    pub fn rolled_back(&self) -> Result<Option<String>> {
        self.read_version_file("rolled-back")
    }

    /// Versiones marcadas como omitidas.
    pub fn skipped(&self) -> Vec<String> {
        fs::read_to_string(self.root.join("skipped"))
            .map(|s| {
                s.lines()
                    .map(str::trim)
                    .filter(|l| is_valid_version(l))
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Fija la versión activa (instalación inicial o reparación).
    pub fn set_current(&self, version: &str) -> Result<()> {
        check_version(version)?;
        if !self.app_dir(version).is_dir() {
            return Err(Error::MissingVersion(self.app_dir(version)));
        }
        self.write_atomic("current", version)
    }

    /// Deja `app-<version>` como versión preparada (`pending`). La carpeta ya debe existir y estar verificada.
    pub fn set_pending(&self, version: &str) -> Result<()> {
        check_version(version)?;
        if !self.app_dir(version).is_dir() {
            return Err(Error::MissingVersion(self.app_dir(version)));
        }
        self.write_atomic("pending", version)
    }

    /// Si hay una versión preparada, pasa a ser la actual (atómico). Devuelve la versión activada.
    pub fn activate_pending(&self) -> Result<Option<String>> {
        let Some(pending) = self.pending()? else {
            // `pending` ilegible o vacío: se descarta para no volver a intentarlo.
            self.remove_file("pending")?;
            return Ok(None);
        };
        let current = self.current()?;
        if current.as_deref() == Some(pending.as_str()) || !self.app_dir(&pending).is_dir() {
            self.remove_file("pending")?;
            return Ok(None);
        }
        self.activate(&pending)?;
        Ok(Some(pending))
    }

    /// Activa `app-<version>` ya extraída (instalación, reparación o actualización directa): la actual pasa a
    /// `previous`, se limpia `pending` y se reinicia el contador de arranques fallidos.
    pub fn activate(&self, version: &str) -> Result<()> {
        check_version(version)?;
        if !self.app_dir(version).is_dir() {
            return Err(Error::MissingVersion(self.app_dir(version)));
        }
        if let Some(cur) = self.current()?
            && cur != version
        {
            self.write_atomic("previous", &cur)?;
        }
        let _ = fs::remove_file(self.app_dir(version).join("launch-failures"));
        self.write_atomic("current", version)?;
        if self.pending()?.as_deref() == Some(version) {
            self.remove_file("pending")?;
        }
        Ok(())
    }

    /// Vuelve a la versión anterior y marca la actual como omitida. Devuelve la versión restaurada.
    pub fn rollback(&self) -> Result<String> {
        let current = self.current()?;
        let previous = self.previous()?.ok_or(Error::NoPrevious)?;
        if !self.app_dir(&previous).is_dir() {
            return Err(Error::NoPrevious);
        }
        if let Some(cur) = current {
            let mut skipped = self.skipped();
            if !skipped.contains(&cur) {
                skipped.push(cur.clone());
            }
            self.write_atomic("skipped", &skipped.join("\n"))?;
            self.write_atomic("rolled-back", &cur)?;
        }
        self.write_atomic("current", &previous)?;
        self.remove_file("previous")?;
        Ok(previous)
    }

    /// Marca la versión como sana (lo hace la app tras [`HEALTHY_AFTER`] funcionando).
    pub fn mark_healthy(&self, version: &str) -> Result<()> {
        check_version(version)?;
        let dir = self.app_dir(version);
        fs::write(dir.join("healthy"), b"ok").map_err(|e| Error::io("no se pudo escribir «healthy»", e))?;
        let _ = fs::remove_file(dir.join("launch-failures"));
        Ok(())
    }

    pub fn is_healthy(&self, version: &str) -> bool {
        self.app_dir(version).join("healthy").is_file()
    }

    fn failed_starts(&self, version: &str) -> u32 {
        fs::read_to_string(self.app_dir(version).join("launch-failures"))
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0)
    }

    /// Paso previo a ejecutar: activa `pending` y aplica la regla de salud.
    /// `None` si no hay ninguna versión instalada.
    pub fn prepare_launch(&self) -> Result<Option<LaunchPlan>> {
        let activated = self.activate_pending()?;
        let Some(mut version) = self.current()? else {
            return Ok(None);
        };
        let mut rolled_back_from = None;
        if !self.is_healthy(&version) && self.failed_starts(&version) >= MAX_FAILED_STARTS {
            match self.rollback() {
                Ok(prev) => {
                    rolled_back_from = Some(version);
                    version = prev;
                }
                // Sin versión anterior no hay a dónde volver: se sigue intentando con la actual.
                Err(Error::NoPrevious) => {}
                Err(e) => return Err(e),
            }
        }
        let dir = self.app_dir(&version);
        if !dir.is_dir() {
            return Err(Error::MissingVersion(dir));
        }
        Ok(Some(LaunchPlan {
            version,
            dir,
            activated,
            rolled_back_from,
        }))
    }

    /// Informa de que la versión terminó tras `ran_for`. Si no era sana y duró menos de [`HEALTHY_AFTER`],
    /// cuenta como arranque fallido.
    pub fn report_exit(&self, version: &str, ran_for: Duration) -> Result<()> {
        check_version(version)?;
        if self.is_healthy(version) || ran_for >= HEALTHY_AFTER {
            return Ok(());
        }
        let n = self.failed_starts(version) + 1;
        fs::write(self.app_dir(version).join("launch-failures"), n.to_string())
            .map_err(|e| Error::io("no se pudo anotar el arranque fallido", e))
    }

    /// Versiones instaladas (`app-<v>`), de más nueva a más antigua.
    pub fn installed_versions(&self) -> Vec<String> {
        let mut out: Vec<String> = fs::read_dir(&self.root)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().is_dir())
            .filter_map(|e| e.file_name().to_str()?.strip_prefix("app-").map(String::from))
            .filter(|v| is_valid_version(v) && !v.ends_with(".partial"))
            .collect();
        out.sort_by(|a, b| cmp_versions(b, a));
        out
    }

    /// Borra carpetas antiguas y restos `.partial`. Conserva `keep` versiones (la actual, la anterior y las más
    /// nuevas) y siempre la actual y la pendiente.
    pub fn cleanup(&self, keep: usize) -> Result<Vec<String>> {
        let current = self.current()?;
        let pending = self.pending()?;
        let mut kept: Vec<String> = current.into_iter().chain(self.previous()?).collect();
        kept.dedup();
        let installed = self.installed_versions();
        for v in &installed {
            if kept.len() >= keep {
                break;
            }
            if !kept.contains(v) {
                kept.push(v.clone());
            }
        }
        let mut removed = Vec::new();
        for v in installed {
            if kept.contains(&v) || pending.as_deref() == Some(v.as_str()) {
                continue;
            }
            fs::remove_dir_all(self.app_dir(&v)).map_err(|e| Error::io(format!("no se pudo borrar app-{v}"), e))?;
            removed.push(v);
        }
        for e in fs::read_dir(&self.root).into_iter().flatten().flatten() {
            if e.file_name().to_string_lossy().ends_with(".partial") {
                let _ = fs::remove_dir_all(e.path());
            }
        }
        Ok(removed)
    }
}

pub(crate) fn check_version(v: &str) -> Result<()> {
    if is_valid_version(v) {
        Ok(())
    } else {
        Err(Error::InvalidVersion(v.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn install(l: &Layout, v: &str) {
        fs::create_dir_all(l.app_dir(v)).unwrap();
        fs::write(l.app_dir(v).join("pliego-app.exe"), v).unwrap();
    }

    fn setup() -> (tempfile::TempDir, Layout) {
        let d = tempfile::tempdir().unwrap();
        let l = Layout::new(d.path());
        (d, l)
    }

    #[test]
    fn activates_pending_atomically_and_keeps_previous() {
        let (_d, l) = setup();
        install(&l, "0.4.0");
        l.set_current("0.4.0").unwrap();
        install(&l, "0.5.0");
        l.set_pending("0.5.0").unwrap();
        assert_eq!(l.activate_pending().unwrap().as_deref(), Some("0.5.0"));
        assert_eq!(l.current().unwrap().as_deref(), Some("0.5.0"));
        assert_eq!(l.previous().unwrap().as_deref(), Some("0.4.0"));
        assert_eq!(l.pending().unwrap(), None);
        assert_eq!(l.activate_pending().unwrap(), None);
        assert!(!l.root().join("current.tmp").exists());
    }

    #[test]
    fn pending_without_folder_is_discarded() {
        let (_d, l) = setup();
        install(&l, "0.4.0");
        l.set_current("0.4.0").unwrap();
        l.write_atomic("pending", "9.9.9").unwrap();
        assert_eq!(l.activate_pending().unwrap(), None);
        assert_eq!(l.pending().unwrap(), None);
        assert_eq!(l.current().unwrap().as_deref(), Some("0.4.0"));
    }

    #[test]
    fn rejects_unsafe_version_names() {
        let (_d, l) = setup();
        assert!(matches!(l.set_pending("../x"), Err(Error::InvalidVersion(_))));
        l.write_atomic("current", "..\\evil").unwrap();
        assert_eq!(l.current().unwrap(), None);
    }

    #[test]
    fn rollback_restores_previous_and_skips_bad_one() {
        let (_d, l) = setup();
        install(&l, "0.4.0");
        l.set_current("0.4.0").unwrap();
        install(&l, "0.5.0");
        l.set_pending("0.5.0").unwrap();
        l.activate_pending().unwrap();
        assert_eq!(l.rollback().unwrap(), "0.4.0");
        assert_eq!(l.current().unwrap().as_deref(), Some("0.4.0"));
        assert_eq!(l.skipped(), vec!["0.5.0"]);
        assert_eq!(l.rolled_back().unwrap().as_deref(), Some("0.5.0"));
        assert!(matches!(l.rollback(), Err(Error::NoPrevious)));
    }

    /// Una versión que falla al arrancar dos veces seguidas se descarta en el tercer intento.
    #[test]
    fn health_rule_rolls_back_after_two_failed_starts() {
        let (_d, l) = setup();
        install(&l, "0.4.0");
        l.set_current("0.4.0").unwrap();
        l.mark_healthy("0.4.0").unwrap();
        install(&l, "0.5.0"); // el «pliego-app.exe» falso de esta versión falla al arrancar
        l.set_pending("0.5.0").unwrap();

        let p = l.prepare_launch().unwrap().unwrap();
        assert_eq!((p.version.as_str(), p.activated.as_deref()), ("0.5.0", Some("0.5.0")));
        l.report_exit("0.5.0", Duration::from_millis(200)).unwrap();

        let p = l.prepare_launch().unwrap().unwrap();
        assert_eq!(p.version, "0.5.0");
        l.report_exit("0.5.0", Duration::from_millis(200)).unwrap();

        let p = l.prepare_launch().unwrap().unwrap();
        assert_eq!(p.version, "0.4.0");
        assert_eq!(p.rolled_back_from.as_deref(), Some("0.5.0"));
        assert_eq!(l.skipped(), vec!["0.5.0"]);
        // Y no vuelve a saltar: 0.4.0 es sana.
        assert_eq!(l.prepare_launch().unwrap().unwrap().rolled_back_from, None);
    }

    #[test]
    fn healthy_version_never_rolls_back() {
        let (_d, l) = setup();
        install(&l, "0.4.0");
        l.set_current("0.4.0").unwrap();
        install(&l, "0.5.0");
        l.set_pending("0.5.0").unwrap();
        l.prepare_launch().unwrap();
        l.mark_healthy("0.5.0").unwrap();
        for _ in 0..5 {
            l.report_exit("0.5.0", Duration::from_millis(50)).unwrap();
        }
        assert_eq!(l.prepare_launch().unwrap().unwrap().version, "0.5.0");
    }

    #[test]
    fn long_run_does_not_count_as_failure() {
        let (_d, l) = setup();
        install(&l, "0.4.0");
        l.set_current("0.4.0").unwrap();
        for _ in 0..4 {
            l.report_exit("0.4.0", Duration::from_secs(30)).unwrap();
        }
        assert_eq!(l.failed_starts("0.4.0"), 0);
    }

    #[test]
    fn without_previous_keeps_trying_current() {
        let (_d, l) = setup();
        install(&l, "0.4.0");
        l.set_current("0.4.0").unwrap();
        for _ in 0..3 {
            l.report_exit("0.4.0", Duration::from_millis(10)).unwrap();
        }
        let p = l.prepare_launch().unwrap().unwrap();
        assert_eq!((p.version.as_str(), p.rolled_back_from), ("0.4.0", None));
    }

    #[test]
    fn cleanup_keeps_current_previous_and_pending() {
        let (_d, l) = setup();
        for v in ["0.1.0", "0.2.0", "0.3.0", "0.4.0", "0.5.0"] {
            install(&l, v);
        }
        fs::create_dir_all(l.root().join("app-0.6.0.partial")).unwrap();
        l.set_current("0.3.0").unwrap();
        l.write_atomic("previous", "0.2.0").unwrap();
        l.set_pending("0.5.0").unwrap();
        let mut removed = l.cleanup(2).unwrap();
        removed.sort();
        assert_eq!(removed, vec!["0.1.0", "0.4.0"]);
        assert_eq!(l.installed_versions(), vec!["0.5.0", "0.3.0", "0.2.0"]);
        assert!(!l.root().join("app-0.6.0.partial").exists());
    }
}
