//! Error serializable que reciben los comandos desde la interfaz.

use pliego_core::Error;
use serde::Serialize;

/// `code` permite a la interfaz reaccionar (p. ej. pedir contraseña); `message` es para mostrar a la persona.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ApiError {
    pub code: &'static str,
    pub message: String,
}

impl From<Error> for ApiError {
    fn from(e: Error) -> Self {
        let code = match &e {
            Error::Password => "password",
            Error::Io { .. } => "io",
            Error::Pdf(_) => "pdf",
            Error::InvalidInput(_) => "invalid_input",
            Error::Unsupported(_) => "unsupported",
            Error::Cancelled => "cancelled",
            Error::Other(_) => "other",
        };
        Self {
            code,
            message: e.to_string(),
        }
    }
}

pub type ApiResult<T> = Result<T, ApiError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_error_has_stable_code() {
        let e: ApiError = Error::Password.into();
        assert_eq!(e.code, "password");
        assert!(!e.message.is_empty());
        assert_eq!(serde_json::to_value(&e).unwrap()["code"], "password");
    }
}
