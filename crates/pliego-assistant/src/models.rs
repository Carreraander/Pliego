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
    /// Tamaño exacto del archivo, en bytes (para comprobar la descarga y avisar del espacio necesario).
    pub size_bytes: u64,
    /// Modelo que se propone por defecto.
    pub recommended: bool,
    pub requirements: Requirements,
    pub context_tokens: u32,
    /// Entiende imágenes (texto alternativo).
    pub multimodal: bool,
    pub notes: &'static str,
}

/// Modelos ofrecidos, de menor a mayor. Todos con licencia permisiva (Apache-2.0 o MIT) y bajo 4 GB.
/// Hashes y tamaños verificados contra Hugging Face el 2026-11-03 (descarga completa en acr12).
/// Medido con 10 preguntas sobre 50 páginas: Qwen3 4B citó la página correcta 10/10, Phi-3.5 7/10, Qwen2.5 1,5B 1/10.
pub fn catalog() -> &'static [ModelSpec] {
    const MODELS: &[ModelSpec] = &[
        ModelSpec {
            id: "qwen2.5-1.5b-instruct-q4",
            name: "Qwen2.5 1,5B Instruct (ligero)",
            license: "Apache-2.0",
            file_name: "qwen2.5-1.5b-instruct-q4_k_m.gguf",
            url: "https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct-GGUF/resolve/main/qwen2.5-1.5b-instruct-q4_k_m.gguf",
            sha256: Some("6a1a2eb6d15622bf3c96857206351ba97e1af16c30d7a74ee38970e434e9407e"),
            size_bytes: 1_117_320_736,
            recommended: false,
            requirements: Requirements {
                ram_gb: 3.0,
                download_mb: 1118,
            },
            context_tokens: 8192,
            multimodal: false,
            notes: "Para equipos modestos. Responde bien pero casi nunca cita la página: úsalo solo si no cabe otro.",
        },
        ModelSpec {
            id: "phi-3.5-mini-instruct-q4",
            name: "Phi-3.5 mini Instruct (equilibrado)",
            license: "MIT",
            file_name: "Phi-3.5-mini-instruct-Q4_K_M.gguf",
            url: "https://huggingface.co/bartowski/Phi-3.5-mini-instruct-GGUF/resolve/main/Phi-3.5-mini-instruct-Q4_K_M.gguf",
            sha256: Some("e4165e3a71af97f1b4820da61079826d8752a2088e313af0c7d346796c38eff5"),
            size_bytes: 2_393_232_672,
            recommended: false,
            requirements: Requirements {
                ram_gb: 5.0,
                download_mb: 2394,
            },
            context_tokens: 8192,
            multimodal: false,
            notes: "Razona bien; algo más lento y a veces cita mal la página.",
        },
        ModelSpec {
            id: "qwen3-4b-instruct-2507-q4",
            name: "Qwen3 4B Instruct 2507 (recomendado)",
            license: "Apache-2.0",
            file_name: "Qwen3-4B-Instruct-2507-Q4_K_M.gguf",
            url: "https://huggingface.co/unsloth/Qwen3-4B-Instruct-2507-GGUF/resolve/main/Qwen3-4B-Instruct-2507-Q4_K_M.gguf",
            sha256: Some("3605803b982cb64aead44f6c1b2ae36e3acdb41d8e46c8a94c6533bc4c67e597"),
            size_bytes: 2_497_281_120,
            recommended: true,
            requirements: Requirements {
                ram_gb: 6.0,
                download_mb: 2498,
            },
            context_tokens: 16384,
            multimodal: false,
            notes: "El más fiable citando páginas (10/10 en la prueba) y buen español.",
        },
    ];
    MODELS
}

/// Carpeta donde se guardan los modelos: `%LOCALAPPDATA%\Pliego\models` en Windows, o
/// `$XDG_DATA_HOME/pliego/models` (`~/.local/share/pliego/models`) en otros sistemas.
pub fn default_models_dir() -> std::path::PathBuf {
    use std::path::PathBuf;
    let env = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    if cfg!(windows) {
        env("LOCALAPPDATA")
            .unwrap_or_else(std::env::temp_dir)
            .join("Pliego")
            .join("models")
    } else {
        env("XDG_DATA_HOME")
            .or_else(|| env("HOME").map(|h| h.join(".local/share")))
            .unwrap_or_else(std::env::temp_dir)
            .join("pliego")
            .join("models")
    }
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
        assert_eq!(catalog().iter().filter(|m| m.recommended).count(), 1);
        for m in catalog() {
            let h = m.sha256.expect("todo modelo lleva SHA-256");
            assert!(h.len() == 64 && h.bytes().all(|b| b.is_ascii_hexdigit()), "{}", m.id);
            assert!(m.size_bytes > 500_000_000);
        }
    }
}
