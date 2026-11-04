//! Entorno de la app en marcha: qué instalación es, qué versión, dónde guarda el estado y con qué configuración
//! trabaja el actualizador.

use std::path::{Path, PathBuf};

use semver::Version;

use crate::config::{Policy, UpdateConfig, UserSettings, build_url, resolve};
use crate::install::InstallKind;
use crate::keys::release_keys;

#[derive(Debug, Clone)]
pub struct Environment {
    /// Carpeta de instalación (la pasa el lanzador); `None` si Pliego se ejecuta suelto.
    pub root: Option<PathBuf>,
    pub kind: InstallKind,
    pub current_version: Version,
    /// Versión que el lanzador abandonó en este arranque por no arrancar.
    pub rolled_back_from: Option<Version>,
    /// `%LOCALAPPDATA%\Pliego\update-state.json`.
    pub state_path: PathBuf,
    /// `%LOCALAPPDATA%\Pliego\updates\`.
    pub downloads: PathBuf,
}

impl Environment {
    /// Lee el entorno del proceso. `app_version`: versión del binario, por si el lanzador no pasó `PLIEGO_VERSION`.
    pub fn detect(app_version: &str) -> Self {
        Self::from_vars(
            app_version,
            |k| std::env::var(k).ok().filter(|v| !v.is_empty()),
            &crate::platform::program_dirs(),
        )
    }

    pub fn from_vars(app_version: &str, var: impl Fn(&str) -> Option<String>, program_dirs: &[PathBuf]) -> Self {
        let parse = |s: String| Version::parse(s.trim()).ok();
        let current_version = var("PLIEGO_VERSION")
            .and_then(parse)
            .or_else(|| Version::parse(app_version).ok())
            .unwrap_or_else(|| Version::new(0, 0, 0));
        let root = var("PLIEGO_INSTALL_ROOT").map(PathBuf::from);
        let mark = root
            .as_deref()
            .and_then(|r| std::fs::read_to_string(r.join("install-kind")).ok());
        let kind = InstallKind::infer(root.as_deref(), mark.as_deref(), program_dirs);
        let local = var("LOCALAPPDATA")
            .map(PathBuf::from)
            .or_else(|| var("XDG_DATA_HOME").map(PathBuf::from))
            .or_else(|| var("HOME").map(|h| Path::new(&h).join(".local/share")))
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Pliego");
        Self {
            root,
            kind,
            current_version,
            rolled_back_from: var("PLIEGO_ROLLED_BACK_FROM").and_then(parse),
            state_path: local.join("update-state.json"),
            downloads: local.join("updates"),
        }
    }

    /// Configuración efectiva: directiva > `PLIEGO_UPDATE_URL` en ejecución > ajustes del usuario > URL compilada.
    pub fn config(&self, user: &UserSettings, policy: &Policy) -> UpdateConfig {
        self.config_with(
            build_url(),
            std::env::var("PLIEGO_UPDATE_URL").ok().as_deref(),
            user,
            policy,
            release_keys(),
            crate::platform::os_build(),
        )
    }

    pub fn config_with(
        &self,
        build: Option<&str>,
        runtime: Option<&str>,
        user: &UserSettings,
        policy: &Policy,
        keys: Vec<crate::PublicKey>,
        os_build: Option<u32>,
    ) -> UpdateConfig {
        let mut c = resolve(build, runtime, user, policy, self.current_version.clone(), keys);
        c.os_build = os_build;
        c
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn vars(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let m: HashMap<String, String> = pairs.iter().map(|(k, v)| ((*k).into(), (*v).into())).collect();
        move |k| m.get(k).cloned()
    }

    #[test]
    fn launcher_variables_define_the_installation() {
        let e = Environment::from_vars(
            "0.0.1",
            vars(&[
                ("PLIEGO_INSTALL_ROOT", r"C:\Users\ana\AppData\Local\Programs\Pliego"),
                ("PLIEGO_VERSION", "0.1.1"),
                ("PLIEGO_ROLLED_BACK_FROM", "0.1.2"),
                ("LOCALAPPDATA", r"C:\Users\ana\AppData\Local"),
            ]),
            &[PathBuf::from(r"C:\Program Files")],
        );
        assert_eq!(e.kind, InstallKind::PerUser);
        assert_eq!(e.current_version, Version::new(0, 1, 1));
        assert_eq!(e.rolled_back_from, Some(Version::new(0, 1, 2)));
        assert!(
            e.state_path.ends_with("Pliego/update-state.json") || e.state_path.ends_with(r"Pliego\update-state.json")
        );
    }

    #[test]
    fn without_launcher_it_is_portable_and_uses_the_binary_version() {
        let e = Environment::from_vars("0.3.0", vars(&[("HOME", "/home/ana")]), &[]);
        assert_eq!(e.kind, InstallKind::Portable);
        assert_eq!(e.current_version, Version::new(0, 3, 0));
        assert!(e.root.is_none() && e.rolled_back_from.is_none());
    }

    #[test]
    fn install_kind_mark_beats_the_folder() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("install-kind"), "msi\n").unwrap();
        let e = Environment::from_vars(
            "0.1.0",
            vars(&[("PLIEGO_INSTALL_ROOT", d.path().to_str().unwrap())]),
            &[],
        );
        assert_eq!(e.kind, InstallKind::Msi);
    }
}
