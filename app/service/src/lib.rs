//! Servicio de documentos de la app de Pliego, independiente de Tauri.
//!
//! Cada documento abierto vive en un hilo propio (los objetos de MuPDF no son `Send`) y se le habla por
//! canales. La capa Tauri (`app/src-tauri`) solo traduce comandos y el protocolo `pliego://tile/...` a
//! llamadas de este servicio.

mod cache;
mod worker;

pub mod api;
pub mod uri;

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use pliego_core::{DocInfo, EngineDocument, Error, OutlineItem, PageSize, Result, SearchHit, TileRequest};
use serde::{Deserialize, Serialize};

use cache::{TileCache, TileKey};
use worker::DocWorker;

/// Identificador de un documento abierto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DocId(pub u64);

/// Evento `document-changed`: otro módulo modificó el documento y las teselas deben recargarse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentChanged {
    pub doc_id: DocId,
    pub revision: u64,
}

type Listener = Box<dyn Fn(DocumentChanged) + Send + Sync>;

const DEFAULT_CACHE_BYTES: usize = 64 * 1024 * 1024;

pub struct DocumentService {
    next_id: AtomicU64,
    docs: Mutex<HashMap<DocId, Arc<DocWorker>>>,
    cache: Mutex<TileCache>,
    listener: Option<Listener>,
}

impl Default for DocumentService {
    fn default() -> Self {
        Self::new()
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl DocumentService {
    pub fn new() -> Self {
        Self::with_cache_bytes(DEFAULT_CACHE_BYTES)
    }

    pub fn with_cache_bytes(max_bytes: usize) -> Self {
        Self {
            next_id: AtomicU64::new(1),
            docs: Mutex::new(HashMap::new()),
            cache: Mutex::new(TileCache::new(max_bytes)),
            listener: None,
        }
    }

    /// Registra el receptor de `document-changed` (la capa Tauri lo reenvía como evento).
    pub fn on_document_changed(mut self, f: impl Fn(DocumentChanged) + Send + Sync + 'static) -> Self {
        self.listener = Some(Box::new(f));
        self
    }

    fn worker(&self, doc: DocId) -> Result<Arc<DocWorker>> {
        lock(&self.docs)
            .get(&doc)
            .cloned()
            .ok_or_else(|| Error::InvalidInput(format!("el documento {} no está abierto", doc.0)))
    }

    /// Abre un documento en un hilo propio. Devuelve `Error::Password` si hace falta contraseña.
    pub fn open_document(&self, path: &Path, password: Option<&str>) -> Result<DocId> {
        let worker = DocWorker::spawn(path.to_path_buf(), password.map(str::to_owned))?;
        let id = DocId(self.next_id.fetch_add(1, Ordering::Relaxed));
        lock(&self.docs).insert(id, Arc::new(worker));
        Ok(id)
    }

    pub fn close_document(&self, doc: DocId) -> Result<()> {
        lock(&self.docs)
            .remove(&doc)
            .ok_or_else(|| Error::InvalidInput(format!("el documento {} no está abierto", doc.0)))?;
        lock(&self.cache).drop_doc(doc);
        Ok(())
    }

    pub fn document_info(&self, doc: DocId) -> Result<DocInfo> {
        self.worker(doc)?.call(|d| d.info())
    }

    pub fn page_sizes(&self, doc: DocId) -> Result<Vec<PageSize>> {
        self.worker(doc)?
            .call(|d| (0..d.page_count()?).map(|p| d.page_size(p)).collect())
    }

    pub fn outline(&self, doc: DocId) -> Result<Vec<OutlineItem>> {
        self.worker(doc)?.call(|d| d.outline())
    }

    pub fn page_text(&self, doc: DocId, page: u32) -> Result<String> {
        self.worker(doc)?.call(move |d| d.page_text(page))
    }

    /// Busca `needle` en todo el documento. Devuelve solo páginas con coincidencias y se detiene en `max_total`.
    pub fn search(&self, doc: DocId, needle: &str, max_total: u32) -> Result<Vec<SearchHit>> {
        if needle.trim().is_empty() {
            return Ok(Vec::new());
        }
        let needle = needle.to_owned();
        self.worker(doc)?.call(move |d| {
            let mut out = Vec::new();
            let mut total = 0u32;
            for page in 0..d.page_count()? {
                if total >= max_total {
                    break;
                }
                let hit = d.search_page(page, &needle, max_total - total)?;
                if !hit.quads.is_empty() {
                    total += hit.quads.len() as u32;
                    out.push(hit);
                }
            }
            Ok(out)
        })
    }

    /// Revisión actual del documento (empieza en 0).
    pub fn revision(&self, doc: DocId) -> Result<u64> {
        Ok(self.worker(doc)?.revision())
    }

    /// Sube la revisión (otro módulo modificó el documento), invalida teselas y emite `document-changed`.
    pub fn bump_revision(&self, doc: DocId) -> Result<u64> {
        let revision = self.worker(doc)?.bump_revision();
        if let Some(f) = &self.listener {
            f(DocumentChanged { doc_id: doc, revision });
        }
        Ok(revision)
    }

    /// PNG de una tesela, desde caché si es posible.
    pub fn tile(&self, doc: DocId, req: TileRequest) -> Result<Arc<Vec<u8>>> {
        let worker = self.worker(doc)?;
        let revision = worker.revision();
        let key = TileKey {
            doc,
            revision,
            page: req.page,
            zoom_bits: req.zoom.to_bits(),
            col: req.col,
            row: req.row,
        };
        if let Some(hit) = lock(&self.cache).get(&key) {
            return Ok(hit);
        }
        let png = Arc::new(worker.call(move |d| {
            let img = d.render_tile(req)?;
            pliego_engine_mupdf::encode_png(&img)
        })?);
        // Si el documento cambió mientras se renderizaba, la tesela ya es obsoleta: no se cachea.
        if worker.revision() == revision {
            lock(&self.cache).insert(key, png.clone());
        }
        Ok(png)
    }

    /// Atiende una URI `pliego://tile/<doc>/<page>/<zoom>/<col>/<row>` y devuelve el PNG.
    pub fn tile_from_uri(&self, uri: &str) -> Result<Arc<Vec<u8>>> {
        let (doc, req) = uri::parse_tile_uri(uri)?;
        self.tile(doc, req)
    }

    /// Número de teselas en caché (para pruebas y diagnóstico).
    pub fn cached_tiles(&self) -> usize {
        lock(&self.cache).len()
    }
}

#[cfg(test)]
mod tests;
