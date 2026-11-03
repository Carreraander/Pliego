//! Tipos y utilidades de la frontera con la interfaz que no dependen de Tauri (así se prueban sin enlazar WebView2).

use std::path::Path;

use pliego_core::Error;
use serde::Serialize;

/// Error serializable que reciben los comandos desde la interfaz.
/// `code` permite reaccionar (p. ej. pedir contraseña); `message` es para mostrar a la persona.
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

/// Rutas de PDF entre los argumentos de la línea de comandos (se ignoran el ejecutable y las opciones `-x`).
pub fn pdf_args<I: IntoIterator<Item = String>>(args: I) -> Vec<String> {
    args.into_iter()
        .skip(1)
        .filter(|a| !a.starts_with('-'))
        .filter(|a| Path::new(a).extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf")))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn password_error_has_stable_code() {
        let e: ApiError = Error::Password.into();
        assert_eq!(e.code, "password");
        assert!(!e.message.is_empty());
        assert_eq!(serde_json::to_value(&e).unwrap()["code"], "password");
    }

    #[test]
    fn keeps_only_pdf_paths() {
        let got = pdf_args(v(&["pliego-app.exe", "a.pdf", "--flag", "b.PDF", "c.txt"]));
        assert_eq!(got, v(&["a.pdf", "b.PDF"]));
        assert!(pdf_args(v(&["pliego-app.exe"])).is_empty());
    }
}
