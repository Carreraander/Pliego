//! Servicio de documentos de la app de Pliego, independiente de Tauri.
//!
//! Los PDF se abren en el proceso aislado `pliego-worker` (si el motor cae con un archivo dañado, la app sigue viva y
//! el worker se relanza). La caché de teselas es la de `pliego_worker::TileCache`. La capa Tauri
//! (`app/src-tauri`) solo traduce comandos y el protocolo `pliego://tile/...` a llamadas de este servicio.

pub mod actions;
pub mod annot;
pub mod api;
pub mod checks;
pub mod convert;
pub mod facts;
pub mod formdesign;
pub mod measure;
pub mod optimize;
pub mod organize;
pub mod outputpreview;
pub mod pageedit;
pub mod portfolio;
pub mod protect;
pub mod search_index;
pub mod sign;
pub mod stamp;
pub mod stamps;
pub mod uri;

/// Ayudantes solo para pruebas.
#[cfg(test)]
pub(crate) mod test_support {
    use std::sync::{Mutex, MutexGuard};

    static SERIAL: Mutex<()> = Mutex::new(());

    /// Las pruebas que arrancan el worker (con tiempo límite por petición) o que trabajan con el motor a fondo
    /// (comprimir, convertir…) se ejecutan de una en una: en paralelo, la carga de CPU de las pesadas hacía
    /// que el worker tardara más que su límite en la CI de Windows («el motor tardó demasiado en responder»).
    pub fn serial() -> MutexGuard<'static, ()> {
        SERIAL.lock().unwrap_or_else(|e| e.into_inner())
    }
}

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use pliego_core::{
    DocInfo, EngineDocument, Error, Layer, OutlineItem, PageSize, PdfEngine, Result, RgbaImage, SearchHit, TileRequest,
};
use pliego_worker::{WorkerConfig, WorkerDocument, WorkerEngine};
use serde::{Deserialize, Serialize};

/// Identificador de un documento abierto (el del motor, que también identifica sus teselas en caché).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DocId(pub u64);

/// Evento `document-changed`: otro módulo modificó el documento y las teselas deben recargarse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentChanged {
    pub doc_id: DocId,
    pub revision: u64,
}

type Listener = Box<dyn Fn(DocumentChanged) + Send + Sync>;

