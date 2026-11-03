use std::path::PathBuf;

use anyhow::Context;
use clap::Args;
use pliego_compare::{ChangeKind, CompareOptions, PageStatus};
use pliego_core::{JobContext, NoProgress};

#[derive(Args)]
pub struct CompareArgs {
    /// Versión original.
    pub a: PathBuf,
    /// Versión nueva.
    pub b: PathBuf,
    /// Informe de salida: `.pdf` (por defecto) o `.json` según la extensión.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    /// Contraseña del original, si la tiene.
    #[arg(long)]
    pub password_a: Option<String>,
    /// Contraseña de la versión nueva, si la tiene.
    #[arg(long)]
    pub password_b: Option<String>,
    /// No comparar el texto.
    #[arg(long)]
    pub no_text: bool,
    /// No hacer la comparación visual por píxeles.
    #[arg(long)]
    pub no_visual: bool,
    /// Resolución de la comparación visual (ppp).
    #[arg(long, default_value_t = 96.0)]
    pub dpi: f32,
}

pub fn compare(a: CompareArgs, json: bool) -> anyhow::Result<()> {
    let opts = CompareOptions {
        text: !a.no_text,
        visual: !a.no_visual,
        visual_dpi: a.dpi,
        password_a: a.password_a.clone(),
        password_b: a.password_b.clone(),
        ..CompareOptions::default()
    };
    let ctx = JobContext::new(&NoProgress, Default::default());
    let result = pliego_compare::compare(&a.a, &a.b, &opts, &ctx).context("no se pudieron comparar los documentos")?;

    if let Some(out) = &a.output {
        let is_json = out.extension().is_some_and(|e| e.eq_ignore_ascii_case("json"));
        if is_json {
            std::fs::write(out, serde_json::to_string_pretty(&result)?)
                .with_context(|| format!("no se pudo escribir {}", out.display()))?;
        } else {
            pliego_compare::write_report(&result, out)
                .with_context(|| format!("no se pudo escribir el informe {}", out.display()))?;
        }
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }
    let s = &result.summary;
    if result.is_identical() {
        println!("Sin diferencias.");
    } else {
        println!(
            "Páginas: {} → {} · insertadas {} · eliminadas {} · con cambios {}",
            s.pages_a, s.pages_b, s.inserted_pages, s.deleted_pages, s.changed_pages
        );
        println!(
            "Palabras: +{} −{} ~{} · formato {} · zonas visuales {}",
            s.added_words, s.removed_words, s.changed_words, s.format_changes, s.visual_regions
        );
        for p in &result.pages {
            let pa = p.page_a.map_or("-".to_string(), |n| (n + 1).to_string());
            let pb = p.page_b.map_or("-".to_string(), |n| (n + 1).to_string());
            match p.status {
                PageStatus::Inserted => println!("[{pa} → {pb}] página insertada"),
                PageStatus::Deleted => println!("[{pa} → {pb}] página eliminada"),
                PageStatus::Matched => {
                    for t in &p.text_changes {
                        let what = match (t.format_only, t.kind) {
                            (true, _) => format!("formato «{}» ({})", t.text_b, t.note.clone().unwrap_or_default()),
                            (_, ChangeKind::Added) => format!("añadido «{}»", t.text_b),
                            (_, ChangeKind::Removed) => format!("borrado «{}»", t.text_a),
                            (_, ChangeKind::Changed) => format!("cambiado «{}» → «{}»", t.text_a, t.text_b),
                        };
                        println!("[{pa} → {pb}] {what}");
                    }
                    if !p.visual_regions.is_empty() {
                        println!(
                            "[{pa} → {pb}] {} zona(s) con diferencias visuales",
                            p.visual_regions.len()
                        );
                    }
                }
            }
        }
    }
    if let Some(out) = &a.output {
        println!("Informe: {}", out.display());
    }
    Ok(())
}
