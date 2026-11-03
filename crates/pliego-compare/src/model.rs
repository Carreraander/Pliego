//! Tipos públicos del resultado de una comparación.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Rectángulo en puntos PDF con origen arriba a la izquierda de la página (ya con la rotación aplicada).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

impl Rect {
    pub fn width(&self) -> f32 {
        self.x1 - self.x0
    }
    pub fn height(&self) -> f32 {
        self.y1 - self.y0
    }
    pub fn union(&self, o: &Rect) -> Rect {
        Rect {
            x0: self.x0.min(o.x0),
            y0: self.y0.min(o.y0),
            x1: self.x1.max(o.x1),
            y1: self.y1.max(o.y1),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    /// Texto que solo existe en la versión nueva (verde).
    Added,
    /// Texto que solo existe en la versión original (rojo).
    Removed,
    /// Texto reemplazado o con otro formato (ámbar).
    Changed,
}

/// Un cambio de texto dentro de una página emparejada.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextChange {
    pub kind: ChangeKind,
    /// Página (desde 0) del documento original.
    pub page_a: u32,
    /// Página (desde 0) del documento nuevo.
    pub page_b: u32,
    pub text_a: String,
    pub text_b: String,
    /// Posiciones en la página original (una por línea afectada).
    pub rects_a: Vec<Rect>,
    /// Posiciones en la página nueva.
    pub rects_b: Vec<Rect>,
    /// `true` si el texto es igual y solo cambió la fuente o el tamaño.
    pub format_only: bool,
    /// Descripción legible del cambio de formato, si lo hay.
    pub note: Option<String>,
}

/// Región de la página donde los píxeles difieren (coordenadas comunes a ambas versiones).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct VisualRegion {
    pub rect: Rect,
    pub changed_pixels: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PageStatus {
    /// La página existe en ambas versiones (puede tener cambios o no).
    Matched,
    /// Página nueva: solo existe en la versión nueva.
    Inserted,
    /// Página eliminada: solo existe en la versión original.
    Deleted,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageComparison {
    pub status: PageStatus,
    pub page_a: Option<u32>,
    pub page_b: Option<u32>,
    /// Parecido del contenido entre 0 y 1 (1 para páginas insertadas/eliminadas no aplica: 0).
    pub similarity: f32,
    pub text_changes: Vec<TextChange>,
    pub visual_regions: Vec<VisualRegion>,
}

impl PageComparison {
    /// ¿Hay alguna diferencia en esta página?
    pub fn has_differences(&self) -> bool {
        self.status != PageStatus::Matched || !self.text_changes.is_empty() || !self.visual_regions.is_empty()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Summary {
    pub pages_a: u32,
    pub pages_b: u32,
    pub inserted_pages: u32,
    pub deleted_pages: u32,
    /// Páginas emparejadas con alguna diferencia.
    pub changed_pages: u32,
    pub added_words: u32,
    pub removed_words: u32,
    /// Palabras de la versión nueva que sustituyen a otras o cambian de formato.
    pub changed_words: u32,
    pub format_changes: u32,
    pub visual_regions: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Comparison {
    pub path_a: PathBuf,
    pub path_b: PathBuf,
    pub summary: Summary,
    /// Páginas en orden de lectura de la versión nueva, con las eliminadas intercaladas.
    pub pages: Vec<PageComparison>,
    #[serde(skip)]
    pub(crate) password_a: Option<String>,
    #[serde(skip)]
    pub(crate) password_b: Option<String>,
}

impl Comparison {
    /// ¿Los documentos son equivalentes según las opciones usadas?
    pub fn is_identical(&self) -> bool {
        !self.pages.iter().any(PageComparison::has_differences)
    }
}
