//! Registro de diagnóstico con rotación: una carpeta `logs` con pocos archivos de tamaño acotado.
//!
//! Sirve para investigar fallos en equipos de usuarios (arranque, worker, motor, actualizaciones) sin tener que
//! reproducirlos. Reglas:
//! - Solo la app de escritorio lo activa con [`init`]; la CLI y las pruebas no escriben nada (las funciones de este
//!   módulo no hacen nada mientras no se haya llamado a `init`).
//! - Nunca se registran contenidos de documentos ni contraseñas: los motores solo pasan mensajes de error y eventos.
//!   Por si un mensaje trae una ruta, la carpeta personal del usuario se sustituye por `~`.
//! - Una línea por evento: `2026-11-05T23:58:01Z WARN  worker  mensaje`. Los saltos de línea se escapan y cada línea
//!   se corta a [`MAX_LINE`] caracteres.
//! - Rotación: `pliego.log` se cierra al llegar a [`MAX_BYTES`] y pasa a `pliego.1.log`, `pliego.2.log`…; se conservan
//!   [`KEEP`] archivos en total (los más antiguos se borran), unos 2,5 MiB como máximo.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// Tamaño a partir del cual se rota el archivo activo.
pub const MAX_BYTES: u64 = 512 * 1024;
/// Archivos que se conservan (el activo más los rotados).
pub const KEEP: usize = 5;
/// Longitud máxima de una línea (en caracteres).
pub const MAX_LINE: usize = 2000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Info,
    Warn,
    Error,
}

impl Level {
    fn tag(self) -> &'static str {
        match self {
            Level::Info => "INFO ",
            Level::Warn => "WARN ",
            Level::Error => "ERROR",
        }
    }
}

/// Archivo de registro con rotación. Se usa directamente en las pruebas y a través de [`init`] en la app.
pub struct RotatingLog {
    dir: PathBuf,
    max_bytes: u64,
    keep: usize,
    home: Option<String>,
    file: Option<File>,
    size: u64,
}

impl RotatingLog {
    /// Abre (o crea) `dir/pliego.log` en modo añadir.
    pub fn open(dir: &Path, max_bytes: u64, keep: usize, home: Option<String>) -> std::io::Result<Self> {
        fs::create_dir_all(dir)?;
        let mut log = RotatingLog {
            dir: dir.to_owned(),
            max_bytes,
            keep: keep.max(1),
            home: home.filter(|h| h.len() > 3),
            file: None,
            size: 0,
        };
        log.reopen()?;
        Ok(log)
    }

    fn path(&self, n: usize) -> PathBuf {
        if n == 0 {
            self.dir.join("pliego.log")
        } else {
            self.dir.join(format!("pliego.{n}.log"))
        }
    }

    fn reopen(&mut self) -> std::io::Result<()> {
        let f = OpenOptions::new().create(true).append(true).open(self.path(0))?;
        self.size = f.metadata().map(|m| m.len()).unwrap_or(0);
        self.file = Some(f);
        Ok(())
    }

    /// Cierra el archivo activo, desplaza los anteriores y abre uno nuevo.
    fn rotate(&mut self) -> std::io::Result<()> {
        self.file = None; // en Windows no se puede renombrar un archivo abierto
        let _ = fs::remove_file(self.path(self.keep - 1));
        for n in (0..self.keep - 1).rev() {
            let from = self.path(n);
            if from.exists() {
                // Si no se puede renombrar (antivirus, bloqueo) se prefiere perder el archivo viejo a crecer sin límite.
                if fs::rename(&from, self.path(n + 1)).is_err() {
                    let _ = fs::remove_file(&from);
                }
            }
        }
        self.reopen()
    }

    /// Texto de una línea: sin saltos, sin la carpeta personal y acotado.
    fn clean(&self, msg: &str) -> String {
        let mut s = msg.replace("\r\n", "\\n").replace(['\n', '\r'], "\\n");
        if let Some(home) = &self.home {
            s = s.replace(home.as_str(), "~");
            // Rutas de Windows escritas con la otra barra.
            s = s.replace(&home.replace('\\', "/"), "~");
        }
        if s.chars().count() > MAX_LINE {
            s = s.chars().take(MAX_LINE).collect::<String>() + "…";
        }
        s
    }

    pub fn write(&mut self, level: Level, component: &str, msg: &str) {
        self.write_at(unix_now(), level, component, msg);
    }

