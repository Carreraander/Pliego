//! Textos nativos (línea de órdenes, `TaskDialog`, errores). La interfaz web lleva los suyos en `installer/setup/ui`.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Es,
    En,
}

impl Lang {
    /// A partir de una etiqueta de configuración regional (`es-ES`, `en-US`…); por defecto español.
    pub fn from_locale(tag: &str) -> Lang {
        if tag.to_ascii_lowercase().starts_with("en") {
            Lang::En
        } else {
            Lang::Es
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Lang::Es => "es",
            Lang::En => "en",
        }
    }
}

pub fn webview2_title(l: Lang) -> &'static str {
    match l {
        Lang::Es => "Instalando un componente de Windows necesario (WebView2)…",
        Lang::En => "Installing a required Windows component (WebView2)…",
    }
}

pub fn webview2_offline(l: Lang) -> &'static str {
    match l {
        Lang::Es => {
            "Pliego necesita Microsoft Edge WebView2 y no se pudo descargar. Descárgalo desde https://developer.microsoft.com/microsoft-edge/webview2/ y vuelve a ejecutar el instalador."
        }
        Lang::En => {
            "Pliego needs Microsoft Edge WebView2 and it could not be downloaded. Get it from https://developer.microsoft.com/microsoft-edge/webview2/ and run the installer again."
        }
    }
}

pub fn fatal(l: Lang, detail: &str) -> String {
    match l {
        Lang::Es => format!(
            "No se pudo completar la operación.\n\n{detail}\n\nRegistro: {}",
            crate::logging::path().display()
        ),
        Lang::En => format!(
            "The operation could not be completed.\n\n{detail}\n\nLog: {}",
            crate::logging::path().display()
        ),
    }
}

pub fn usage() -> &'static str {
    "Pliego-Setup.exe [/S | --silent] [--per-machine] [--dir <ruta>] [--[no-]desktop-shortcut] [--no-assoc]\n\
     \x20 [--no-context-menu] [--[no-]path] [--no-updates] [--log <archivo>] [--lang es|en]\n\
     Uninstall.exe --uninstall [--silent] [--purge]\n\
     Uninstall.exe --post-update [--root <ruta>]\n\
     Pliego-Setup.exe --update --paq <archivo> [--root <ruta>]\n\
     pliego-setup --pack --base <exe> --paq <archivo> --out <exe>\n\
     Códigos de salida: 0 correcto · 1602 cancelado · 1603 error · 87 argumentos no válidos"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_mapping() {
        assert_eq!(Lang::from_locale("en-US"), Lang::En);
        assert_eq!(Lang::from_locale("es-ES"), Lang::Es);
        assert_eq!(Lang::from_locale("fr-FR"), Lang::Es);
    }
}
