//! `pliego index add|search`: índice de texto de carpetas de PDFs.

use std::path::PathBuf;

use clap::{Args, Subcommand};
use pliego_core::{JobContext, Progress};
use pliego_index::Index;

#[derive(Subcommand)]
pub enum IndexCommand {
    /// Indexa (o actualiza) una carpeta de PDFs, recursivo. Solo se relee lo que cambió.
    Add(AddArgs),
    /// Busca en el índice: palabras, "frase exacta", +obligatoria, -excluida.
    Search(SearchArgs),
}

#[derive(Args)]
pub struct AddArgs {
    /// Carpeta donde se guarda el índice (se crea si no existe).
    pub index: PathBuf,
    /// Carpeta de PDFs a indexar.
    pub folder: PathBuf,
}

#[derive(Args)]
pub struct SearchArgs {
    /// Carpeta del índice.
    pub index: PathBuf,
    pub query: String,
    /// Máximo de resultados.
    #[arg(short = 'n', long, default_value_t = 20)]
    pub limit: usize,
}

struct LineProgress;
impl Progress for LineProgress {
    fn report(&self, done: u64, total: u64, message: &str) {
        eprintln!("[{done}/{total}] {message}");
    }
}

pub fn run(c: IndexCommand, json: bool) -> anyhow::Result<()> {
    match c {
        IndexCommand::Add(a) => {
            let idx = Index::open(&a.index)?;
            let ctx = JobContext::new(&LineProgress, Default::default());
            let r = idx.add_folder(&a.folder, &ctx)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&r)?);
            } else {
                for w in &r.warnings {
                    eprintln!("aviso: {w}");
                }
                println!("{}", r.summary);
            }
        }
        IndexCommand::Search(a) => {
            let hits = Index::open(&a.index)?.search(&a.query, a.limit)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&hits)?);
            } else if hits.is_empty() {
                println!("Sin resultados.");
            } else {
                for h in hits {
                    println!("{}  p. {}", h.path.display(), h.page + 1);
                    println!("    {}", h.snippet.split_whitespace().collect::<Vec<_>>().join(" "));
                }
            }
        }
    }
    Ok(())
}
