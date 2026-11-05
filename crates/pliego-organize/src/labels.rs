use pliego_core::Result;
use pliego_pdf::{PageLabelRule, PageLabelStyle, PdfDocument};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelStyle {
    None,
    Decimal,
    UpperRoman,
    LowerRoman,
    UpperAlpha,
    LowerAlpha,
}

/// Regla de etiquetado: desde la página `start_page` (desde 0) hasta la siguiente regla.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageLabelSpec {
    pub start_page: u32,
    pub style: LabelStyle,
    #[serde(default)]
    pub prefix: String,
    /// Número de la primera página del tramo (1 por defecto).
    #[serde(default = "one")]
    pub first_number: i32,
}

fn one() -> i32 {
    1
}

/// Añade o sustituye reglas de etiquetas de página.
pub fn set_page_labels(doc: &mut PdfDocument, rules: &[PageLabelSpec]) -> Result<()> {
    for r in rules {
        let style = match r.style {
            LabelStyle::None => PageLabelStyle::None,
            LabelStyle::Decimal => PageLabelStyle::Decimal,
            LabelStyle::UpperRoman => PageLabelStyle::UpperRoman,
            LabelStyle::LowerRoman => PageLabelStyle::LowerRoman,
            LabelStyle::UpperAlpha => PageLabelStyle::UpperAlpha,
            LabelStyle::LowerAlpha => PageLabelStyle::LowerAlpha,
        };
        let mut rule = PageLabelRule::new(r.start_page as usize, style);
        rule.prefix = r.prefix.clone();
        rule.start = r.first_number;
        doc.set_page_label_rule(rule)?;
    }
    Ok(())
}
