//! Descripción de las impresoras: lo que la interfaz necesita para armar el diálogo. Son datos puros; quien los
//! rellena es el módulo de Windows.

use serde::{Deserialize, Serialize};

/// Una impresora instalada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrinterEntry {
    pub name: String,
    /// Es la predeterminada de Windows.
    pub is_default: bool,
}

/// Un tamaño de papel que declara el controlador.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Paper {
    /// Identificador `DMPAPER_*` de Windows (9 = A4, 1 = Carta…).
    pub id: i16,
    pub name: String,
    pub width_mm: f32,
    pub height_mm: f32,
}

/// Qué sabe hacer una impresora.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrinterCaps {
    pub name: String,
    pub color: bool,
    pub duplex: bool,
    /// Copias que hace el propio controlador (1 si no sabe).
    pub max_copies: u32,
    pub collate: bool,
    pub papers: Vec<Paper>,
    /// Papel con el que arranca el controlador.
    pub default_paper: Option<i16>,
}
