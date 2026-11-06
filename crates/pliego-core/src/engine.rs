//! Abstracción del motor de render. La implementación por defecto es `pliego-engine`.
//!
//! Los documentos del motor NO son `Send` (el modelo de objetos y PDFium viven en un solo hilo): la app abre cada
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

/// Palabra de una página con su caja, para la capa de texto del visor (selección, copia y lectores de pantalla).
///
/// `rect` = `[x0, y0, x1, y1]` en puntos del espacio de página: origen arriba a la izquierda y con la rotación de la
/// página aplicada (el mismo sistema que [`SearchHit::quads`] y [`PageSize`]). Las palabras salen en orden de lectura;
/// `block` y `line` (esta, dentro del bloque) sirven para volver a formar líneas y párrafos al copiar.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PageWord {
    pub text: String,
    pub rect: [f32; 4],
    pub block: u32,
    pub line: u32,
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

/// Capa (grupo de contenido opcional) del documento.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Layer {
    /// Identificador estable dentro del documento (número de objeto del grupo).
    pub id: i32,
    pub name: String,
    pub visible: bool,
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
    /// Palabras de la página con su caja, en orden de lectura (vacío si el motor no las da o la página no tiene
    /// texto). Añadido sin romper el contrato anterior.
    fn page_words(&self, _page: u32) -> Result<Vec<PageWord>> {
        Ok(Vec::new())
    }
    fn outline(&self) -> Result<Vec<OutlineItem>>;
    /// Capas del documento (vacío si no tiene o el motor no las soporta). Añadido sin romper el contrato anterior.
    fn layers(&self) -> Result<Vec<Layer>> {
        Ok(Vec::new())
    }
    /// Muestra u oculta una capa solo en memoria: no modifica el archivo. Por defecto no está soportado.
    fn set_layer_visible(&self, _id: i32, _visible: bool) -> Result<()> {
        Err(crate::Error::Unsupported("este motor no admite capas".into()))
    }
}
