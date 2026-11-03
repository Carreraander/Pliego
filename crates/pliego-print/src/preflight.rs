use mupdf::pdf::PdfDocument;
use pliego_core::{JobContext, Result};
use serde::{Deserialize, Serialize};

use crate::pdf_err;
use crate::scan::{Cs, scan_page};

/// Reglas que se comprueban. Los perfiles incluidos son un punto de partida; todo es ajustable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    /// Resolución mínima efectiva de las imágenes, en ppp.
    pub min_image_dpi: Option<f32>,
    /// Grosor mínimo de línea, en puntos (0,25 pt ≈ 0,09 mm).
    pub min_line_width: Option<f32>,
    pub require_embedded_fonts: bool,
    /// Si es `false`, el contenido RGB es un error (en vez de un aviso).
    pub allow_rgb: bool,
    /// Si es `false`, avisa del contenido RGB; si es `true`, no dice nada.
    pub warn_rgb: bool,
    pub max_spot_inks: Option<usize>,
    pub require_trim_box: bool,
    pub require_output_intent: bool,
}

impl Profile {
    /// Comprobaciones mínimas: fuentes incrustadas y líneas muy finas.
    pub fn basic() -> Self {
        Self {
            name: "basic".into(),
            min_image_dpi: Some(72.0),
            min_line_width: Some(0.1),
            require_embedded_fonts: true,
            allow_rgb: true,
            warn_rgb: false,
            max_spot_inks: None,
            require_trim_box: false,
            require_output_intent: false,
        }
    }

    /// Imprenta offset/digital habitual: 300 ppp, líneas ≥ 0,25 pt, avisa del RGB.
    pub fn print() -> Self {
        Self {
            name: "print".into(),
            min_image_dpi: Some(300.0),
            min_line_width: Some(0.25),
            require_embedded_fonts: true,
            allow_rgb: true,
            warn_rgb: true,
            max_spot_inks: Some(2),
            require_trim_box: true,
            require_output_intent: false,
        }
    }

    /// Requisitos de PDF/X: sin RGB, con caja de corte e intención de salida.
    pub fn pdfx() -> Self {
        Self {
            name: "pdfx".into(),
            min_image_dpi: Some(300.0),
            min_line_width: Some(0.25),
            require_embedded_fonts: true,
            allow_rgb: false,
            warn_rgb: true,
            max_spot_inks: None,
            require_trim_box: true,
            require_output_intent: true,
        }
    }

