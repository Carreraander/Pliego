//! `pliego run accion.toml entrada... -o salida`: aplica una acción guardada a archivos y carpetas de PDFs.
//!
//! Los pasos disponibles se registran en `pliego-steps`.

use std::path::PathBuf;

use anyhow::Context;
use clap::Args;
use pliego_actions::{Action, RunOptions, collect_inputs};
use pliego_core::{JobContext, Progress};
use pliego_steps::builtin_registry;

#[derive(Args)]
pub struct RunArgs {
    /// Acción guardada (.toml o .json).
    #[arg(required_unless_present = "list_steps")]
    pub action: Option<PathBuf>,
    /// PDFs o carpetas (se recorren las subcarpetas).
    pub inputs: Vec<PathBuf>,
    /// Carpeta de salida; se conserva la estructura de subcarpetas.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    /// Retomar un lote anterior en la misma carpeta de salida: se saltan los archivos ya procesados.
    #[arg(long)]
    pub resume: bool,
    /// Lista los pasos disponibles y sale.
    #[arg(long)]
    pub list_steps: bool,
}

/// Progreso por línea en stderr (no estorba a `--json` en stdout).
struct LineProgress;
impl Progress for LineProgress {
    fn report(&self, done: u64, total: u64, message: &str) {
        eprintln!("[{done}/{total}] {message}");
    }
}

pub fn run(a: RunArgs, json: bool) -> anyhow::Result<()> {
    let registry = builtin_registry();
    if a.list_steps {
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&registry.kinds().collect::<Vec<_>>())?
            );
        } else {
            for k in registry.kinds() {
                println!("{k}");
            }
        }
        return Ok(());
    }
    let action_path = a.action.context("falta la acción")?;
    let action = Action::load(&action_path)?;
    let output_dir = a
        .output
        .context("falta la carpeta de salida: usa -o carpeta (los originales nunca se modifican)")?;
    anyhow::ensure!(!a.inputs.is_empty(), "indica al menos un PDF o carpeta de entrada");
    let inputs = collect_inputs(&a.inputs, Some(&output_dir))?;
    anyhow::ensure!(!inputs.is_empty(), "no hay ningún PDF en las entradas indicadas");

    let ctx = JobContext::new(&LineProgress, Default::default());
    let r = pliego_actions::run(
        &action,
        &registry,
        &inputs,
        &RunOptions {
            output_dir,
            resume: a.resume,
        },
        &ctx,
    )?;
    if json {
        println!("{}", serde_json::to_string_pretty(&r)?);
    } else {
        for w in &r.report.warnings {
            eprintln!("aviso: {w}");
        }
        println!("{}", r.report.summary);
    }
    if !r.failures.is_empty() {
        std::process::exit(2);
    }
    Ok(())
}
