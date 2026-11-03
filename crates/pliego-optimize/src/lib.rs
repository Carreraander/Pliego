//! Optimización de PDFs (flujo W08).
//!
//! - [`analyze`]: reparto del tamaño del archivo por categorías (imágenes, fuentes, contenido…).
//! - [`optimize`]: submuestreo y recompresión JPEG de imágenes según un [`Preset`], eliminación de
//!   objetos duplicados y sin referencia, compresión de flujos y linealización opcional.
//!
//! `Preset::Lossless` nunca cambia el render: no toca los píxeles de ninguna imagen.
//! Páginas desde 0 en la API.

mod images;
mod usage;

use std::path::{Path, PathBuf};
use std::str::FromStr;

use mupdf::pdf::{PdfDocument, PdfWriteOptions};
use pliego_core::{Error, JobContext, Report, Result};
use serde::{Deserialize, Serialize};

pub use usage::{ImageUsage, SpaceUsage, analyze, analyze_with_password};

pub(crate) fn pdf_err(e: mupdf::Error) -> Error {
    Error::Pdf(e.to_string())
}

/// Preajustes de optimización.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Preset {
    /// 96 ppp: lo más pequeño, para ver en pantalla.
    #[serde(rename = "pantalla")]
    Screen,
    /// 150 ppp: buen equilibrio para lectores y correo.
    Ebook,
    /// 300 ppp: calidad de impresión.
    #[serde(rename = "impresion")]
    Print,
    /// Solo estructura (duplicados, flujos, objetos sin uso): el render no cambia.
    #[serde(rename = "sin-perdida")]
    Lossless,
}

impl Preset {
    /// Resolución objetivo de las imágenes; `None` = no se tocan.
    pub fn target_dpi(self) -> Option<f32> {
        match self {
            Preset::Screen => Some(96.0),
            Preset::Ebook => Some(150.0),
            Preset::Print => Some(300.0),
            Preset::Lossless => None,
        }
    }
    /// Calidad JPEG (1–100).
    pub fn jpeg_quality(self) -> u8 {
        match self {
            Preset::Screen => 55,
            Preset::Ebook => 72,
            Preset::Print => 85,
            Preset::Lossless => 100,
        }
    }
}

impl FromStr for Preset {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self> {
        match s.trim().to_lowercase().as_str() {
            "pantalla" | "screen" => Ok(Preset::Screen),
            "ebook" => Ok(Preset::Ebook),
            "impresion" | "impresión" | "print" => Ok(Preset::Print),
            "sin-perdida" | "sin-pérdida" | "lossless" => Ok(Preset::Lossless),
            other => Err(Error::InvalidInput(format!(
                "preajuste desconocido: {other:?} (usa pantalla, ebook, impresion o sin-perdida)"
            ))),
        }
    }
}

impl std::fmt::Display for Preset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Preset::Screen => "pantalla",
            Preset::Ebook => "ebook",
            Preset::Print => "impresion",
            Preset::Lossless => "sin-perdida",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OptimizeOptions {
    pub preset: Preset,
    /// Sustituye la resolución objetivo del preajuste.
    pub target_dpi: Option<f32>,
    /// Sustituye la calidad JPEG del preajuste (1–100).
    pub jpeg_quality: Option<u8>,
    /// Pedir linealización para ver por web. Aún no disponible: se avisa y se guarda sin linealizar.
    pub linearize: bool,
    /// Imágenes de menos bytes que esto no se tocan.
    pub min_image_bytes: u64,
    pub password: Option<String>,
}

impl Default for OptimizeOptions {
    fn default() -> Self {
        Self {
            preset: Preset::Ebook,
            target_dpi: None,
            jpeg_quality: None,
            linearize: false,
            min_image_bytes: 4096,
            password: None,
        }
    }
}

/// Qué se hizo con las imágenes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageStats {
    pub total: u32,
    pub recompressed: u32,
    pub downsampled: u32,
    /// Imágenes que se dejaron como estaban (máscaras, paleta, ya pequeñas, sin ganancia…).
    pub skipped: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OptimizeStats {
    pub before: SpaceUsage,
    pub after: SpaceUsage,
    pub images: ImageStats,
}