    pub fn by_name(name: &str) -> Option<Self> {
        match name {
            "basic" => Some(Self::basic()),
            "print" => Some(Self::print()),
            "pdfx" => Some(Self::pdfx()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingCode {
    FontNotEmbedded,
    LowResolution,
    HairLine,
    RgbContent,
    TooManySpotInks,
    MissingTrimBox,
    MissingOutputIntent,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Finding {
    pub severity: Severity,
    pub code: FindingCode,
    /// Página (desde 0); `None` si afecta a todo el documento.
    pub page: Option<usize>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreflightReport {
    pub profile: String,
    pub pages: usize,
    pub findings: Vec<Finding>,
}

impl PreflightReport {
    pub fn errors(&self) -> usize {
        self.count(Severity::Error)
    }
    pub fn warnings(&self) -> usize {
        self.count(Severity::Warning)
    }
    fn count(&self, s: Severity) -> usize {
        self.findings.iter().filter(|f| f.severity == s).count()
    }
    /// `true` si no hay errores (los avisos no impiden aprobar).
    pub fn passed(&self) -> bool {
        self.errors() == 0
    }
}

/// Comprueba el documento contra `profile`.
pub fn preflight(doc: &PdfDocument, profile: &Profile, ctx: &JobContext<'_>) -> Result<PreflightReport> {
    let pages = doc.page_count().map_err(pdf_err)?.max(0) as usize;
    let mut findings = Vec::new();
    let mut all_spots = std::collections::BTreeSet::new();

    if profile.require_output_intent {
        let has = doc
            .catalog()
            .map_err(pdf_err)?
            .get_dict("OutputIntents")
            .map_err(pdf_err)?
            .is_some_and(|a| a.len().unwrap_or(0) > 0);
        if !has {
            findings.push(Finding {
                severity: Severity::Error,
                code: FindingCode::MissingOutputIntent,
                page: None,
                message: "Falta la intención de salida (perfil de color de la condición de impresión).".into(),
            });
        }
    }

    for p in 0..pages {
        ctx.step(p as u64, pages as u64, "Comprobando páginas")?;
        let n = p + 1;
        let scan = scan_page(doc, p)?;
        let mut add = |severity, code, message: String| {
            findings.push(Finding {
                severity,
                code,
                page: Some(p),
                message,
            })
        };

        if profile.require_embedded_fonts {
            for f in scan.fonts.iter().filter(|f| !f.embedded) {
                add(
                    Severity::Error,
                    FindingCode::FontNotEmbedded,
                    format!("Página {n}: la fuente «{}» no está incrustada.", f.name),
                );
            }
        }
        if let Some(min) = profile.min_image_dpi {
            for img in scan.images.iter().filter(|i| i.dpi < min) {
                add(
                    Severity::Warning,
                    FindingCode::LowResolution,
                    format!(
                        "Página {n}: imagen de {}×{} px a {:.0} ppp (mínimo {min:.0}).",
                        img.width, img.height, img.dpi
                    ),
                );
            }
        }
        if let Some(min) = profile.min_line_width {
            let thin: Vec<f32> = scan.strokes.iter().copied().filter(|w| *w < min).collect();
            if !thin.is_empty() {
                let thinnest = thin.iter().copied().fold(f32::INFINITY, f32::min);
                add(
                    Severity::Warning,
                    FindingCode::HairLine,
                    format!(
                        "Página {n}: {} línea(s) más finas de {min} pt (la más fina, {thinnest:.3} pt).",
                        thin.len()
                    ),
                );
            }
        }
        let rgb_uses = scan.rgb + scan.images.iter().filter(|i| i.cs == Cs::Rgb).count() as u32;
        if rgb_uses > 0 && (!profile.allow_rgb || profile.warn_rgb) {
            add(
                if profile.allow_rgb {
                    Severity::Warning
                } else {
                    Severity::Error
                },
                FindingCode::RgbContent,
                format!("Página {n}: usa color RGB ({rgb_uses} ajuste(s) o imagen(es))."),
            );
        }
        if profile.require_trim_box {
            let page = doc.find_page(p as i32).map_err(pdf_err)?;
            if page.get_dict("TrimBox").map_err(pdf_err)?.is_none() {
                add(
                    Severity::Warning,
                    FindingCode::MissingTrimBox,
                    format!("Página {n}: no define la caja de corte (TrimBox)."),
                );
            }
        }
        all_spots.extend(scan.spots.into_iter().filter(|s| !matches!(s.as_str(), "None" | "All")));
    }
    if let Some(max) = profile.max_spot_inks {
        let spots: Vec<_> = all_spots
            .iter()
            .filter(|s| {
                !["Cyan", "Magenta", "Yellow", "Black"]
                    .iter()
                    .any(|p| p.eq_ignore_ascii_case(s))
            })
            .collect();
        if spots.len() > max {
            findings.push(Finding {
                severity: Severity::Warning,
                code: FindingCode::TooManySpotInks,
                page: None,
                message: format!(
                    "El documento usa {} colores directos ({}); el perfil admite {max}.",
                    spots.len(),
                    spots.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
                ),
            });
        }
    }
    ctx.step(pages as u64, pages as u64, "Comprobación terminada")?;
    Ok(PreflightReport {
        profile: profile.name.clone(),
        pages,
        findings,
    })
}
