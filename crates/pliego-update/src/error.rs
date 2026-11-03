/// Errores de actualización. Los mensajes son para personas: la app los muestra en Ajustes > Actualizaciones.
#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error("no se pudo conectar con el servidor de actualizaciones: {0}")]
    Network(String),
    #[error("el servidor de actualizaciones respondió con un error ({0})")]
    Http(u16),
    #[error("la información de actualización no está firmada por Pliego; se ha ignorado")]
    BadSignature,
    #[error("la información de actualización está dañada: {0}")]
    Malformed(String),
    #[error("la información de actualización es de otro producto o canal")]
    WrongTarget,
    #[error("la información de actualización ha caducado; vuelve a intentarlo más tarde")]
    Expired,
    #[error("la información de actualización es más antigua que la última vista; se ha ignorado")]
    Replayed,
    #[error("la descarga no coincide con la publicada (tamaño o huella distintos); se ha borrado")]
    DigestMismatch,
    #[error("la dirección del servidor no es válida o no usa HTTPS: {0}")]
    InsecureUrl(String),
    #[error("actualización cancelada")]
    Cancelled,
    #[error("error de archivo: {0}")]
    Io(#[from] std::io::Error),
}

impl From<UpdateError> for pliego_core::Error {
    fn from(e: UpdateError) -> Self {
        match e {
            UpdateError::Cancelled => pliego_core::Error::Cancelled,
            UpdateError::Io(source) => pliego_core::Error::Io { path: None, source },
            other => pliego_core::Error::Other(other.to_string()),
        }
    }
}

pub type Result<T, E = UpdateError> = std::result::Result<T, E>;
