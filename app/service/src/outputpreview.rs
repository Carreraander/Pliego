//! Vista previa de salida (flujo W29): planchas CMYK de una página, tinta total y tintas del documento.
//!
//! Solo lectura. La lógica vive aquí, sin Tauri, para probarla en Linux; `commands/outputpreview.rs` solo traduce.

use std::path::Path;

use base64::Engine as _;
use pliego_core::Result;
pub use pliego_print::{Ink, PlateChannel};
use serde::Serialize;

/// Una plancha lista para mostrar: PNG en base64 (negro donde hay tinta) y su cobertura media (0 a 1).
#[derive(Debug, Clone, Serialize)]
pub struct PlatePreview {
    pub channel: PlateChannel,
    pub coverage: f32,
    pub png: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct OutputPreview {
    pub plates: Vec<PlatePreview>,
    /// Suma máxima de C+M+Y+K en un píxel (0 a 4; 3 es el 300 %).
    pub max_total_ink: f32,
    /// Tintas de todo el documento (proceso y directas).
    pub inks: Vec<Ink>,
}

/// Planchas de la página `page` (desde 0) a `dpi` ppp y tintas del documento.
pub fn output_preview(path: &Path, password: Option<&str>, page: usize, dpi: f32) -> Result<OutputPreview> {
    let doc = pliego_print::open(path, password)?;
    let seps = pliego_print::separations(&doc, page, dpi)?;
    let plates = seps
        .plates
        .iter()
        .map(|p| {
            Ok(PlatePreview {
                channel: p.channel,
                coverage: p.coverage(),
                png: base64::engine::general_purpose::STANDARD.encode(p.to_png()?),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(OutputPreview {
        plates,
        max_total_ink: seps.max_total_ink,
        inks: pliego_print::inks(&doc)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn corpus(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/corpus/print")
            .join(name)
    }

    #[test]
    fn mixed_cmyk_page_gives_four_plates_and_total_ink() {
        let r = output_preview(&corpus("cmyk-mezcla.pdf"), None, 0, 36.0).unwrap();
        assert_eq!(r.plates.len(), 4);
        assert_eq!(r.plates[0].channel, PlateChannel::Cyan);
        assert!((r.plates[0].coverage - 0.5).abs() < 0.02);
        assert!((r.max_total_ink - 1.6).abs() < 0.03);
        let png = base64::engine::general_purpose::STANDARD
            .decode(&r.plates[3].png)
            .unwrap();
        assert_eq!(&png[1..4], b"PNG");
        assert!(!r.inks.is_empty());
    }

    #[test]
    fn bad_page_and_dpi_are_errors() {
        let p = corpus("cmyk-cian.pdf");
        assert!(output_preview(&p, None, 9, 36.0).is_err());
        assert!(output_preview(&p, None, 0, 0.0).is_err());
        assert!(output_preview(Path::new("no-existe.pdf"), None, 0, 36.0).is_err());
    }
}
