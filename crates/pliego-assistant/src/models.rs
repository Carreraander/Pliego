//! Catálogo de modelos GGUF que se pueden descargar bajo demanda. Solo licencias abiertas.

use serde::{Deserialize, Serialize};

/// Requisitos del equipo para un modelo.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Requirements {
    /// RAM libre recomendada, en GB.
    pub ram_gb: f32,
    /// Descarga, en MB.
    pub download_mb: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelSpec {
    pub id: &'static str,
    pub name: &'static str,
    /// Identificador SPDX de la licencia de los pesos.
    pub license: &'static str,
    pub file_name: &'static str,
    pub url: &'static str,
    /// SHA-256 del archivo. `None` hasta fijarlo en una descarga verificada: sin hash no se descarga.
    pub sha256: Option<&'static str>,
    pub requirements: Requirements,
    pub context_tokens: u32,
    /// Entiende imágenes (texto alternativo).
    pub multimodal: bool,
    pub notes: &'static str,
}

/// Modelos ofrecidos, de menor a mayor. Todos con licencia permisiva (Apache-2.0 o MIT) y bajo 4 GB.
pub fn catalog() -> &'static [ModelSpec] {
    const MODELS: &[ModelSpec] = &[
        ModelSpec {
            id: "qwen2.5-1.5b-instruct-q4",
            name: "Qwen2.5 1,5B Instruct (ligero)",
            license: "Apache-2.0",
            file_name: "qwen2.5-1.5b-instruct-q4_k_m.gguf",
            url: "https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct-GGUF/resolve/main/qwen2.5-1.5b-instruct-q4_k_m.gguf",
            sha256: None,
            requirements: Requirements {
                ram_gb: 3.0,
                download_mb: 1100,
            },
            context_tokens: 8192,
            multimodal: false,
            notes: "Para equipos modestos. Respuestas más simples.",
        },
        ModelSpec {
            id: "phi-3.5-mini-instruct-q4",
            name: "Phi-3.5 mini Instruct (equilibrado)",
            license: "MIT",
            file_name: "Phi-3.5-mini-instruct-Q4_K_M.gguf",
            url: "https://huggingface.co/bartowski/Phi-3.5-mini-instruct-GGUF/resolve/main/Phi-3.5-mini-instruct-Q4_K_M.gguf",
            sha256: None,
            requirements: Requirements {
                ram_gb: 5.0,
                download_mb: 2400,
            },
            context_tokens: 8192,
            multimodal: false,
            notes: "Mejor razonamiento; el español es correcto pero no su punto fuerte.",
        },
        ModelSpec {
            id: "qwen3-4b-instruct-2507-q4",
            name: "Qwen3 4B Instruct 2507 (recomendado)",
            license: "Apache-2.0",
            file_name: "Qwen3-4B-Instruct-2507-Q4_K_M.gguf",
            url: "https://huggingface.co/unsloth/Qwen3-4B-Instruct-2507-GGUF/resolve/main/Qwen3-4B-Instruct-2507-Q4_K_M.gguf",
            sha256: None,
            requirements: Requirements {
                ram_gb: 6.0,
                download_mb: 2500,
            },
            context_tokens: 16384,
            multimodal: false,
            notes: "Buen español y contexto largo.",
        },
    ];
    MODELS
}

impl ModelSpec {
    pub fn find(id: &str) -> Option<&'static ModelSpec> {
        catalog().iter().find(|m| m.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_open_and_small() {
        for m in catalog() {
            assert!(
                matches!(m.license, "Apache-2.0" | "MIT"),
                "{} tiene licencia {}",
                m.id,
                m.license
            );
            assert!(m.requirements.download_mb < 4000, "{} supera 4 GB", m.id);
            assert!(m.url.starts_with("https://huggingface.co/"));
        }
        assert!(ModelSpec::find("qwen3-4b-instruct-2507-q4").is_some());
    }
}
