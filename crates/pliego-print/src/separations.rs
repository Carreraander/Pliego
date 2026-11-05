use pliego_core::{Error, Result};
use pliego_pdf::{Colorspace, ImageFormat, Matrix, PdfDocument, Pixmap};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlateChannel {
    Cyan,
    Magenta,
    Yellow,
    Black,
}

impl PlateChannel {
    pub const ALL: [PlateChannel; 4] = [Self::Cyan, Self::Magenta, Self::Yellow, Self::Black];

    pub fn name(self) -> &'static str {
        match self {
            Self::Cyan => "Cyan",
            Self::Magenta => "Magenta",
            Self::Yellow => "Yellow",
            Self::Black => "Black",
        }
    }
}

/// Plancha en escala de grises: 0 = sin tinta, 255 = tinta al 100 %.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plate {
    pub channel: PlateChannel,
    pub width: u32,
    pub height: u32,
    pub samples: Vec<u8>,
}

impl Plate {
    /// Cobertura media de la plancha (0.0 a 1.0).
    pub fn coverage(&self) -> f32 {
        if self.samples.is_empty() {
            return 0.0;
        }
        self.samples.iter().map(|&v| v as u64).sum::<u64>() as f32 / (255.0 * self.samples.len() as f32)
    }

    /// PNG en escala de grises como una película de imprenta: negro donde hay tinta, blanco donde no.
    pub fn to_png(&self) -> Result<Vec<u8>> {
        let mut pix = Pixmap::new_with_w_h(Colorspace::Gray, self.width as i32, self.height as i32, false)?;
        let stride = pix.stride();
        let w = self.width as usize;
        let out = pix.samples_mut();
        for (y, row) in self.samples.chunks(w).enumerate() {
            for (x, v) in row.iter().enumerate() {
                out[y * stride + x] = 255 - v;
            }
        }
        let mut png = Vec::new();
        pix.write_to(&mut png, ImageFormat::Png)?;
        Ok(png)
    }
}

/// Planchas CMYK de una página y la cobertura total de tinta (TAC) máxima.
#[derive(Debug, Clone, PartialEq)]
pub struct Separations {
    pub plates: Vec<Plate>,
    /// Suma máxima de C+M+Y+K en un píxel (0.0 a 4.0); 3.0 equivale al 300 %.
    pub max_total_ink: f32,
}

/// Vista previa de separaciones de la página `page` (desde 0) a `dpi` ppp.
///
/// Se renderiza la página en CMYK y se reparte en cuatro planchas. Los colores directos se convierten por su
/// alternativa CMYK, así que aparecen repartidos en las planchas de proceso (ver [`crate::inks`] para listarlos).
pub fn separations(doc: &PdfDocument, page: usize, dpi: f32) -> Result<Separations> {
    if !(dpi.is_finite() && dpi > 0.0) {
        return Err(Error::InvalidInput("la resolución debe ser un número positivo".into()));
    }
    let count = doc.page_count()?.max(0) as usize;
    if page >= count {
        return Err(Error::InvalidInput(format!(
            "la página {} no existe (el documento tiene {count})",
            page + 1
        )));
    }
    let p = doc.load_page(page as i32)?;
    let scale = dpi / 72.0;
    let pix = p.to_pixmap(&Matrix::new_scale(scale, scale), Colorspace::Cmyk, false, true)?;
    let (w, h, n) = (pix.width() as usize, pix.height() as usize, pix.n() as usize);
    if n != 4 {
        return Err(Error::Pdf("el motor no devolvió una imagen CMYK".into()));
    }
    let stride = pix.stride();
    let data = pix.samples();
    let mut planes = vec![vec![0u8; w * h]; 4];
    let mut max_total = 0u32;
    for y in 0..h {
        let row = &data[y * stride..y * stride + w * 4];
        for x in 0..w {
            let px = &row[x * 4..x * 4 + 4];
            let mut sum = 0u32;
            for c in 0..4 {
                planes[c][y * w + x] = px[c];
                sum += px[c] as u32;
            }
            max_total = max_total.max(sum);
        }
    }
    let plates = PlateChannel::ALL
        .into_iter()
        .zip(planes)
        .map(|(channel, samples)| Plate {
            channel,
            width: w as u32,
            height: h as u32,
            samples,
        })
        .collect();
    Ok(Separations {
        plates,
        max_total_ink: max_total as f32 / 255.0,
    })
}
