use std::path::{Path, PathBuf};

use pliego_core::{Error, JobContext, Report, Result};
use pliego_pdf::{InsertPdfOptions, PdfDocument};
use serde::{Deserialize, Serialize};

use crate::annots::{FormState, carry_annotations};
use crate::io::{open, save};
use crate::outline::{OutlineNode, read_outline, retarget, write_outline};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergeOptions {
    /// Conserva los marcadores de cada archivo (desplazados a su nueva página).
    #[serde(default = "yes")]
    pub keep_bookmarks: bool,
    /// Añade un marcador de primer nivel por archivo con su nombre, del que cuelgan los suyos.
    #[serde(default)]
    pub bookmark_per_file: bool,
    /// Contraseña común para los archivos protegidos.
    #[serde(default)]
    pub password: Option<String>,
}

fn yes() -> bool {
    true
}

impl Default for MergeOptions {
    fn default() -> Self {
        Self {
            keep_bookmarks: true,
            bookmark_per_file: false,
            password: None,
        }
    }
}

/// Combina varios PDFs en uno nuevo, en el orden dado.
pub fn merge(inputs: &[PathBuf], out: &Path, options: &MergeOptions, ctx: &JobContext) -> Result<Report> {
    if inputs.is_empty() {
        return Err(Error::InvalidInput("no hay archivos que combinar".into()));
    }
    let mut dest = PdfDocument::new();
    let mut outline: Vec<OutlineNode> = Vec::new();
    let total = inputs.len() as u64;
    let mut form = FormState::default();
    for (i, path) in inputs.iter().enumerate() {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        ctx.step(i as u64, total, &format!("Añadiendo {name}"))?;
        let mut src = open(path, options.password.as_deref())?;
        let src_pages = src.page_count()?.max(0) as u32;
        let offset = dest.page_count()?.max(0) as u32;
        if src_pages == 0 {
            continue;
        }
        dest.insert_pdf(&src, InsertPdfOptions::default())?;
        let pairs: Vec<(u32, u32)> = (0..src_pages).map(|i| (i, offset + i)).collect();
        carry_annotations(&mut dest, &mut src, &pairs, &mut form)?;
        if options.keep_bookmarks || options.bookmark_per_file {
            let kept = if options.keep_bookmarks {
                retarget(&read_outline(&src)?, &|p| Some(p + offset))
            } else {
                Vec::new()
            };
            if options.bookmark_per_file {
                let title = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or(name);
                outline.push(OutlineNode {
                    title,
                    page: Some(offset),
                    children: kept,
                });
            } else {
                outline.extend(kept);
            }
        }
    }
    write_outline(&mut dest, &outline)?;
    ctx.step(total, total, "Guardando")?;
    save(&dest, out)?;
    let pages = dest.page_count()?;
    let mut report = Report::with_output(out, format!("{} archivos combinados, {pages} páginas", inputs.len()));
    if form.renamed() > 0 {
        report.warnings.push(format!(
            "{} campos de formulario se renombraron (_2, _3…) porque su nombre ya existía en otro archivo",
            form.renamed()
        ));
    }
    Ok(report)
}
