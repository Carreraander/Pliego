//! Estado del asistente dentro de la app: ajustes, modelo cargado e índices por documento.
//!
//! La app guarda una `Session` tras un `Mutex`; los métodos son breves (clonan `Arc`) para que el trabajo
//! largo —indexar, preguntar, resumir— ocurra fuera del cerrojo.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use pliego_core::{Error, Result};

use crate::assistant::Assistant;
use crate::chunk::DocumentText;
use crate::config::AssistantConfig;
use crate::models::{ModelSpec, default_models_dir};
use crate::retrieve::DocIndex;

/// Texto e índice de un documento para una revisión concreta.
pub struct DocData {
    pub text: DocumentText,
    pub index: DocIndex,
}

type Loader = Box<dyn Fn(&Path) -> Result<Assistant> + Send>;

pub struct Session {
    models_dir: PathBuf,
    settings: PathBuf,
    config: AssistantConfig,
    assistant: Option<Arc<Assistant>>,
    docs: HashMap<u64, (u64, Arc<DocData>)>,
    loader: Loader,
}

impl Session {
    /// Sesión con la carpeta de modelos por defecto; los ajustes viven junto a ella (`assistant.json`).
    pub fn new() -> Self {
        Self::at(default_models_dir(), Box::new(Assistant::load))
    }

    pub fn at(models_dir: PathBuf, loader: Loader) -> Self {
        let settings = models_dir
            .parent()
            .map_or_else(|| models_dir.clone(), Path::to_path_buf)
            .join("assistant.json");
        let config = AssistantConfig::load_or_default(&settings);
        Self {
            models_dir,
            settings,
            config,
            assistant: None,
            docs: HashMap::new(),
            loader,
        }
    }

    pub fn config(&self) -> &AssistantConfig {
        &self.config
    }

    pub fn models_dir(&self) -> &Path {
        &self.models_dir
    }

    /// El motor llama.cpp está compilado en esta versión de la app.
    pub fn engine_available() -> bool {
        cfg!(feature = "llama")
    }

    pub fn set_enabled(&mut self, enabled: bool) -> Result<()> {
        self.config.enabled = enabled;
        if !enabled {
            // Desactivar libera la memoria del modelo y los índices.
            self.assistant = None;
            self.docs.clear();
        }
        self.config.save(&self.settings)
    }

    /// Elige un modelo del catálogo, que debe estar ya descargado.
    pub fn select_model(&mut self, id: &str) -> Result<()> {
        let spec = ModelSpec::find(id).ok_or_else(|| Error::InvalidInput(format!("no existe el modelo «{id}»")))?;
        if self.model_path(spec).is_none() {
            return Err(Error::InvalidInput("ese modelo todavía no está descargado".into()));
        }
        self.config.model_id = Some(spec.id.to_string());
        self.assistant = None;
        self.docs.clear();
        self.config.save(&self.settings)
    }

    /// Ruta del modelo si está descargado y completo.
    pub fn model_path(&self, spec: &ModelSpec) -> Option<PathBuf> {
        let path = self.models_dir.join(spec.file_name);
        (std::fs::metadata(&path).ok()?.len() == spec.size_bytes).then_some(path)
    }

    /// El asistente listo para usar; carga el modelo la primera vez.
    pub fn assistant(&mut self) -> Result<Arc<Assistant>> {
        if !self.config.enabled {
            return Err(Error::InvalidInput(
                "el asistente está desactivado; actívalo en sus ajustes".into(),
            ));
        }
        if let Some(a) = &self.assistant {
            return Ok(a.clone());
        }
        let id = self
            .config
            .model_id
            .as_deref()
            .ok_or_else(|| Error::InvalidInput("elige y descarga un modelo para usar el asistente".into()))?;
        let spec = ModelSpec::find(id).ok_or_else(|| Error::InvalidInput(format!("no existe el modelo «{id}»")))?;
        let path = self
            .model_path(spec)
            .ok_or_else(|| Error::InvalidInput("el modelo elegido no está descargado".into()))?;
        let a = Arc::new((self.loader)(&path)?);
        self.assistant = Some(a.clone());
        Ok(a)
    }

    /// Datos ya calculados del documento si siguen vigentes para esa revisión.
    pub fn doc_data(&self, doc: u64, revision: u64) -> Option<Arc<DocData>> {
        self.docs
            .get(&doc)
            .filter(|(r, _)| *r == revision)
            .map(|(_, d)| d.clone())
    }

    pub fn store_doc_data(&mut self, doc: u64, revision: u64, data: DocData) -> Arc<DocData> {
        let data = Arc::new(data);
        self.docs.insert(doc, (revision, data.clone()));
        data
    }

    /// Olvida los datos de un documento cerrado.
    pub fn forget(&mut self, doc: u64) {
        self.docs.remove(&doc);
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use crate::backend::{GenerateRequest, LlmBackend};

    use super::*;

    struct Fake;
    impl LlmBackend for Fake {
        fn generate(&self, _: &GenerateRequest, _: &mut dyn FnMut(&str) -> bool) -> Result<String> {
            Ok("ok".into())
        }
        fn context_tokens(&self) -> usize {
            4096
        }
    }

    fn session(dir: &Path) -> Session {
        let loader: Loader = Box::new(|_| Ok(Assistant::with_backend(Arc::new(Fake), AssistantConfig::default())));
        Session::at(dir.join("models"), loader)
    }

    #[test]
    fn starts_disabled_and_refuses_to_run() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = session(dir.path());
        assert!(!s.config().enabled);
        assert!(s.assistant().is_err());
    }

    #[test]
    fn settings_persist_and_model_must_be_downloaded() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = session(dir.path());
        s.set_enabled(true).unwrap();
        assert!(s.assistant().err().unwrap().to_string().contains("modelo"));
        assert!(s.select_model("qwen3-4b-instruct-2507-q4").is_err());
        assert!(s.select_model("no-existe").is_err());
        assert!(session(dir.path()).config().enabled, "los ajustes se recuerdan");
    }

    #[test]
    fn loads_once_and_disabling_unloads() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = session(dir.path());
        s.set_enabled(true).unwrap();
        // Un archivo del tamaño exacto cuenta como descargado (la huella se verificó al descargar).
        let spec = ModelSpec::find("qwen2.5-1.5b-instruct-q4").unwrap();
        std::fs::create_dir_all(s.models_dir()).unwrap();
        let f = std::fs::File::create(s.models_dir().join(spec.file_name)).unwrap();
        f.set_len(spec.size_bytes).unwrap();
        s.select_model(spec.id).unwrap();
        let a = s.assistant().unwrap();
        assert!(Arc::ptr_eq(&a, &s.assistant().unwrap()));
        s.set_enabled(false).unwrap();
        assert!(s.assistant().is_err());
    }

    #[test]
    fn doc_data_is_per_revision() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = session(dir.path());
        let text = DocumentText::from_pages(vec!["hola mundo".into()]);
        let index = DocIndex::lexical(&text);
        s.store_doc_data(1, 5, DocData { text, index });
        assert!(s.doc_data(1, 5).is_some());
        assert!(s.doc_data(1, 6).is_none(), "una revisión nueva invalida el índice");
        s.forget(1);
        assert!(s.doc_data(1, 5).is_none());
    }
}
