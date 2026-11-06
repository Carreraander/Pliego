//! Abrir en el navegador del sistema una dirección que viene de un PDF (enlaces del documento y marcadores).
//!
//! La dirección es **entrada no fiable**: solo se abren `http`, `https` y `mailto`, sin espacios ni caracteres de control
//! ni nada fuera de ASCII, y sin pasar por un intérprete de órdenes (el programa y sus argumentos van por separado).
//! Quien llama debe haber pedido antes confirmación a la persona.

/// Longitud máxima de una dirección que se abre.
pub const MAX_URL_LEN: usize = 2048;

/// Comprueba la dirección y la devuelve limpia, o el motivo (en español) por el que no se abre.
pub fn check_url(url: &str) -> Result<String, String> {
    let url = url.trim();
    if url.is_empty() {
        return Err("El enlace está vacío.".into());
    }
    if url.len() > MAX_URL_LEN {
        return Err("La dirección del enlace es demasiado larga.".into());
    }
    if !url.bytes().all(|b| b.is_ascii_graphic()) || url.contains(['"', '<', '>', '\\', '^', '`', '|']) {
        return Err("La dirección del enlace tiene caracteres que no se admiten.".into());
    }
    let scheme = url
        .split_once(':')
        .map(|(s, _)| s.to_ascii_lowercase())
        .unwrap_or_default();
    let rest = &url[scheme.len() + 1..url.len().max(scheme.len() + 1)];
    match scheme.as_str() {
        "http" | "https" if rest.starts_with("//") && rest.len() > 2 => Ok(url.to_owned()),
        "mailto" if rest.len() > 1 => Ok(url.to_owned()),
        "" => Err("El enlace no es una dirección web.".into()),
        other => Err(format!(
            "Pliego no abre enlaces de tipo «{other}»; solo web (http, https) y correo (mailto)."
        )),
    }
}

/// Orden del sistema para abrir la dirección ya comprobada con [`check_url`].
pub fn open_command(url: &str) -> (&'static str, Vec<String>) {
    if cfg!(windows) {
        (
            "rundll32.exe",
            vec!["url.dll,FileProtocolHandler".into(), url.to_owned()],
        )
    } else if cfg!(target_os = "macos") {
        ("open", vec![url.to_owned()])
    } else {
        ("xdg-open", vec![url.to_owned()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_and_mail_addresses_pass() {
        assert_eq!(
            check_url("  https://pdfpliego.com/a?b=c#d ").unwrap(),
            "https://pdfpliego.com/a?b=c#d"
        );
        assert!(check_url("HTTP://example.org").is_ok());
        assert!(check_url("mailto:hola@pdfpliego.com?subject=Hola").is_ok());
    }

    #[test]
    fn other_schemes_and_odd_characters_are_refused() {
        for bad in [
            "file:///C:/Windows/System32/calc.exe",
            "javascript:alert(1)",
            "ms-msdt:/id PCWDiagnostic",
            "\\\\servidor\\recurso",
            "C:\\Windows\\notepad.exe",
            "https://",
            "mailto:",
            "https://a b",
            "https://a.com/\"&calc",
            "https://a.com/\nx",
            "https://exámple.com",
            "",
            "pliego",
        ] {
            assert!(check_url(bad).is_err(), "{bad:?} debería rechazarse");
        }
        assert!(check_url(&format!("https://a.com/{}", "x".repeat(MAX_URL_LEN))).is_err());
    }

    #[test]
    fn the_address_is_one_argument() {
        let (_, args) = open_command("https://a.com/x");
        assert_eq!(args.last().map(String::as_str), Some("https://a.com/x"));
    }
}
