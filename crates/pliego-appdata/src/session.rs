//! Sesión de la app: qué documentos había abiertos y si la última vez Pliego se cerró bien.
//!
//! - `session.json` guarda las pestañas abiertas (ruta y página), la interfaz lo actualiza al abrir, cerrar o cambiar de
//!   pestaña. Nunca guarda contraseñas.
//! - `running` es una marca que existe mientras la app está abierta: se crea al arrancar y se borra al salir bien. Si al
//!   arrancar ya estaba, la vez anterior Pliego se cerró de golpe (fallo, corte de luz, proceso matado) y la interfaz
//!   ofrece reabrir las pestañas y copiar un informe.
//!
//! Todo se queda en el equipo: el informe solo se copia al portapapeles si la persona lo pide.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Máximo de pestañas que se recuerdan.
pub const MAX_SESSION_DOCS: usize = 50;
/// Líneas del registro que van en el informe de fallo.
pub const REPORT_LOG_LINES: usize = 60;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionDoc {
    pub path: String,
    /// Página visible (desde 0).
    #[serde(default)]
    pub page: u32,
}

/// Lo que dejó la ejecución anterior.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct PreviousRun {
    /// `true` si la app no llegó a cerrarse bien.
    pub crashed: bool,
    /// Pestañas abiertas al final (solo las que siguen existiendo en disco).
    pub docs: Vec<SessionDoc>,
}

pub struct SessionStore {
    dir: PathBuf,
}

impl SessionStore {
    pub fn new(dir: &Path) -> Self {
        Self { dir: dir.to_path_buf() }
    }

    fn session_path(&self) -> PathBuf {
        self.dir.join("session.json")
    }

    fn marker_path(&self) -> PathBuf {
        self.dir.join("running")
    }

    /// Arranque: lee cómo acabó la vez anterior y deja la marca de «en marcha». Llamar una sola vez por proceso.
    pub fn start(&self) -> PreviousRun {
        let crashed = self.marker_path().exists();
        let docs = self
            .read()
            .into_iter()
            .filter(|d| Path::new(&d.path).is_file())
            .collect();
        let _ = std::fs::create_dir_all(&self.dir);
        let _ = std::fs::write(self.marker_path(), std::process::id().to_string());
        PreviousRun { crashed, docs }
    }

    /// Salida normal: quita la marca. Las pestañas se conservan para quien quiera reabrirlas.
    pub fn clean_exit(&self) {
        let _ = std::fs::remove_file(self.marker_path());
    }

    fn read(&self) -> Vec<SessionDoc> {
        std::fs::read_to_string(self.session_path())
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    /// Guarda las pestañas abiertas (sin duplicados, como mucho [`MAX_SESSION_DOCS`]).
    pub fn save(&self, docs: &[SessionDoc]) -> Result<(), String> {
        let mut seen = std::collections::HashSet::new();
        let list: Vec<&SessionDoc> = docs
            .iter()
            .filter(|d| !d.path.is_empty() && seen.insert(d.path.as_str()))
            .take(MAX_SESSION_DOCS)
            .collect();
        let text = serde_json::to_vec_pretty(&list).map_err(|e| e.to_string())?;
        crate::write_atomic(&self.session_path(), &text).map_err(|e| e.to_string())
    }
}

/// Informe para pegar en un correo o una incidencia: versión, sistema y las últimas líneas del registro (que ya no
/// llevan contenidos ni contraseñas, y cambian la carpeta personal por `~`).
pub fn crash_report(data_dir: &Path, version: &str) -> String {
    let log = pliego_core::log::logs_dir(data_dir).join("pliego.log");
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    let lines: Vec<&str> = text.lines().collect();
    let tail = &lines[lines.len().saturating_sub(REPORT_LOG_LINES)..];
    let mut out = format!(
        "Pliego {version} · {} {}\nÚltimas líneas del registro ({}):\n",
        std::env::consts::OS,
        std::env::consts::ARCH,
        tail.len()
    );
    for l in tail {
        out.push_str(l);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(path: &Path, page: u32) -> SessionDoc {
        SessionDoc {
            path: path.to_string_lossy().into_owned(),
            page,
        }
    }

    #[test]
    fn clean_exit_is_not_a_crash() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        assert!(!store.start().crashed, "primer arranque");
        store.clean_exit();
        assert!(!store.start().crashed);
    }

    #[test]
    fn missing_clean_exit_is_a_crash_and_keeps_the_tabs() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.pdf");
        let b = dir.path().join("b.pdf");
        std::fs::write(&a, b"%PDF").unwrap();
        std::fs::write(&b, b"%PDF").unwrap();
        let store = SessionStore::new(dir.path());
        store.start();
        store.save(&[doc(&a, 3), doc(&b, 0), doc(&a, 9)]).unwrap();
        std::fs::remove_file(&b).unwrap(); // se borró mientras tanto
        // Sin clean_exit: la app murió.
        let prev = SessionStore::new(dir.path()).start();
        assert!(prev.crashed);
        assert_eq!(
            prev.docs,
            vec![doc(&a, 3)],
            "sin duplicados y sin archivos que ya no existen"
        );
    }

    #[test]
    fn report_has_version_and_last_log_lines() {
        let dir = tempfile::tempdir().unwrap();
        let logs = pliego_core::log::logs_dir(dir.path());
        std::fs::create_dir_all(&logs).unwrap();
        let lines: Vec<String> = (0..100).map(|i| format!("línea {i}")).collect();
        std::fs::write(logs.join("pliego.log"), lines.join("\n")).unwrap();
        let r = crash_report(dir.path(), "1.0.0");
        assert!(r.starts_with("Pliego 1.0.0"));
        assert!(r.contains("línea 99") && r.contains("línea 40") && !r.contains("línea 39\n"));
        // Sin registro también sale algo útil.
        assert!(crash_report(&dir.path().join("nada"), "1.0.0").contains("(0)"));
    }
}
