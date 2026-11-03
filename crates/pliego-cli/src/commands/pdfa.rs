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
    /// Valida además con veraPDF (descarga aparte, necesita Java).
    #[arg(long)]
    pub verapdf: bool,
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
    /// Comprueba el resultado con veraPDF (descarga aparte, necesita Java).
    #[arg(long)]
    pub verapdf: bool,
}

fn print_vera(r: &pliego_pdfa::verapdf::VeraReport) {
    if r.compliant {
        println!("veraPDF: cumple.");
    } else {
        println!("veraPDF: no cumple.");
        for f in &r.failures {
            println!(
                "  - {} cláusula {} ({} fallo(s)): {}",
                f.specification, f.clause, f.failed_checks, f.description
            );
        }
    }
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
            let vera = if a.verapdf {
                Some(pliego_pdfa::verapdf::validate(&a.input, a.level)?)
            } else {
                None
            };
            if let Some(r) = &vera
                && !json
            {
                print_vera(r);
            }
            if !v.conforms() || vera.as_ref().is_some_and(|r| !r.compliant) {
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
            if a.verapdf {
                let r = pliego_pdfa::verapdf::validate(&a.output, a.level)?;
                if !json {
                    print_vera(&r);
                }
                if !r.compliant {
                    bail!("veraPDF dice que el resultado no cumple {}", a.level.name());
                }
            }
            Ok(())
        }
    }
}