/// Optimiza `input` y escribe el resultado en `out` (nunca sobrescribe la entrada).
pub fn optimize(input: &Path, out: &Path, opts: &OptimizeOptions, ctx: &JobContext) -> Result<Report> {
    optimize_detailed(input, out, opts, ctx).map(|(r, _)| r)
}

/// Como [`optimize`], y además devuelve el tamaño antes/después por categoría.
pub fn optimize_detailed(
    input: &Path,
    out: &Path,
    opts: &OptimizeOptions,
    ctx: &JobContext,
) -> Result<(Report, OptimizeStats)> {
    if same_file(input, out) {
        return Err(Error::InvalidInput(
            "el archivo de salida no puede ser el mismo que el de entrada".into(),
        ));
    }
    let before = analyze_with_password(input, opts.password.as_deref())?;
    let mut doc = open(input, opts.password.as_deref())?;
    ctx.step(0, 3, "Optimizando imágenes")?;

    let mut stats = ImageStats::default();
    if let Some(dpi) = opts.target_dpi.or(opts.preset.target_dpi()) {
        let quality = opts.jpeg_quality.unwrap_or(opts.preset.jpeg_quality()).clamp(1, 100);
        stats = images::optimize_images(
            &mut doc,
            &images::ImageParams {
                target_dpi: dpi,
                quality,
                min_bytes: opts.min_image_bytes,
            },
            ctx,
        )?;
    }

    ctx.step(1, 3, "Guardando")?;
    let tmp = tmp_path(out);
    let mut wo = PdfWriteOptions::default();
    wo.set_garbage_level(4)
        .set_compress(true)
        .set_compress_images(true)
        .set_compress_fonts(true);
    doc.save_with_options(&path_str(&tmp)?, wo).map_err(pdf_err)?;
    if let Err(e) = std::fs::rename(&tmp, out) {
        let _ = std::fs::remove_file(&tmp);
        return Err(Error::io_at(out, e));
    }

    ctx.step(2, 3, "Comprobando el resultado")?;
    let after = analyze(out)?;
    let mut report = Report::with_output(
        out,
        format!(
            "{} → {} ({:+.0} %)",
            human(before.total_bytes),
            human(after.total_bytes),
            (after.total_bytes as f64 / before.total_bytes.max(1) as f64 - 1.0) * 100.0
        ),
    );
    if opts.linearize {
        report.warnings.push(
            "La linealización para web todavía no está disponible: el motor actual (MuPDF) ya no la admite. \
             El archivo se ha guardado sin linealizar."
                .into(),
        );
    }
    if after.total_bytes >= before.total_bytes {
        report
            .warnings
            .push("El archivo resultante no es más pequeño que el original.".into());
    }
    ctx.step(3, 3, "Terminado")?;
    Ok((
        report,
        OptimizeStats {
            before,
            after,
            images: stats,
        },
    ))
}

pub(crate) fn open(path: &Path, password: Option<&str>) -> Result<PdfDocument> {
    if !path.exists() {
        return Err(Error::io_at(path, std::io::Error::from(std::io::ErrorKind::NotFound)));
    }
    let mut doc = PdfDocument::open(&path_str(path)?).map_err(pdf_err)?;
    if doc.needs_password().map_err(pdf_err)? {
        let ok = match password {
            Some(p) => doc.authenticate(p).map_err(pdf_err)?,
            None => false,
        };
        if !ok {
            return Err(Error::Password);
        }
    }
    Ok(doc)
}

fn path_str(p: &Path) -> Result<String> {
    p.to_str()
        .map(str::to_owned)
        .ok_or_else(|| Error::InvalidInput(format!("ruta no válida: {}", p.display())))
}

fn tmp_path(out: &Path) -> PathBuf {
    let mut name = out.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    name.push(".tmp");
    out.with_file_name(name)
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

/// Tamaño legible en español (KB/MB con coma decimal).
pub fn human(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    let b = bytes as f64;
    let s = if b >= KB * KB {
        format!("{:.1} MB", b / (KB * KB))
    } else if b >= KB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{bytes} B")
    };
    s.replace('.', ",")
}
