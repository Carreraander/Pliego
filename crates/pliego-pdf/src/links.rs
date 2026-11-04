//! Enlaces y destinos.

use crate::geom::Rect;

/// Cómo se muestra la página de destino.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DestinationKind {
    Fit,
    FitH {
        top: Option<f32>,
    },
    FitV {
        left: Option<f32>,
    },
    XYZ {
        left: Option<f32>,
        top: Option<f32>,
        zoom: Option<f32>,
    },
    FitR {
        left: f32,
        bottom: f32,
        right: f32,
        top: f32,
    },
    FitB,
    FitBH {
        top: Option<f32>,
    },
    FitBV {
        left: Option<f32>,
    },
}

/// Destino interno: página (desde 0) y vista.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinkDestination {
    pub page: u32,
    pub kind: DestinationKind,
}

/// Enlace de una página tal como lo ve el lector.
#[derive(Debug, Clone, PartialEq)]
pub struct Link {
    pub bounds: Rect,
    /// Destino interno si el enlace va a otra página del documento.
    pub dest: Option<LinkDestination>,
    /// URI (externa, o `#page=N` en documentos que no son PDF).
    pub uri: String,
}

/// Adónde lleva un enlace nuevo.
#[derive(Debug, Clone, PartialEq)]
pub enum LinkTarget {
    /// Dirección externa; debe ser ASCII.
    Uri(String),
    /// Página del propio documento (desde 0).
    Page { page: u32, kind: DestinationKind },
}

/// Enlace por crear con [`crate::PdfPage::add_links`].
#[derive(Debug, Clone, PartialEq)]
pub struct NewLink {
    pub bounds: Rect,
    pub target: LinkTarget,
}
