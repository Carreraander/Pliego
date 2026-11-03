//! Tipos serializables de anotaciones. Las coordenadas son puntos PDF en el espacio de página de MuPDF
//! (origen arriba a la izquierda, y hacia abajo, ya con la rotación de página aplicada).

use serde::{Deserialize, Serialize};

/// Identificador estable de una anotación: página (desde 0) y número de objeto PDF.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AnnotId {
    pub page: usize,
    pub xref: i32,
}

/// Rectángulo `[x0, y0, x1, y1]`.
pub type RectPts = [f32; 4];
/// Punto `[x, y]`.
pub type PointPts = [f32; 2];
/// Cuadrilátero de texto `[ul.x, ul.y, ur.x, ur.y, ll.x, ll.y, lr.x, lr.y]`.
pub type QuadPts = [f32; 8];
/// Color RGB con componentes de 0 a 1.
pub type Rgb = [f32; 3];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineEnd {
    #[default]
    None,
    Square,
    Circle,
    Diamond,
    OpenArrow,
    ClosedArrow,
    Butt,
    Slash,
}

/// Forma de la anotación que se va a crear.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Geometry {
    /// Nota adhesiva (icono) en un punto.
    Note {
        at: PointPts,
    },
    /// Texto libre en un recuadro.
    FreeText {
        rect: RectPts,
        font_size: f32,
    },
    Highlight {
        quads: Vec<QuadPts>,
    },
    Underline {
        quads: Vec<QuadPts>,
    },
    StrikeOut {
        quads: Vec<QuadPts>,
    },
    Squiggly {
        quads: Vec<QuadPts>,
    },
    /// Línea; con `end: closed_arrow` es una flecha.
    Line {
        start: PointPts,
        end: PointPts,
        start_style: LineEnd,
        end_style: LineEnd,
    },
    Square {
        rect: RectPts,
    },
    Circle {
        rect: RectPts,
    },
    Polygon {
        points: Vec<PointPts>,
    },
    PolyLine {
        points: Vec<PointPts>,
    },
    /// Lápiz: uno o más trazos.
    Ink {
        strokes: Vec<Vec<PointPts>>,
    },
}

/// Resultado de una revisión (respuesta de estado).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewState {
    Accepted,
    Rejected,
    Cancelled,
    Completed,
    None,
}

impl ReviewState {
    pub(crate) fn pdf_name(self) -> &'static str {
        match self {
            ReviewState::Accepted => "Accepted",
            ReviewState::Rejected => "Rejected",
            ReviewState::Cancelled => "Cancelled",
            ReviewState::Completed => "Completed",
            ReviewState::None => "None",
        }
    }
    pub(crate) fn from_pdf_name(s: &str) -> Option<Self> {
        Some(match s {
            "Accepted" => ReviewState::Accepted,
            "Rejected" => ReviewState::Rejected,
            "Cancelled" => ReviewState::Cancelled,
            "Completed" => ReviewState::Completed,
            "None" => ReviewState::None,
            _ => return None,
        })
    }
}

/// Propiedades comunes de aspecto y metadatos.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Style {
    pub color: Option<Rgb>,
    /// Color de relleno (cuadrado, círculo, polígono, extremos de línea).
    pub interior_color: Option<Rgb>,
    pub opacity: Option<f32>,
    pub border_width: Option<f32>,
}

/// Qué crear.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnnotSpec {
    /// Página (desde 0).
    pub page: usize,
    pub geometry: Geometry,
    #[serde(default)]
    pub style: Style,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub contents: Option<String>,
    /// Si se indica, la anotación es una respuesta a esta otra (IRT).
    pub reply_to: Option<AnnotId>,
    /// Estado de revisión (solo tiene sentido en respuestas).
    pub state: Option<ReviewState>,
}

impl AnnotSpec {
    pub fn new(page: usize, geometry: Geometry) -> Self {
        Self {
            page,
            geometry,
            style: Style::default(),
            author: None,
            subject: None,
            contents: None,
            reply_to: None,
            state: None,
        }
    }
}

/// Cambios sobre una anotación existente. Los campos `None` no se tocan.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AnnotPatch {
    pub style: Style,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub contents: Option<String>,
    /// Desplazamiento `[dx, dy]` en puntos.
    pub move_by: Option<PointPts>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnnotKind {
    Note,
    FreeText,
    Highlight,
    Underline,
    StrikeOut,
    Squiggly,
    Line,
    Square,
    Circle,
    Polygon,
    PolyLine,
    Ink,
    Stamp,
    FileAttachment,
    Link,
    Caret,
    Redact,
    Other,
}

/// Lo que devuelve `list`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnnotInfo {
    pub id: AnnotId,
    pub kind: AnnotKind,
    pub rect: RectPts,
    pub style: Style,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub contents: Option<String>,
    /// Fecha de creación en formato PDF (`D:AAAAMMDDhhmmss...`).
    pub created: Option<String>,
    pub modified: Option<String>,
    pub quads: Vec<QuadPts>,
    pub points: Vec<PointPts>,
    pub strokes: Vec<Vec<PointPts>>,
    pub line: Option<(PointPts, PointPts)>,
    pub reply_to: Option<i32>,
    pub state: Option<ReviewState>,
}
