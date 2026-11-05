//! «Acerca de»: versión, licencia, enlace al código fuente y componentes de terceros.

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
        license: "BUSL-1.1",
        source_url: SOURCE_URL,
        components: vec![
            Component {
                name: "PDFium (pdfium-render)",
                version: env!("PLIEGO_DEP_PDFIUM"),
                license: "BSD-3-Clause / Apache-2.0",
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
                name: "Boa (JavaScript de formularios)",
                version: env!("PLIEGO_DEP_BOA"),
                license: "MIT / Unlicense",
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
        assert!(info.components.iter().all(|c| !c.name.contains("MuPDF")));
        assert!(
            info.components.iter().all(|c| c.version != "desconocida"),
            "{:?}",
            info.components
        );
        let pdfium = info.components.iter().find(|c| c.name.starts_with("PDFium")).unwrap();
        assert!(
            pdfium.version.chars().next().unwrap().is_ascii_digit(),
            "versión: {}",
            pdfium.version
        );
        assert_eq!(info.license, "BUSL-1.1");
        assert!(info.source_url.starts_with("https://"));
    }
}
