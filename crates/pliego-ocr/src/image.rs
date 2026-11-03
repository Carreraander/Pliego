use serde::{Deserialize, Serialize};

use pliego_core::{Error, Result};

/// Imagen en escala de grises de 8 bits, fila a fila, origen arriba a la izquierda.
#[derive(Debug, Clone)]
pub struct GrayImage {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl GrayImage {
    pub fn new(width: u32, height: u32, data: Vec<u8>) -> Result<Self> {
        if data.len() != (width as usize) * (height as usize) {
            return Err(Error::InvalidInput(
                "el tamaño de la imagen no coincide con sus datos".into(),
            ));
        }
        Ok(Self { width, height, data })
    }

    /// Imagen del color indicado (255 = blanco).
    pub fn filled(width: u32, height: u32, value: u8) -> Self {
        Self {
            width,
            height,
            data: vec![value; (width as usize) * (height as usize)],
        }
    }

    pub fn get(&self, x: u32, y: u32) -> u8 {
        self.data[(y as usize) * (self.width as usize) + x as usize]
    }

    /// Convierte RGB (3 bytes por píxel) o RGBA (4) a gris con la luminosidad estándar.
    pub fn from_interleaved(width: u32, height: u32, channels: usize, pixels: &[u8]) -> Result<Self> {
        let n = (width as usize) * (height as usize);
        if !matches!(channels, 1 | 3 | 4) || pixels.len() != n * channels {
            return Err(Error::InvalidInput("formato de píxeles no válido para OCR".into()));
        }
        let data = match channels {
            1 => pixels.to_vec(),
            _ => pixels
                .chunks_exact(channels)
                .map(|p| ((p[0] as u32 * 299 + p[1] as u32 * 587 + p[2] as u32 * 114) / 1000) as u8)
                .collect(),
        };
        Ok(Self { width, height, data })
    }
}

/// Rectángulo en píxeles (`x1`, `y1` exclusivos).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PixelRect {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

/// Palabra reconocida.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Word {
    pub text: String,
    pub bbox: PixelRect,
    /// Confianza del motor, de 0 a 100.
    pub confidence: f32,
}
