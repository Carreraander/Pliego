//! Análisis del reparto de espacio en un PDF.

use std::path::Path;

use pliego_core::Result;
use serde::{Deserialize, Serialize};

use crate::{open, pdf_err};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageUsage {
    pub xref: i32,
    pub width: u32,
    pub height: u32,
    pub filter: Option<String>,
    pub bytes: u64,
}

/// Bytes por categoría. La suma de las categorías es `total_bytes`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpaceUsage {
    pub total_bytes: u64,
    pub images_bytes: u64,
    pub fonts_bytes: u64,
    /// Contenido de páginas y formularios (XObject).
    pub content_bytes: u64,
    /// Metadatos, perfiles de color y otros flujos.
    pub other_streams_bytes: u64,
    /// Estructura del documento: objetos, tabla de referencias, cabeceras de flujos.
    pub structure_bytes: u64,
    pub image_count: u32,
    pub object_count: u32,
    /// Las imágenes más pesadas, de mayor a menor (hasta 10).
    pub largest_images: Vec<ImageUsage>,
}

pub fn analyze(input: &Path) -> Result<SpaceUsage> {
    analyze_with_password(input, None)
}

pub fn analyze_with_password(input: &Path, password: Option<&str>) -> Result<SpaceUsage> {
    let total = std::fs::metadata(input)
        .map_err(|e| pliego_core::Error::io_at(input, e))?
        .len();
    let doc = open(input, password)?;
    let n = doc.xref_len().map_err(pdf_err)?;
    let mut u = SpaceUsage {
        total_bytes: total,
        object_count: n.saturating_sub(1) as u32,
        ..Default::default()
    };
    let mut images = Vec::new();
    let mut streams = 0u64;
    for xref in 1..n as i32 {
        let Ok(obj) = doc.new_indirect(xref, 0) else { continue };
        let Ok(true) = obj.is_stream() else { continue };
        let bytes = obj.read_raw_stream().map(|s| s.len() as u64).unwrap_or(0);
        streams += bytes;
        let name = |k: &str| {
            obj.get_dict(k)
                .ok()
                .flatten()
                .and_then(|v| v.as_name().ok())
                .map(|b| String::from_utf8_lossy(&b).into_owned())
        };
        let subtype = name("Subtype");
        let ty = name("Type");
        let has = |k: &str| obj.get_dict(k).ok().flatten().is_some();
        if subtype.as_deref() == Some("Image") {
            u.images_bytes += bytes;
            u.image_count += 1;
            let dim = |k: &str| {
                obj.get_dict(k)
                    .ok()
                    .flatten()
                    .and_then(|v| v.as_int().ok())
                    .unwrap_or(0) as u32
            };
            let filter = obj.get_dict("Filter").ok().flatten().and_then(|f| {
                if f.is_array().unwrap_or(false) {
                    f.get_array(f.len().ok()?.checked_sub(1)? as i32).ok().flatten()
                } else {
                    Some(f)
                }
                .and_then(|f| f.as_name().ok())
                .map(|b| String::from_utf8_lossy(&b).into_owned())
            });
            images.push(ImageUsage {
                xref,
                width: dim("Width"),
                height: dim("Height"),
                filter,
                bytes,
            });
        } else if has("Length1") || matches!(subtype.as_deref(), Some("Type1C" | "CIDFontType0C" | "OpenType")) {
            u.fonts_bytes += bytes;
        } else if subtype.as_deref() == Some("Form") || (ty.is_none() && subtype.is_none() && !has("N")) {
            u.content_bytes += bytes;
        } else {
            u.other_streams_bytes += bytes;
        }
    }
    u.structure_bytes = total.saturating_sub(streams);
    images.sort_by_key(|i| std::cmp::Reverse(i.bytes));
    images.truncate(10);
    u.largest_images = images;
    Ok(u)
}
