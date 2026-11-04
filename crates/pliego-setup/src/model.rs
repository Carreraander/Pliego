//! Tipos comunes: opciones de instalación, estado instalado, errores y códigos de salida.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

/// Códigos de salida (ver `instalador.md` §7).
pub mod exit {
    pub const OK: u32 = 0;
    pub const CANCELLED: u32 = 1602;
    pub const FAILED: u32 = 1603;
    /// Argumentos no válidos.
    pub const USAGE: u32 = 87;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    User,
    Machine,
}

/// Lo que el usuario elige en «Opciones de instalación».
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Options {
    pub scope: Scope,
    pub dir: PathBuf,
    pub desktop_shortcut: bool,
    pub assoc: bool,
    pub context_menu: bool,
    pub path: bool,
    pub updates: bool,
}

impl Options {
    pub fn defaults_for(scope: Scope, dir: PathBuf) -> Self {
        Options {
            scope,
            dir,
            desktop_shortcut: false,
            assoc: true,
            context_menu: true,
            path: false,
            updates: true,
        }
    }
}

impl Default for Options {
    fn default() -> Self {
        Options::defaults_for(Scope::User, PathBuf::new())
    }
}

/// `install.json` en la raíz: qué se registró, para poder deshacerlo y repetirlo tras actualizar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallInfo {
    pub version: String,
    pub options: Options,
    /// Accesos directos creados (se borran al desinstalar).
    pub shortcuts: Vec<PathBuf>,
}

pub const INFO_FILE: &str = "install.json";

/// Versión del programa instalador (la del paquete embebido se lee del `.paq`).
pub const SETUP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug)]
pub enum SetupError {
    /// Faltan `needed` bytes libres en `drive`.
    NoSpace { needed: u64, free: u64, drive: String },
    /// El instalador o el paquete están dañados.
    Corrupt(String),
    /// Sin permisos para escribir o registrar.
    Permission(String),
    /// La carpeta de destino no es válida (vacía, relativa, en una unidad inexistente o es un archivo).
    BadPath(String),
    /// Windows anterior a 10 versión 2004.
    OldWindows(u32),
    /// Pliego está abierto y no se pudo cerrar.
    Running,
    /// El usuario canceló.
    Cancelled,
    /// Otro fallo; el texto va al registro.
    Other(String),
}

impl SetupError {
    pub fn exit_code(&self) -> u32 {
        match self {
            SetupError::Cancelled => exit::CANCELLED,
            _ => exit::FAILED,
        }
    }

    /// Clave del mensaje en la interfaz (`errors.<kind>`).
    pub fn kind(&self) -> &'static str {
        match self {
            SetupError::NoSpace { .. } => "no_space",
            SetupError::Corrupt(_) => "corrupt",
            SetupError::Permission(_) => "permission",
            SetupError::BadPath(_) => "bad_path",
            SetupError::OldWindows(_) => "old_windows",
            SetupError::Running => "running",
            SetupError::Cancelled => "cancelled",
            SetupError::Other(_) => "other",
        }
    }
}

impl fmt::Display for SetupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SetupError::NoSpace { needed, free, drive } => {
                write!(
                    f,
                    "espacio insuficiente en {drive}: hacen falta {needed} bytes y hay {free}"
                )
            }
            SetupError::Corrupt(m) => write!(f, "paquete dañado: {m}"),
            SetupError::Permission(m) => write!(f, "sin permisos: {m}"),
            SetupError::BadPath(m) => write!(f, "carpeta de destino no válida: {m}"),
            SetupError::OldWindows(b) => write!(f, "Windows demasiado antiguo (compilación {b}, mínimo 19041)"),
            SetupError::Running => write!(f, "Pliego está abierto"),
            SetupError::Cancelled => write!(f, "cancelado"),
            SetupError::Other(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for SetupError {}

impl From<pliego_install::Error> for SetupError {
    fn from(e: pliego_install::Error) -> Self {
        use pliego_install::Error as E;
        match e {
            E::Corrupt(_) | E::UnsafeEntry(_) | E::InvalidVersion(_) => SetupError::Corrupt(e.to_string()),
            E::Io { ref source, .. } if source.kind() == std::io::ErrorKind::PermissionDenied => {
                SetupError::Permission(e.to_string())
            }
            other => SetupError::Other(other.to_string()),
        }
    }
}

impl From<std::io::Error> for SetupError {
    fn from(e: std::io::Error) -> Self {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            SetupError::Permission(e.to_string())
        } else {
            SetupError::Other(e.to_string())
        }
    }
}

pub type Result<T> = std::result::Result<T, SetupError>;

/// Progreso que el motor comunica a la interfaz.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Progress {
    /// `checking`, `copying`, `verifying`, `registering`, `shortcuts`, `finishing`, `removing`.
    pub step: &'static str,
    /// 0..=100 del total.
    pub percent: u8,
}
