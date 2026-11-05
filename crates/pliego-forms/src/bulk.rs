use std::path::{Path, PathBuf};

use pliego_core::{Error, JobContext, Report, Result};
use pliego_pdf::PdfDocument;

use crate::csv;
use crate::data::coerce;
use crate::fields::{fields, fill, flatten};

fn open(path: &Path) -> Result<PdfDocument> {
    PdfDocument::open(path).map_err(|e| Error::Pdf(format!("no se pudo abrir {}: {e}", path.display())))
}

fn safe_name(s: &str) -> String {
    let t: String = s
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | ' ') {
                c
            } else {
                '_'
            }
        })
        .collect();
    t.trim().trim_matches('.').to_string()
}

/// Genera un PDF por cada fila del CSV a partir de una plantilla con campos.
///
/// La primera fila del CSV son los nombres de campo. Una columna opcional `_filename` fija el nombre
/// del archivo de salida; si no, se usa `<plantilla>-001.pdf`, `-002`, etc. Informa progreso por fila
/// y puede cancelarse (los archivos ya generados se conservan).
pub fn bulk_fill(
    template: &Path,
    csv_path: &Path,
    out_dir: &Path,
    flatten_after: bool,
    ctx: &JobContext,
) -> Result<Report> {
    let text = std::fs::read_to_string(csv_path).map_err(|e| Error::io_at(csv_path, e))?;
    let rows = csv::parse(&text);
    let Some((header, data)) = rows.split_first() else {
        return Err(Error::InvalidInput("el CSV está vacío".into()));
    };
    if data.is_empty() {
        return Err(Error::InvalidInput("el CSV no tiene filas de datos".into()));
    }
    std::fs::create_dir_all(out_dir).map_err(|e| Error::io_at(out_dir, e))?;
    let stem = template.file_stem().and_then(|s| s.to_str()).unwrap_or("formulario");
    let total = data.len() as u64;
    let mut report = Report::default();
    let mut used: Vec<PathBuf> = Vec::new();
    for (i, row) in data.iter().enumerate() {
        ctx.step(i as u64, total, &format!("Fila {} de {total}", i + 1))?;
        let mut doc = open(template)?;
        let infos = fields(&mut doc)?;
        let mut pairs = Vec::new();
        let mut filename = None;
        for (k, v) in header.iter().zip(row) {
            if k == "_filename" {
                filename = Some(safe_name(v)).filter(|s| !s.is_empty());
            } else {
                pairs.push((k.clone(), v.clone()));
            }
        }
        let res = fill(&mut doc, &coerce(&infos, &pairs))?;
        for nf in res.not_found {
            let w = format!("Fila {}: el campo «{nf}» no existe en la plantilla", i + 1);
            if !report.warnings.contains(&w) {
                report.warnings.push(w);
            }
        }
        if flatten_after {
            flatten(&mut doc)?;
        }
        let base = filename.unwrap_or_else(|| format!("{stem}-{:03}", i + 1));
        let mut dest = out_dir.join(format!("{base}.pdf"));
        let mut n = 2;
        while used.contains(&dest) {
            dest = out_dir.join(format!("{base}-{n}.pdf"));
            n += 1;
        }
        let tmp = out_dir.join(format!(".pliego-{}-{i}.tmp", std::process::id()));
        {
            let mut f = std::fs::File::create(&tmp).map_err(|e| Error::io_at(&tmp, e))?;
            doc.write_to(&mut f)?;
        }
        std::fs::rename(&tmp, &dest).map_err(|e| Error::io_at(&dest, e))?;
        used.push(dest.clone());
        report.outputs.push(dest);
    }
    ctx.step(total, total, "Terminado")?;
    report.summary = format!("{} PDF generado(s) en {}", report.outputs.len(), out_dir.display());
    Ok(report)
}