    fn write_at(&mut self, secs: u64, level: Level, component: &str, msg: &str) {
        let line = format!(
            "{} {} {:<8} {}\n",
            iso_utc(secs),
            level.tag(),
            self.clean(component),
            self.clean(msg)
        );
        if self.size > 0 && self.size + line.len() as u64 > self.max_bytes {
            let _ = self.rotate();
        }
        if self.file.is_none() && self.reopen().is_err() {
            return;
        }
        if let Some(f) = self.file.as_mut()
            && f.write_all(line.as_bytes()).is_ok()
        {
            self.size += line.len() as u64;
        }
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// `2026-11-05T23:58:01Z` a partir de segundos desde 1970 (días civiles de Howard Hinnant, sin dependencias).
fn iso_utc(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

static LOGGER: OnceLock<Mutex<RotatingLog>> = OnceLock::new();

/// Activa el registro en `dir` (la app lo llama una vez al arrancar). Una segunda llamada no hace nada.
pub fn init(dir: &Path) -> std::io::Result<()> {
    let home = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).ok();
    let log = RotatingLog::open(dir, MAX_BYTES, KEEP, home)?;
    let _ = LOGGER.set(Mutex::new(log));
    Ok(())
}

/// Carpeta de registros dentro de la carpeta de datos.
pub fn logs_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("logs")
}

pub fn log(level: Level, component: &str, msg: impl AsRef<str>) {
    if let Some(l) = LOGGER.get() {
        l.lock()
            .unwrap_or_else(|p| p.into_inner())
            .write(level, component, msg.as_ref());
    }
}

pub fn info(component: &str, msg: impl AsRef<str>) {
    log(Level::Info, component, msg);
}

pub fn warn(component: &str, msg: impl AsRef<str>) {
    log(Level::Warn, component, msg);
}

pub fn error(component: &str, msg: impl AsRef<str>) {
    log(Level::Error, component, msg);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    }

    #[test]
    fn formats_utc_dates() {
        assert_eq!(iso_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso_utc(1_793_923_081), "2026-11-05T23:58:01Z");
        assert_eq!(iso_utc(951_782_400), "2000-02-29T00:00:00Z");
    }

    #[test]
    fn writes_one_line_per_event() {
        let dir = tempfile::tempdir().unwrap();
        let mut log = RotatingLog::open(dir.path(), MAX_BYTES, KEEP, None).unwrap();
        log.write_at(0, Level::Info, "arranque", "versión 1.0");
        log.write_at(60, Level::Error, "worker", "se cerró\nsegunda línea");
        let text = fs::read_to_string(dir.path().join("pliego.log")).unwrap();
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines.len(), 2, "{text}");
        assert_eq!(lines[0], "1970-01-01T00:00:00Z INFO  arranque versión 1.0");
        assert!(lines[1].ends_with("se cerró\\nsegunda línea"), "{}", lines[1]);
        assert!(lines[1].contains("ERROR worker"));
    }

    #[test]
    fn hides_the_home_folder_and_cuts_long_lines() {
        let dir = tempfile::tempdir().unwrap();
        let mut log = RotatingLog::open(dir.path(), MAX_BYTES, KEEP, Some("C:\\Users\\ana".into())).unwrap();
        log.write_at(
            0,
            Level::Warn,
            "motor",
            "no se pudo abrir C:\\Users\\ana\\Documentos\\a.pdf y C:/Users/ana/b.pdf",
        );
        log.write_at(0, Level::Warn, "motor", &"x".repeat(5000));
        let text = fs::read_to_string(dir.path().join("pliego.log")).unwrap();
        assert!(!text.contains("ana"), "{text}");
        assert!(text.contains("~\\Documentos\\a.pdf y ~/b.pdf"));
        assert!(text.lines().all(|l| l.chars().count() < MAX_LINE + 60));
    }

    #[test]
    fn rotates_and_keeps_a_bounded_number_of_files() {
        let dir = tempfile::tempdir().unwrap();
        let mut log = RotatingLog::open(dir.path(), 400, 3, None).unwrap();
        for i in 0..100 {
            log.write_at(i, Level::Info, "prueba", &format!("evento número {i:03} con relleno"));
        }
        assert_eq!(files(dir.path()), ["pliego.1.log", "pliego.2.log", "pliego.log"]);
        for name in files(dir.path()) {
            let len = fs::metadata(dir.path().join(&name)).unwrap().len();
            assert!(len <= 400, "{name} pesa {len}");
        }
        // Lo último escrito está en el activo; lo antiguo se descartó.
        let active = fs::read_to_string(dir.path().join("pliego.log")).unwrap();
        assert!(active.contains("099"));
        let all: String = files(dir.path())
            .iter()
            .map(|n| fs::read_to_string(dir.path().join(n)).unwrap())
            .collect();
        assert!(!all.contains("número 000"));
    }

    #[test]
    fn appends_after_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        RotatingLog::open(dir.path(), MAX_BYTES, KEEP, None)
            .unwrap()
            .write_at(0, Level::Info, "a", "uno");
        RotatingLog::open(dir.path(), MAX_BYTES, KEEP, None)
            .unwrap()
            .write_at(1, Level::Info, "a", "dos");
        let text = fs::read_to_string(dir.path().join("pliego.log")).unwrap();
        assert_eq!(text.lines().count(), 2);
    }

    #[test]
    fn does_nothing_before_init() {
        // Sin `init` (CLI, pruebas) las funciones no escriben ni fallan.
        info("x", "no hace nada");
        error("x", String::from("tampoco"));
    }
}
