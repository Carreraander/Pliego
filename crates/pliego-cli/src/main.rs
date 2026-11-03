//! `pliego`: la misma funcionalidad que la app, para scripts y lotes.
//!
//! Cada módulo añade su subcomando en `src/commands/<modulo>.rs` y UNA línea en `Command`
//! y en `run` (ver docs/CONVENTIONS.md).

use clap::{Parser, Subcommand};

mod commands;

#[derive(Parser)]
#[command(name = "pliego", version, about = "Pliego: herramientas PDF libres")]
struct Cli {
    /// Salida en JSON (para scripts).
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Información del documento.
    Info(commands::view::InfoArgs),
    /// Renderiza páginas a PNG.
    Render(commands::view::RenderArgs),
    /// Extrae el texto de las páginas.
    Text(commands::view::TextArgs),
    /// Busca texto y lista las coincidencias.
    Search(commands::view::SearchArgs),
    /// Anotaciones: listar, añadir, borrar.
    #[command(subcommand)]
    Annot(commands::annot::AnnotCommand),
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Info(a) => commands::view::info(a, cli.json),
        Command::Render(a) => commands::view::render(a, cli.json),
        Command::Text(a) => commands::view::text(a, cli.json),
        Command::Search(a) => commands::view::search(a, cli.json),
        Command::Annot(c) => commands::annot::run(c, cli.json),
    }
}