pub struct DocumentService {
    engine: WorkerEngine,
    docs: Mutex<HashMap<DocId, Arc<WorkerDocument>>>,
    listener: Option<Listener>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl DocumentService {
    pub fn new(config: WorkerConfig) -> Self {
        Self {
            engine: WorkerEngine::new(config),
            docs: Mutex::new(HashMap::new()),
            listener: None,
        }
    }

    /// Localiza `pliego-worker` (variable `PLIEGO_WORKER` o junto al ejecutable de la app).
    pub fn discover() -> Result<Self> {
        Ok(Self::new(WorkerConfig::discover()?))
    }

    /// Registra el receptor de `document-changed` (la capa Tauri lo reenvía como evento).
    pub fn on_document_changed(mut self, f: impl Fn(DocumentChanged) + Send + Sync + 'static) -> Self {
        self.listener = Some(Box::new(f));
        self
    }

    fn doc(&self, doc: DocId) -> Result<Arc<WorkerDocument>> {
        lock(&self.docs)
            .get(&doc)
            .cloned()
            .ok_or_else(|| Error::InvalidInput(format!("el documento {} no está abierto", doc.0)))
    }

    /// Abre un documento en el worker. Devuelve `Error::Password` si hace falta contraseña.
    pub fn open_document(&self, path: &Path, password: Option<&str>) -> Result<DocId> {
        let t0 = std::time::Instant::now();
        let doc = self.engine.open(path, password)?;
        trace("open_document", t0);
        let id = DocId(doc.id());
        lock(&self.docs).insert(id, Arc::new(doc));
        Ok(id)
    }

    pub fn close_document(&self, doc: DocId) -> Result<()> {
        // Al soltar el último `Arc` se cierra el worker y se limpia su caché.
        lock(&self.docs)
            .remove(&doc)
            .map(drop)
            .ok_or_else(|| Error::InvalidInput(format!("el documento {} no está abierto", doc.0)))
    }

    pub fn document_info(&self, doc: DocId) -> Result<DocInfo> {
        self.doc(doc)?.info()
    }

    pub fn page_sizes(&self, doc: DocId) -> Result<Vec<PageSize>> {
        let t0 = std::time::Instant::now();
        let d = self.doc(doc)?;
        let sizes = (0..d.page_count()?).map(|p| d.page_size(p)).collect();
        trace("page_sizes", t0);
        sizes
    }

    pub fn outline(&self, doc: DocId) -> Result<Vec<OutlineItem>> {
        self.doc(doc)?.outline()
    }

    /// Capas del documento (vacío si no tiene).
    pub fn layers(&self, doc: DocId) -> Result<Vec<Layer>> {
        self.doc(doc)?.layers()
    }

    /// Muestra u oculta una capa solo en pantalla (el archivo no cambia) y repinta el documento.
    pub fn set_layer_visible(&self, doc: DocId, layer: i32, visible: bool) -> Result<u64> {
        self.doc(doc)?.set_layer_visible(layer, visible)?;
        self.bump_revision(doc)
    }

    pub fn page_text(&self, doc: DocId, page: u32) -> Result<String> {
        self.doc(doc)?.page_text(page)
    }

    /// Busca `needle` en todo el documento. Devuelve solo páginas con coincidencias y se detiene en `max_total`.
    pub fn search(&self, doc: DocId, needle: &str, max_total: u32) -> Result<Vec<SearchHit>> {
        if needle.trim().is_empty() {
            return Ok(Vec::new());
        }
        let d = self.doc(doc)?;
        let mut out = Vec::new();
        let mut total = 0u32;
        for page in 0..d.page_count()? {
            if total >= max_total {
                break;
            }
            let hit = d.search_page(page, needle, max_total - total)?;
            if !hit.quads.is_empty() {
                total += hit.quads.len() as u32;
                out.push(hit);
            }
        }
        Ok(out)
    }

    /// Revisión actual del documento (empieza en 0).
    pub fn revision(&self, doc: DocId) -> Result<u64> {
        Ok(self.doc(doc)?.revision())
    }

    /// Sube la revisión (otro módulo modificó el documento), invalida teselas y emite `document-changed`.
    pub fn bump_revision(&self, doc: DocId) -> Result<u64> {
        let d = self.doc(doc)?;
        let revision = d.revision() + 1;
        d.set_revision(revision);
        if let Some(f) = &self.listener {
            f(DocumentChanged { doc_id: doc, revision });
        }
        Ok(revision)
    }

    /// Sustituye el contenido del documento por `bytes` (tras una edición) y sube la revisión.
    pub fn reload_bytes(&self, doc: DocId, bytes: &[u8]) -> Result<u64> {
        let d = self.doc(doc)?;
        let revision = d.revision() + 1;
        d.reload_bytes(bytes, revision)?;
        if let Some(f) = &self.listener {
            f(DocumentChanged { doc_id: doc, revision });
        }
        Ok(revision)
    }

    /// PNG de una tesela; el RGBA sale de la caché compartida del motor.
    pub fn tile(&self, doc: DocId, req: TileRequest) -> Result<Vec<u8>> {
        let t0 = std::time::Instant::now();
        let img = self.doc(doc)?.render_tile_cached(req)?;
        trace("tile_render", t0);
        let t1 = std::time::Instant::now();
        let png = encode_png(&img);
        trace("tile_png", t1);
        png
    }

    /// Atiende una URI `pliego://tile/<doc>/<page>/<zoom>/<col>/<row>` y devuelve el PNG.
    pub fn tile_from_uri(&self, uri: &str) -> Result<Vec<u8>> {
        let (doc, req) = uri::parse_tile_uri(uri)?;
        self.tile(doc, req)
    }

    /// Teselas RGBA en la caché del motor (para pruebas y diagnóstico).
    pub fn cached_tiles(&self) -> usize {
        self.engine.cache().len()
    }
}

/// Codifica RGBA a PNG con compresión rápida (las teselas se piden en cuanto se desplaza la vista).
fn encode_png(img: &RgbaImage) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(img.data.len() / 4);
    let fail = |e: png::EncodingError| Error::Other(format!("no se pudo codificar la tesela: {e}"));
    let mut enc = png::Encoder::new(&mut out, img.width, img.height);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_compression(png::Compression::Fast);
    let mut w = enc.write_header().map_err(fail)?;
    w.write_image_data(&img.data).map_err(fail)?;
    w.finish().map_err(fail)?;
    Ok(out)
}

#[cfg(test)]
mod tests;

/// Con `PLIEGO_TRACE` definida escribe en stderr cuánto tarda cada paso (diagnóstico de rendimiento).
fn trace(step: &str, since: std::time::Instant) {
    if std::env::var_os("PLIEGO_TRACE").is_some() {
        eprintln!(
            "[pliego-trace] {step}: {:.1} ms",
            since.elapsed().as_secs_f64() * 1000.0
        );
    }
}
