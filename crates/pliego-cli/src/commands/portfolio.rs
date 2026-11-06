//! `pliego portfolio create|list|extract`: carteras PDF (colecciones).

use std::path::PathBuf;

use clap::{Args, Subcommand};
use pliego_core::{JobContext, Progress};

#[derive(Subcommand)]
pub enum PortfolioCommand {
    /// Crea una cartera con los archivos indicados.
    Create(CreateArgs),
    /// Lista los archivos incrustados de un PDF.
    List(ListArgs),
    /// Extrae archivos de una cartera a una carpeta.
    Extract(ExtractArgs),
}

#[derive(Args)]
pub struct CreateArgs {
    /// Archivos a incluir (cualquier tipo).
    #[arg(required = true)]
    pub files: Vec<PathBuf>,
    #[arg(short, long)]
    pub output: PathBuf,
}

#[derive(Args)]
pub struct ListArgs {
    pub input: PathBuf,
    #[arg(long)]
    pub password: Option<String>,
}

#[derive(Args)]
pub struct ExtractArgs {
    pub input: PathBuf,
    /// Carpeta de salida (se crea si no existe).
    #[arg(short, long)]
    pub output: PathBuf,
    /// Nombres de los archivos a extraer (todos si se omite).
    pub names: Vec<String>,
    #[arg(long)]
    pub password: Option<String>,
}

struct LineProgress;
impl Progress for LineProgress {
    fn report(&self, done: u64, total: u64, message: &str) {
        eprintln!("[{done}/{total}] {message}");
    }
}

pub fn run(c: PortfolioCommand, json: bool) -> anyhow::Result<()> {
    let ctx = JobContext::new(&LineProgress, Default::default());
    match c {
        PortfolioCommand::Create(a) => {
            let r = pliego_portfolio::create_in(&a.files, &a.output, &ctx, pliego_core::Lang::from_env())?;
            print_report(&r, json)?;
        }
        PortfolioCommand::List(a) => {
            let l = pliego_portfolio::list(&a.input, a.password.as_deref())?;
            if json {
                println!("{}", serde_json::to_string_pretty(&l)?);
            } else if l.entries.is_empty() {
                println!("Sin archivos incrustados.");
            } else {
                println!(
                    "{}",
                    if l.is_portfolio {
                        "Cartera PDF"
                    } else {
                        "PDF con archivos adjuntos"
                    }
                );
                for e in l.entries {
                    println!("{:>10}  {}  {}", e.size, e.name, e.mime_type.unwrap_or_default());
                }
            }
        }
        PortfolioCommand::Extract(a) => {
            let r = pliego_portfolio::extract(&a.input, &a.output, &a.names, a.password.as_deref(), &ctx)?;
            print_report(&r, json)?;
        }
    }
    Ok(())
}

fn print_report(r: &pliego_core::Report, json: bool) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(r)?);
    } else {
        for w in &r.warnings {
            eprintln!("aviso: {w}");
        }
        println!("{}", r.summary);
    }
    Ok(())
}
