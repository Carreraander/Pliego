//! Idioma de los textos que Pliego escribe dentro de un documento (informes, hojas de firmas, nombres de campo…).
//!
//! Los mensajes de error y de progreso se escriben en español y la interfaz los traduce al mostrarlos (ver
//! `app/src/i18n/engine/`); en cambio lo que se guarda en un PDF o en otro archivo no se puede traducir después, así
//! que quien lo escribe recibe el idioma de la interfaz (`"es"` o `"en"`).

use serde::{Deserialize, Serialize};

/// Idioma de los textos que se escriben en un archivo. Por defecto, español (el idioma del producto).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    #[default]
    Es,
    En,
}

impl Lang {
    /// Idioma a partir de una etiqueta (`en`, `en-US`, `es-ES`…): inglés si empieza por `en`; en otro caso, español.
    pub fn from_tag(tag: &str) -> Lang {
        if tag.trim().to_ascii_lowercase().starts_with("en") {
            Lang::En
        } else {
            Lang::Es
        }
    }

    /// Idioma del entorno: `PLIEGO_LANG` si está definida y, si no, la primera de `LC_ALL`, `LC_MESSAGES` y `LANG` que
    /// tenga valor. Sin ninguna, español. Lo usa la línea de comandos para los textos que escribe en los archivos.
    pub fn from_env() -> Lang {
        Lang::from_vars(|k| std::env::var(k).ok())
    }

    /// Como [`Lang::from_env`], con un lector de variables propio (para probarlo sin tocar el entorno).
    pub fn from_vars(get: impl Fn(&str) -> Option<String>) -> Lang {
        ["PLIEGO_LANG", "LC_ALL", "LC_MESSAGES", "LANG"]
            .iter()
            .find_map(|k| get(k).filter(|v| !v.trim().is_empty() && v.trim() != "C" && v.trim() != "POSIX"))
            .map_or(Lang::Es, |v| Lang::from_tag(&v))
    }

    /// Elige entre el texto en español y el texto en inglés.
    pub fn pick<'a>(self, es: &'a str, en: &'a str) -> &'a str {
        match self {
            Lang::Es => es,
            Lang::En => en,
        }
    }

    /// Código corto del idioma (`es` o `en`).
    pub fn tag(self) -> &'static str {
        self.pick("es", "en")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_the_language_from_a_tag() {
        assert_eq!(Lang::from_tag("en-US"), Lang::En);
        assert_eq!(Lang::from_tag(" EN"), Lang::En);
        assert_eq!(Lang::from_tag("es-ES"), Lang::Es);
        assert_eq!(Lang::from_tag(""), Lang::Es);
        assert_eq!(Lang::default(), Lang::Es);
    }

    #[test]
    fn reads_the_language_from_the_environment() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |k: &str| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string())
        };
        assert_eq!(Lang::from_vars(env(&[("LANG", "en_US.UTF-8")])), Lang::En);
        assert_eq!(
            Lang::from_vars(env(&[("LANG", "en_US.UTF-8"), ("PLIEGO_LANG", "es")])),
            Lang::Es
        );
        assert_eq!(Lang::from_vars(env(&[("LC_ALL", "C"), ("LANG", "en_GB")])), Lang::En);
        assert_eq!(Lang::from_vars(env(&[])), Lang::Es);
    }

    #[test]
    fn serializes_as_a_lowercase_tag() {
        assert_eq!(serde_json::to_string(&Lang::En).unwrap(), "\"en\"");
        assert_eq!(serde_json::from_str::<Lang>("\"es\"").unwrap(), Lang::Es);
        assert_eq!(Lang::En.pick("hola", "hello"), "hello");
    }
}
