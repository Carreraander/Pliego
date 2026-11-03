use serde::{Deserialize, Serialize};

/// Rectángulo `[x0, y0, x1, y1]`.
pub type RectPts = [f32; 4];
/// Color RGB con componentes de 0 a 1.
pub type Rgb = [f32; 3];

/// Familia de fuente estándar usada al componer texto nuevo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FontFamily {
    /// Sin remate (Helvetica).
    #[default]
    Sans,
    /// Con remate (Times).
    Serif,
    /// Monoespaciada (Courier).
    Mono,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
}

/// Estilo del texto. Lo que queda en `None` se toma del bloque original (al reemplazar) o de un valor
/// por defecto (Helvetica 12 pt negro al insertar).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Style {
    pub family: Option<FontFamily>,
    pub size: Option<f32>,
    pub color: Option<Rgb>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub align: Align,
}

/// Bloque de texto editable (aproximadamente un párrafo).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextBlock {
    /// Página (desde 0).
    pub page: usize,
    /// Identificador del bloque dentro de la página; vale mientras la página no cambie.
    pub id: usize,
    pub bbox: RectPts,
    /// Texto del bloque con saltos de línea entre líneas.
    pub text: String,
    pub lines: Vec<String>,
    /// Nombre de la fuente original, si se conoce.
    pub font_name: Option<String>,
    pub font_size: f32,
    pub color: Rgb,
    pub bold: bool,
    pub italic: bool,
    pub family: FontFamily,
}

/// Destino de un enlace.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LinkTarget {
    /// Dirección externa (`https://`, `mailto:`, ...).
    Uri { uri: String },
    /// Página del propio documento (desde 0).
    Page { page: usize },
}
