use std::path::PathBuf;

/// Error común de Pliego. Los módulos pueden envolver errores propios en [`Error::Other`].
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("error de E/S en {path:?}: {source}")]
    Io {
        path: Option<PathBuf>,
        #[source]
        source: std::io::Error,
    },
    #[error("error del motor PDF: {0}")]
    Pdf(String),
    #[error("el documento necesita contraseña o la contraseña no es válida")]
    Password,
    #[error("entrada no válida: {0}")]
    InvalidInput(String),
    #[error("operación no soportada: {0}")]
    Unsupported(String),
    #[error("operación cancelada")]
    Cancelled,
    #[error("{0}")]
    Other(String),
}

impl From<std::io::Error> for Error {
    fn from(source: std::io::Error) -> Self {
        Error::Io { path: None, source }
    }
}

impl Error {
    pub fn io_at(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Error::Io {
            path: Some(path.into()),
            source,
        }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
