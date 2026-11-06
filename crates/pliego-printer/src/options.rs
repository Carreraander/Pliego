//! Opciones de impresión: lo que elige la persona en el diálogo. Se serializan (camelCase) para viajar entre la
//! interfaz, la app y el proceso `pliego printer send`.

use std::path::PathBuf;

use pliego_core::{Error, PageRange, Result};
use serde::{Deserialize, Serialize};

/// Orientación de la hoja.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Orientation {
    /// La de la primera página que se imprime.
    #[default]
    Auto,
    Portrait,
    Landscape,
}

/// Cómo se coloca la página en la hoja.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ScaleMode {
    /// Ajustar a la zona imprimible (amplía o reduce).
    #[default]
    Fit,
    /// Reducir solo lo que no cabe; lo pequeño se queda como está.
    Shrink,
    /// Tamaño real (100 %), centrado en la hoja.
    Actual,
    /// Porcentaje propio (`scale_percent`).
    Custom,
}

/// Impresión a dos caras.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Duplex {
    #[default]
    Off,
    /// Encuadernar por el lado largo.
    LongEdge,
    /// Encuadernar por el lado corto.
    ShortEdge,
}

/// Qué páginas de la selección se imprimen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Subset {
    #[default]
    All,
    /// Solo las páginas impares (1, 3, 5…).
    Odd,
    /// Solo las pares (2, 4, 6…).
    Even,
}

/// Todo lo que decide la persona al imprimir.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PrintOptions {
    /// Nombre de la impresora; `None` = la predeterminada de Windows.
    pub printer: Option<String>,
    /// Páginas desde 1 con la sintaxis de siempre: `all`, `3`, `1-3,5`, `7-`.
    pub pages: String,
    pub subset: Subset,
    /// Imprimir de la última a la primera.
    pub reverse: bool,
    pub copies: u32,
    /// Con varias copias: 1,2,3 · 1,2,3 (sí) o 1,1 · 2,2 · 3,3 (no).
    pub collate: bool,
    pub orientation: Orientation,
    /// Girar 90° las páginas que así caben mejor (solo con Ajustar y Reducir).
    pub auto_rotate: bool,
    pub scale: ScaleMode,
    /// Porcentaje de `ScaleMode::Custom` (10 a 400).
    pub scale_percent: u32,
    pub duplex: Duplex,
    /// Imprimir en escala de grises.
    pub grayscale: bool,
    /// Incluir comentarios y marcas (los campos de formulario se imprimen siempre con su valor).
    pub comments: bool,
    /// Identificador de papel de Windows (`DMPAPER_*`); `None` = el de la impresora.
    pub paper: Option<i16>,
    /// Imprimir en un archivo en vez de enviarlo a la hoja (impresoras «a PDF»; también sirve para probar).
    pub output: Option<PathBuf>,
}

impl Default for PrintOptions {
    fn default() -> Self {
        Self {
            printer: None,
            pages: "all".into(),
            subset: Subset::All,
            reverse: false,
            copies: 1,
            collate: true,
            orientation: Orientation::Auto,
            auto_rotate: true,
            scale: ScaleMode::Fit,
            scale_percent: 100,
            duplex: Duplex::Off,
            grayscale: false,
            comments: true,
            paper: None,
            output: None,
        }
    }
}

impl PrintOptions {
    /// Comprueba los valores; los mensajes son para personas.
    pub fn validate(&self) -> Result<()> {
        if self.copies == 0 || self.copies > 999 {
            return Err(Error::InvalidInput("las copias van de 1 a 999".into()));
        }
        if self.scale == ScaleMode::Custom && !(10..=400).contains(&self.scale_percent) {
            return Err(Error::InvalidInput("la escala va del 10 % al 400 %".into()));
        }
        self.pages
            .parse::<PageRange>()
            .map(|_| ())
            .map_err(|_| Error::InvalidInput(format!("páginas no válidas: «{}»", self.pages)))
    }

    /// Páginas que se imprimen (índices desde 0), en el orden en que salen.
    pub fn selection(&self, page_count: u32) -> Result<Vec<u32>> {
        self.validate()?;
        let range: PageRange = self
            .pages
            .parse()
            .map_err(|_| Error::InvalidInput(format!("páginas no válidas: «{}»", self.pages)))?;
        let mut pages: Vec<u32> = range.resolve(page_count)?;
        // Una selección «5-3» se escribe al revés a propósito: se respeta; lo que se ordena es lo demás.
        pages.retain(|p| match self.subset {
            Subset::All => true,
            Subset::Odd => p % 2 == 0,
            Subset::Even => p % 2 == 1,
        });
        if self.reverse {
            pages.reverse();
        }
        if pages.is_empty() {
            return Err(Error::InvalidInput(
                "no hay ninguna página que imprimir con esa selección".into(),
            ));
        }
        Ok(pages)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(pages: &str) -> PrintOptions {
        PrintOptions {
            pages: pages.into(),
            ..PrintOptions::default()
        }
    }

    #[test]
    fn selection_all_range_and_subsets() {
        assert_eq!(opts("all").selection(4).unwrap(), vec![0, 1, 2, 3]);
        assert_eq!(opts("2-3").selection(4).unwrap(), vec![1, 2]);
        assert_eq!(opts("1,3-4").selection(5).unwrap(), vec![0, 2, 3]);
        let odd = PrintOptions {
            subset: Subset::Odd,
            ..opts("all")
        };
        assert_eq!(odd.selection(5).unwrap(), vec![0, 2, 4]);
        let even = PrintOptions {
            subset: Subset::Even,
            ..opts("2-5")
        };
        assert_eq!(even.selection(6).unwrap(), vec![1, 3]);
    }

    #[test]
    fn selection_reverse_and_errors() {
        let r = PrintOptions {
            reverse: true,
            ..opts("1-3")
        };
        assert_eq!(r.selection(3).unwrap(), vec![2, 1, 0]);
        assert!(opts("9").selection(3).is_err());
        assert!(opts("a-b").selection(3).is_err());
        let none = PrintOptions {
            subset: Subset::Even,
            ..opts("1")
        };
        assert!(none.selection(3).is_err());
    }

    #[test]
    fn validate_limits() {
        assert!(
            PrintOptions {
                copies: 0,
                ..opts("all")
            }
            .validate()
            .is_err()
        );
        assert!(
            PrintOptions {
                copies: 1000,
                ..opts("all")
            }
            .validate()
            .is_err()
        );
        let custom = |p| PrintOptions {
            scale: ScaleMode::Custom,
            scale_percent: p,
            ..opts("all")
        };
        assert!(custom(9).validate().is_err());
        assert!(custom(401).validate().is_err());
        assert!(custom(150).validate().is_ok());
    }

    #[test]
    fn json_roundtrip_uses_camel_case_and_defaults() {
        let o: PrintOptions = serde_json::from_str(r#"{"copies":2,"scale":"actual","duplex":"longEdge"}"#).unwrap();
        assert_eq!(o.copies, 2);
        assert_eq!(o.scale, ScaleMode::Actual);
        assert_eq!(o.duplex, Duplex::LongEdge);
        assert!(o.collate && o.comments && o.auto_rotate);
        let text = serde_json::to_string(&o).unwrap();
        assert!(text.contains("\"autoRotate\":true") && text.contains("\"scalePercent\":100"));
    }
}
