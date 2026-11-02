//! Abstracción del motor de render. La implementación por defecto es `pliego-engine-mupdf`.
//!
//! Los documentos del motor NO son `Send` (MuPDF usa contextos por hilo): la app abre cada
//! documento en un hilo dedicado y le envía peticiones por canal.

use std::path::Path;

use crate::Result;

/// Tamaño de página en puntos PDF (1/72 de pulgada), ya con la rotación aplicada.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PageSize {
    pub width: f32,
    pub height: f32,
}

/// Petición de tesela: región cuadrada de `size` píxeles en la posición de rejilla (`col`, `row`)
/// de la página `page` renderizada a `zoom` (1.0 = 72 ppp).
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TileRequest {
    pub page: u32,
    pub zoom: f32,
    pub col: u32,
    pub row: u32,
    pub size: u32,
}

/// Imagen RGBA de 8 bits por canal, filas contiguas sin relleno.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbaImage {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct OutlineItem {
    pub title: String,
    /// Índice de página desde 0, si el destino es interno.
    pub page: Option<u32>,
    pub uri: Option<String>,
    pub children: Vec<OutlineItem>,
}

/// Coincidencia de búsqueda: cuadriláteros en puntos PDF `[ul.x, ul.y, ur.x, ur.y, ll.x, ll.y, lr.x, lr.y]`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SearchHit {
    pub page: u32,
    pub quads: Vec<[f32; 8]>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DocInfo {
    pub page_count: u32,
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub keywords: Option<String>,
    pub creator: Option<String>,
    pub producer: Option<String>,
    pub format: Option<String>,
    pub encryption: Option<String>,
    pub is_pdf: bool,
}

/// Fábrica de documentos.
pub trait PdfEngine {
    type Doc: EngineDocument;
    fn open(&self, path: &Path, password: Option<&str>) -> Result<Self::Doc>;
    fn open_bytes(&self, bytes: &[u8], password: Option<&str>) -> Result<Self::Doc>;
}

/// Operaciones de solo lectura que necesita el visor.
pub trait EngineDocument {
    fn info(&self) -> Result<DocInfo>;
    fn page_count(&self) -> Result<u32>;
    fn page_size(&self, page: u32) -> Result<PageSize>;
    /// Página completa a `zoom`. Incluye anotaciones y widgets.
    fn render_page(&self, page: u32, zoom: f32) -> Result<RgbaImage>;
    /// Una tesela; las teselas del borde pueden ser más pequeñas que `size`.
    fn render_tile(&self, req: TileRequest) -> Result<RgbaImage>;
    fn page_text(&self, page: u32) -> Result<String>;
    fn search_page(&self, page: u32, needle: &str, max_hits: u32) -> Result<SearchHit>;
    fn outline(&self) -> Result<Vec<OutlineItem>>;
}
