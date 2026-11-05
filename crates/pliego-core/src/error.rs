use std::path::PathBuf;

/// Error común de Pliego. Los módulos pueden envolver errores propios en [`Error::Other`].
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{}", io_message(.path.as_deref(), .source))]
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

/// Mensaje de un error de E/S para la persona: sin `Some("...")` ni barras duplicadas, y con una pista cuando el sistema
/// niega el acceso (el caso típico en Windows es un archivo abierto por otro programa).
fn io_message(path: Option<&std::path::Path>, source: &std::io::Error) -> String {
    use std::io::ErrorKind;
    match (path, source.kind()) {
        (Some(p), ErrorKind::NotFound) => format!("no se encuentra {}", p.display()),
        (Some(p), ErrorKind::PermissionDenied) => format!(
            "acceso denegado a {} (¿está abierto en otro programa, es de solo lectura o está protegido?)",
            p.display()
        ),
        (Some(p), _) => format!("error de E/S en {}: {source}", p.display()),
        (None, _) => format!("error de E/S: {source}"),
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Error as IoError, ErrorKind};

    #[test]
    fn el_mensaje_no_enseña_option_ni_barras_duplicadas() {
        let e = Error::io_at("C:\\Users\\ana\\a.pdf", IoError::from(ErrorKind::PermissionDenied));
        let m = e.to_string();
        assert!(!m.contains("Some("), "{m}");
        assert!(!m.contains("\\\\"), "{m}");
        assert!(m.contains("acceso denegado"), "{m}");
        let e = Error::io_at("a.pdf", IoError::other("disco lleno"));
        assert_eq!(e.to_string(), "error de E/S en a.pdf: disco lleno");
        assert_eq!(Error::from(IoError::other("x")).to_string(), "error de E/S: x");
    }
}
