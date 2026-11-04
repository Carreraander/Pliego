//! Mapas de bits propios. Un [`Pixmap`] son datos planos (sin punteros del motor), con filas contiguas.

use std::io::Write;

use pliego_core::{Error, Result};

use crate::backend::{err, irect_to};
use crate::geom::IRect;

/// Espacio de color de un mapa de bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Colorspace {
    Gray,
    Rgb,
    Bgr,
    Cmyk,
}

impl Colorspace {
    pub fn device_gray() -> Self {
        Self::Gray
    }
    pub fn device_rgb() -> Self {
        Self::Rgb
    }
    pub fn device_bgr() -> Self {
        Self::Bgr
    }
    pub fn device_cmyk() -> Self {
        Self::Cmyk
    }

    /// Número de componentes de color (sin alfa).
    pub fn n(self) -> u8 {
        match self {
            Self::Gray => 1,
            Self::Rgb | Self::Bgr => 3,
            Self::Cmyk => 4,
        }
    }

    pub(crate) fn to_backend(self) -> mupdf::Colorspace {
        match self {
            Self::Gray => mupdf::Colorspace::device_gray(),
            Self::Rgb => mupdf::Colorspace::device_rgb(),
            Self::Bgr => mupdf::Colorspace::device_bgr(),
            Self::Cmyk => mupdf::Colorspace::device_cmyk(),
        }
    }

    pub(crate) fn from_backend(cs: Option<&mupdf::Colorspace>, n: u8, alpha: bool) -> Self {
        match cs {
            Some(c) if c.is_gray() => Self::Gray,
            Some(c) if c.is_cmyk() => Self::Cmyk,
            Some(c) if c.is_rgb() => Self::Rgb,
            _ => match n - u8::from(alpha) {
                1 => Self::Gray,
                4 => Self::Cmyk,
                _ => Self::Rgb,
            },
        }
    }
}

/// Formatos de salida de [`Pixmap::write_to`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Png,
}

/// Imagen de 8 bits por componente. Orden de las muestras: componentes de color y, si `alpha`, el alfa al final.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pixmap {
    width: u32,
    height: u32,
    colorspace: Colorspace,
    alpha: bool,
    samples: Vec<u8>,
}

impl Pixmap {
    /// Mapa de bits a cero (negro/transparente) de `width × height`.
    pub fn new_with_w_h(colorspace: Colorspace, width: i32, height: i32, alpha: bool) -> Result<Self> {
        if width <= 0 || height <= 0 {
            return Err(Error::InvalidInput(format!(
                "tamaño de imagen no válido: {width}x{height}"
            )));
        }
        let n = colorspace.n() as usize + usize::from(alpha);
        let len = (width as usize)
            .checked_mul(height as usize)
            .and_then(|p| p.checked_mul(n))
            .ok_or_else(|| Error::InvalidInput("imagen demasiado grande".into()))?;
        Ok(Self {
            width: width as u32,
            height: height as u32,
            colorspace,
            alpha,
            samples: vec![0; len],
        })
    }

