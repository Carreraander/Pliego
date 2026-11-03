//! «Acerca de»: versión, licencia, enlace al código fuente (obligatorio por la AGPL) y componentes de terceros.

use serde::Serialize;

pub const SOURCE_URL: &str = "https://github.com/Carreraander/Pliego";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Component {
    pub name: &'static str,
    pub version: &'static str,
    pub license: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AboutInfo {
    pub version: &'static str,
    pub license: &'static str,
    pub source_url: &'static str,
    pub components: Vec<Component>,
}

/// `version` es la de la app que llama (`env!("CARGO_PKG_VERSION")` de `pliego-app`).
pub fn about(version: &'static str) -> AboutInfo {
    AboutInfo {
        version,
        license: "AGPL-3.0-or-later",
        source_url: SOURCE_URL,
        components: vec![
            Component {
                name: "MuPDF",
                version: env!("PLIEGO_DEP_MUPDF"),
                license: "AGPL-3.0 (Artifex)",
            },
            Component {
                name: "Tauri",
                version: env!("PLIEGO_DEP_TAURI"),
                license: "MIT / Apache-2.0",
            },
            Component {
                name: "lopdf",
                version: env!("PLIEGO_DEP_LOPDF"),
                license: "MIT",
            },
            Component {
                name: "rustls",
                version: env!("PLIEGO_DEP_RUSTLS"),
                license: "MIT / Apache-2.0 / ISC",
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_come_from_the_lockfile() {
        let info = about("9.9.9");
        let mupdf = info.components.iter().find(|c| c.name == "MuPDF").unwrap();
        assert!(
            mupdf.version.chars().next().unwrap().is_ascii_digit(),
            "versión: {}",
            mupdf.version
        );
        assert_eq!(info.license, "AGPL-3.0-or-later");
        assert!(info.source_url.starts_with("https://"));
    }
}
