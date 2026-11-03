use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Aspecto del texto. `font` es una de las 14 fuentes estándar de PDF ("Helvetica", "Times-Roman",
/// "Courier", "Helvetica-Bold"…). El texto se escribe en Latin-1 (cubre el español).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextStyle {
    #[serde(default = "default_font")]
    pub font: String,
    #[serde(default = "default_size")]
    pub size: f32,
    /// Color RGB con componentes de 0 a 1.
    #[serde(default)]
    pub color: [f32; 3],
}

fn default_font() -> String {
    "Helvetica".into()
}
fn default_size() -> f32 {
    10.0
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font: default_font(),
            size: default_size(),
            color: [0.0; 3],
        }
    }
}

/// Textos de la izquierda, el centro y la derecha de una línea. Vacío = nada.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Slots {
    #[serde(default)]
    pub left: String,
    #[serde(default)]
    pub center: String,
    #[serde(default)]
    pub right: String,
}

/// Datos para las variables `{file}` y `{date}`; `{page}` y `{pages}` salen del documento.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StampContext {
    pub file: String,
    pub date: String,
}

impl StampContext {
    /// Contexto con la fecha de hoy (UTC, `AAAA-MM-DD`).
    pub fn today(file: impl Into<String>) -> Self {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        Self {
            file: file.into(),
            date: crate::text::iso_date(secs / 86_400),
        }
    }
}

/// Encabezado y pie con variables `{page}`, `{pages}`, `{date}` y `{file}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeaderFooter {
    #[serde(default)]
    pub header: Slots,
    #[serde(default)]
    pub footer: Slots,
    #[serde(default)]
    pub style: TextStyle,
    /// Distancia en puntos al borde de la página (36 = 0,5 pulgadas).
    #[serde(default = "default_margin")]
    pub margin: f32,
}

fn default_margin() -> f32 {
    36.0
}

impl Default for HeaderFooter {
    fn default() -> Self {
        Self {
            header: Slots::default(),
            footer: Slots::default(),
            style: TextStyle::default(),
            margin: default_margin(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WatermarkContent {
    Text {
        text: String,
        style: TextStyle,
    },
    /// Imagen PNG o JPEG; `scale` es la fracción del ancho de página que ocupa (0,5 = la mitad).
    Image {
        path: PathBuf,
        scale: f32,
    },
}

/// Marca de agua centrada en la página.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Watermark {
    pub content: WatermarkContent,
    /// De 0 (invisible) a 1 (opaca).
    #[serde(default = "default_opacity")]
    pub opacity: f32,
    /// Grados en sentido antihorario; solo para texto.
    #[serde(default = "default_rotation")]
    pub rotation: f32,
    /// Detrás del contenido (visible solo donde la página es transparente) o delante.
    #[serde(default)]
    pub behind: bool,
}

fn default_opacity() -> f32 {
    0.3
}
fn default_rotation() -> f32 {
    45.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BackgroundFill {
    Color {
        rgb: [f32; 3],
    },
    /// Imagen PNG o JPEG a página completa.
    Image {
        path: PathBuf,
    },
}

/// Fondo de página, siempre detrás del contenido.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Background {
    pub fill: BackgroundFill,
    #[serde(default = "one")]
    pub opacity: f32,
}

fn one() -> f32 {
    1.0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// Numeración Bates: `<prefijo><número con ceros><sufijo>`, consecutiva en las páginas indicadas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BatesOptions {
    #[serde(default)]
    pub prefix: String,
    #[serde(default)]
    pub suffix: String,
    /// Número de dígitos (se rellena con ceros).
    #[serde(default = "default_digits")]
    pub digits: u32,
    #[serde(default = "start_one")]
    pub start: u64,
    #[serde(default = "default_corner")]
    pub corner: Corner,
    #[serde(default)]
    pub style: TextStyle,
    #[serde(default = "default_margin")]
    pub margin: f32,
}

fn default_digits() -> u32 {
    6
}
fn start_one() -> u64 {
    1
}
fn default_corner() -> Corner {
    Corner::BottomRight
}

impl Default for BatesOptions {
    fn default() -> Self {
        Self {
            prefix: String::new(),
            suffix: String::new(),
            digits: default_digits(),
            start: 1,
            corner: default_corner(),
            style: TextStyle::default(),
            margin: default_margin(),
        }
    }
}