    /// Construye a partir de muestras ya contiguas (`width·height·n` bytes).
    pub fn from_samples(
        colorspace: Colorspace,
        width: u32,
        height: u32,
        alpha: bool,
        samples: Vec<u8>,
    ) -> Result<Self> {
        let n = colorspace.n() as usize + usize::from(alpha);
        if samples.len() != width as usize * height as usize * n {
            return Err(Error::InvalidInput(
                "el número de muestras no corresponde al tamaño".into(),
            ));
        }
        Ok(Self {
            width,
            height,
            colorspace,
            alpha,
            samples,
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
    pub fn colorspace(&self) -> Colorspace {
        self.colorspace
    }
    pub fn alpha(&self) -> bool {
        self.alpha
    }
    /// Componentes por píxel, alfa incluido.
    pub fn n(&self) -> u8 {
        self.colorspace.n() + u8::from(self.alpha)
    }
    /// Bytes por fila (siempre `width · n`: sin relleno).
    pub fn stride(&self) -> usize {
        self.width as usize * self.n() as usize
    }
    pub fn samples(&self) -> &[u8] {
        &self.samples
    }
    pub fn samples_mut(&mut self) -> &mut [u8] {
        &mut self.samples
    }
    pub fn into_samples(self) -> Vec<u8> {
        self.samples
    }

    /// Rellena todas las muestras con `value`.
    pub fn clear_with(&mut self, value: u8) {
        self.samples.fill(value);
    }

    /// Copia a RGBA de 8 bits (gris → RGB; CMYK sin perfil: conversión simple).
    pub fn to_rgba(&self) -> Vec<u8> {
        let n = self.n() as usize;
        let mut out = Vec::with_capacity(self.width as usize * self.height as usize * 4);
        for px in self.samples.chunks_exact(n) {
            let a = if self.alpha { px[n - 1] } else { 255 };
            match self.colorspace {
                Colorspace::Gray => out.extend_from_slice(&[px[0], px[0], px[0], a]),
                Colorspace::Rgb => out.extend_from_slice(&[px[0], px[1], px[2], a]),
                Colorspace::Bgr => out.extend_from_slice(&[px[2], px[1], px[0], a]),
                Colorspace::Cmyk => {
                    let k = 255 - u16::from(px[3]);
                    let ch = |c: u8| ((255 - u16::from(c)) * k / 255) as u8;
                    out.extend_from_slice(&[ch(px[0]), ch(px[1]), ch(px[2]), a]);
                }
            }
        }
        out
    }

    /// Codifica y escribe la imagen. Devuelve los bytes escritos.
    pub fn write_to<W: Write>(&self, w: &mut W, format: ImageFormat) -> Result<u64> {
        let ImageFormat::Png = format;
        let bytes = self.encode_png()?;
        w.write_all(&bytes)?;
        Ok(bytes.len() as u64)
    }

    /// PNG en memoria.
    pub fn encode_png(&self) -> Result<Vec<u8>> {
        let pix = self.to_backend()?;
        let mut out = Vec::new();
        pix.write_to(&mut out, mupdf::ImageFormat::PNG).map_err(err)?;
        Ok(out)
    }

    /// Copia un `mupdf::Pixmap` (quitando el relleno de filas) a un [`Pixmap`] propio.
    pub(crate) fn from_backend(pix: &mupdf::Pixmap) -> Self {
        let (w, h) = (pix.width() as usize, pix.height() as usize);
        let n = pix.n() as usize;
        let alpha = pix.alpha();
        let stride = pix.stride() as usize;
        let src = pix.samples();
        let mut samples = Vec::with_capacity(w * h * n);
        for y in 0..h {
            samples.extend_from_slice(&src[y * stride..y * stride + w * n]);
        }
        Self {
            width: w as u32,
            height: h as u32,
            colorspace: Colorspace::from_backend(pix.color_space().as_ref(), n as u8, alpha),
            alpha,
            samples,
        }
    }

    /// Pasa a un `mupdf::Pixmap` (para codificar o construir una `Image` de MuPDF).
    pub(crate) fn to_backend(&self) -> Result<mupdf::Pixmap> {
        let rect = IRect::new(0, 0, self.width as i32, self.height as i32);
        let mut pix =
            mupdf::Pixmap::new_with_rect(&self.colorspace.to_backend(), irect_to(rect), self.alpha).map_err(err)?;
        let stride = pix.stride() as usize;
        let row = self.stride();
        let dst = pix.samples_mut();
        for y in 0..self.height as usize {
            dst[y * stride..y * stride + row].copy_from_slice(&self.samples[y * row..(y + 1) * row]);
        }
        Ok(pix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgba_and_png() {
        let mut p = Pixmap::new_with_w_h(Colorspace::Rgb, 2, 1, false).unwrap();
        p.samples_mut().copy_from_slice(&[255, 0, 0, 0, 255, 0]);
        assert_eq!(p.to_rgba(), vec![255, 0, 0, 255, 0, 255, 0, 255]);
        assert_eq!(p.stride(), 6);
        let png = p.encode_png().unwrap();
        assert_eq!(&png[..4], b"\x89PNG");
        let g = Pixmap::new_with_w_h(Colorspace::Gray, 1, 1, true).unwrap();
        assert_eq!(g.n(), 2);
        assert!(Pixmap::new_with_w_h(Colorspace::Gray, 0, 1, false).is_err());
    }
}
