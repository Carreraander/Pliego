use std::path::{Path, PathBuf};

use pliego_core::{Error, JobContext, Report, Result};
use pliego_pdf::PdfDocument;
use serde::{Deserialize, Serialize};

use crate::io::save;
use crate::outline::read_outline;
use crate::pages::{extract, extract_inner};

/// Cómo dividir un documento.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SplitRule {
    /// Un archivo cada `n` páginas.
    EveryN { n: u32 },
    /// Tramos consecutivos de `(primera, última)` páginas, desde 0 e inclusive.
    Ranges { ranges: Vec<(u32, u32)> },
    /// Un archivo por cada marcador de primer nivel.
    Bookmarks,
    /// Archivos que no superen `bytes` (si una sola página lo supera, va sola y se avisa).
    MaxBytes { bytes: u64 },
}

struct Part {
    start: u32,
    end: u32, // exclusivo
    label: Option<String>,
}

/// Divide `doc` y escribe `<nombre>-001.pdf`, `<nombre>-002.pdf`… en `out_dir`.
pub fn split(doc: &PdfDocument, stem: &str, rule: &SplitRule, out_dir: &Path, ctx: &JobContext) -> Result<Report> {
    let n = doc.page_count()?.max(0) as u32;
    if n == 0 {
        return Err(Error::InvalidInput("el documento no tiene páginas".into()));
    }
    let mut warnings = Vec::new();
    let parts = plan(doc, n, rule, &mut warnings)?;
    std::fs::create_dir_all(out_dir).map_err(|e| Error::io_at(out_dir, e))?;
    let total = parts.len() as u64;
    let mut outputs: Vec<PathBuf> = Vec::new();
    for (i, part) in parts.iter().enumerate() {
        ctx.step(i as u64, total, &format!("Parte {} de {total}", i + 1))?;
        let pages: Vec<u32> = (part.start..part.end).collect();
        let sub = extract(doc, &pages)?;
        let name = match &part.label {
            Some(l) => format!("{stem}-{:03}-{}.pdf", i + 1, sanitize(l)),
            None => format!("{stem}-{:03}.pdf", i + 1),
        };
        let path = out_dir.join(name);
        save(&sub, &path)?;
        outputs.push(path);
    }
    ctx.step(total, total, "Listo")?;
    Ok(Report {
        summary: format!("{} archivos creados", outputs.len()),
        outputs,
        warnings,
    })
}

fn sanitize(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    cleaned.trim_matches('_').chars().take(40).collect()
}

fn plan(doc: &PdfDocument, n: u32, rule: &SplitRule, warnings: &mut Vec<String>) -> Result<Vec<Part>> {
    match rule {
        SplitRule::EveryN { n: step } => {
            if *step == 0 {
                return Err(Error::InvalidInput(
                    "el número de páginas por archivo debe ser mayor que 0".into(),
                ));
            }
            Ok((0..n)
                .step_by(*step as usize)
                .map(|s| Part {
                    start: s,
                    end: (s + step).min(n),
                    label: None,
                })
                .collect())
        }
        SplitRule::Ranges { ranges } => {
            if ranges.is_empty() {
                return Err(Error::InvalidInput("no se indicó ningún tramo".into()));
            }
            ranges
                .iter()
                .map(|&(a, b)| {
                    if a > b || b >= n {
                        Err(Error::InvalidInput(format!(
                            "el tramo {}-{} no existe (hay {n} páginas)",
                            a + 1,
                            b + 1
                        )))
                    } else {
                        Ok(Part {
                            start: a,
                            end: b + 1,
                            label: None,
                        })
                    }
                })
                .collect()
        }
        SplitRule::Bookmarks => {
            let mut marks: Vec<(u32, String)> = read_outline(doc)?
                .into_iter()
                .filter_map(|o| o.page.filter(|&p| p < n).map(|p| (p, o.title)))
                .collect();
            marks.sort_by_key(|m| m.0);
            marks.dedup_by_key(|m| m.0);
            if marks.is_empty() {
                return Err(Error::InvalidInput(
                    "el documento no tiene marcadores de primer nivel".into(),
                ));
            }
            if marks[0].0 > 0 {
                warnings.push(format!(
                    "las {} primeras páginas no tienen marcador y van en la primera parte",
                    marks[0].0
                ));
                marks[0].0 = 0;
            }
            Ok(marks
                .iter()
                .enumerate()
                .map(|(i, (s, t))| Part {
                    start: *s,
                    end: marks.get(i + 1).map_or(n, |m| m.0),
                    label: Some(t.clone()),
                })
                .collect())
        }
        SplitRule::MaxBytes { bytes } => {
            let mut parts = Vec::new();
            let mut start = 0;
            while start < n {
                let fits = |len: u32| -> Result<bool> {
                    let pages: Vec<u32> = (start..start + len).collect();
                    let mut buf = Vec::new();
                    extract_inner(doc, &pages, false)?.write_to(&mut buf)?;
                    Ok(buf.len() as u64 <= *bytes)
                };
                let room = n - start;
                if !fits(1)? {
                    warnings.push(format!("la página {} supera por sí sola el tamaño máximo", start + 1));
                    parts.push(Part {
                        start,
                        end: start + 1,
                        label: None,
                    });
                    start += 1;
                    continue;
                }
                // Crecer al doble mientras quepa y luego bisecar: pocas escrituras de prueba por parte.
                let (mut ok, mut bad) = (1, None);
                while ok < room {
                    let next = (ok * 2).min(room);
                    if fits(next)? {
                        ok = next;
                    } else {
                        bad = Some(next);
                        break;
                    }
                }
                if let Some(mut hi) = bad {
                    while hi - ok > 1 {
                        let mid = (ok + hi) / 2;
                        if fits(mid)? {
                            ok = mid;
                        } else {
                            hi = mid;
                        }
                    }
                }
                parts.push(Part {
                    start,
                    end: start + ok,
                    label: None,
                });
                start += ok;
            }
            Ok(parts)
        }
    }
}
