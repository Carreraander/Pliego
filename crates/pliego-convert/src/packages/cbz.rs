//! CBZ (cómic): un ZIP de imágenes que se leen en orden natural de nombres (`2.png` antes que `10.png`).

use std::cmp::Ordering;
use std::path::Path;

use pliego_core::{Error, JobContext, Report, Result};

use super::zipio::Package;
use crate::images::{ImagesToPdfOptions, images_to_pdf};
use crate::util::ensure_not_same;

/// Imágenes máximas por cómic y bytes máximos (descomprimidos) de una imagen y de todas.
const MAX_IMAGES: usize = 20_000;
const MAX_IMAGE_BYTES: u64 = 256 << 20;
const MAX_TOTAL_BYTES: u64 = 4 << 30;

const IMAGE_EXTENSIONS: [&str; 8] = ["jpg", "jpeg", "png", "webp", "gif", "bmp", "tif", "tiff"];

/// Convierte un CBZ en un PDF con una página por imagen. Las carpetas internas se ordenan por nombre completo.
/// `ComicInfo.xml`, miniaturas de sistema (`__MACOSX`, `Thumbs.db`) y los archivos ocultos se ignoran.
pub fn cbz_to_pdf(input: &Path, output: &Path, options: &ImagesToPdfOptions, ctx: &JobContext) -> Result<Report> {
    ensure_not_same(input, output)?;
    let mut package = Package::open(input)?;
    let mut pages: Vec<(String, &'static str)> = package
        .names()
        .iter()
        .filter_map(|n| image_extension(n).map(|e| (n.clone(), e)))
        .collect();
    if pages.is_empty() {
        return Err(Error::InvalidInput(format!(
            "{}: el CBZ no contiene imágenes (admitidas: JPG, PNG, WebP, GIF, BMP y TIFF)",
            input.display()
        )));
    }
    if pages.len() > MAX_IMAGES {
        return Err(Error::InvalidInput(format!(
            "{}: demasiadas imágenes ({})",
            input.display(),
            pages.len()
        )));
    }
    pages.sort_by(|a, b| natural_cmp(&a.0, &b.0));

    let dir = tempfile::tempdir().map_err(|e| Error::io_at(std::env::temp_dir(), e))?;
    let mut paths = Vec::with_capacity(pages.len());
    let mut total = 0u64;
    for (n, (name, ext)) in pages.iter().enumerate() {
        ctx.step(n as u64, pages.len() as u64, "leyendo imágenes")?;
        let bytes = package.require(name, MAX_IMAGE_BYTES)?;
        total += bytes.len() as u64;
        if total > MAX_TOTAL_BYTES {
            return Err(Error::InvalidInput(format!(
                "{}: el cómic descomprimido es demasiado grande",
                input.display()
            )));
        }
        // Nombre propio: el del archivo interno nunca se usa como ruta.
        let path = dir.path().join(format!("{n:06}.{ext}"));
        std::fs::write(&path, bytes).map_err(|e| Error::io_at(&path, e))?;
        paths.push(path);
    }
    let mut report = images_to_pdf(&paths, output, options, ctx)?;
    report.summary = format!("{} (cómic de {} imágenes)", report.summary, paths.len());
    Ok(report)
}

/// Extensión de imagen en minúsculas si el nombre es una imagen de página; `None` para el resto.
fn image_extension(name: &str) -> Option<&'static str> {
    let lower = name.to_lowercase();
    if lower.starts_with("__macosx/") || lower.split('/').any(|p| p.starts_with('.')) {
        return None;
    }
    let ext = lower.rsplit_once('.')?.1;
    IMAGE_EXTENSIONS.iter().find(|e| **e == ext).copied()
}

/// Orden natural: los números dentro del nombre se comparan como números y las letras sin distinguir mayúsculas.
fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (a, b) = (a.to_lowercase(), b.to_lowercase());
    let (mut ia, mut ib) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (ia.peek().copied(), ib.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let (mut na, mut nb) = (String::new(), String::new());
                while let Some(c) = ia.next_if(char::is_ascii_digit) {
                    na.push(c);
                }
                while let Some(c) = ib.next_if(char::is_ascii_digit) {
                    nb.push(c);
                }
                let (ta, tb) = (na.trim_start_matches('0'), nb.trim_start_matches('0'));
                let ord = ta.len().cmp(&tb.len()).then_with(|| ta.cmp(tb));
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            (Some(x), Some(y)) => {
                if x != y {
                    return x.cmp(&y);
                }
                ia.next();
                ib.next();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn natural_order() {
        let mut v = vec!["p10.png", "p2.png", "P1.png", "p02b.png", "carpeta/p1.png", "p100.png"];
        v.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(
            v,
            ["carpeta/p1.png", "P1.png", "p2.png", "p02b.png", "p10.png", "p100.png"]
        );
    }

    #[test]
    fn page_names() {
        assert_eq!(image_extension("a/b.JPG"), Some("jpg"));
        assert_eq!(image_extension("__MACOSX/a.jpg"), None);
        assert_eq!(image_extension(".hidden/a.jpg"), None);
        assert_eq!(image_extension("ComicInfo.xml"), None);
        assert_eq!(image_extension("sin_extension"), None);
    }
}
