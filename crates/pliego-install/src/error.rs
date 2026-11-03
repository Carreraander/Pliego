use std::fmt;
use std::path::PathBuf;

/// Errores de instalación, con mensajes pensados para personas.
#[derive(Debug)]
pub enum Error {
    Io {
        context: String,
        source: std::io::Error,
    },
    /// Nombre de versión no válido (vacío, con separadores, `..`…).
    InvalidVersion(String),
    /// El paquete contiene una ruta absoluta, con `..`, un enlace u otro tipo de entrada no permitido.
    UnsafeEntry(String),
    /// El paquete está dañado o no coincide con su `files.json`.
    Corrupt(String),
    /// No hay versión anterior a la que volver.
    NoPrevious,
    /// No existe la carpeta de la versión.
    MissingVersion(PathBuf),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub(crate) fn io(context: impl Into<String>, source: std::io::Error) -> Self {
        Error::Io {
            context: context.into(),
            source,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io { context, source } => write!(f, "{context}: {source}"),
            Error::InvalidVersion(v) => write!(f, "versión no válida: «{v}»"),
            Error::UnsafeEntry(p) => write!(f, "el paquete contiene una entrada no permitida: «{p}»"),
            Error::Corrupt(m) => write!(f, "el paquete está dañado: {m}"),
            Error::NoPrevious => write!(f, "no hay una versión anterior a la que volver"),
            Error::MissingVersion(p) => write!(f, "falta la carpeta de la versión: {}", p.display()),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}
