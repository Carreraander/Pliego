use std::path::PathBuf;

use anyhow::bail;
use clap::{Args, Subcommand};
use pliego_core::JobContext;
use pliego_pdfa::Level;

#[derive(Subcommand)]
pub enum PdfaCommand {
    /// Comprueba si el documento cumple PDF/A (validación propia de las reglas más comunes).
    Validate(ValidateArgs),
    /// Convierte el documento a PDF/A (1b, 2b, 2u o 3b).
    Convert(ConvertArgs),
}

#[derive(Args)]
pub struct ValidateArgs {
    /// PDF de entrada.
    pub input: PathBuf,
    /// Nivel: 1b, 2b, 2u o 3b.
    #[arg(short, long, default_value = "2b")]
    pub level: Level,
}

#[derive(Args)]
pub struct ConvertArgs {
    /// PDF de entrada.
    pub input: PathBuf,
    /// PDF/A de salida (no se sobrescribe la entrada).
    #[arg(short, long)]
    pub output: PathBuf,
    /// Nivel: 1b, 2b, 2u o 3b.
    #[arg(short, long, default_value = "2b")]
    pub level: Level,
}

pub fn run(cmd: PdfaCommand, json: bool) -> anyhow::Result<()> {
    match cmd {
        PdfaCommand::Validate(a) => {
            let v = pliego_pdfa::validate(&a.input, a.level)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&v)?);
            } else if v.conforms() {
                println!(
                    "{}: cumple {} según las reglas comprobadas.",
                    a.input.display(),
                    v.level.name()
                );
            } else {
                println!("{}: no cumple {}:", a.input.display(), v.level.name());
                for i in &v.issues {
                    println!("  - {}", i.message);
                }
            }
            if !v.conforms() {
                bail!("el documento no cumple {}", v.level.name());
            }
            Ok(())
        }
        PdfaCommand::Convert(a) => {
            let rep = pliego_pdfa::convert(&a.input, &a.output, a.level, &JobContext::silent())?;
            if json {
                println!("{}", serde_json::to_string_pretty(&rep)?);
            } else {
                println!("{}", rep.summary);
                println!("-> {}", a.output.display());
                for w in &rep.warnings {
                    eprintln!("aviso: {w}");
                }
            }
            Ok(())
        }
    }
}
