//! `pliego`: la misma funcionalidad que la app, para scripts y lotes.
//!
//! Cada módulo añade su subcomando en `src/commands/<modulo>.rs` y UNA línea en `Command`
//! y en el `match`, debajo de la marca de su flujo en las ranuras del final (ver docs/CONVENTIONS.md).

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
    /// Firma un PDF con un certificado.
    Sign(commands::sign::SignArgs),
    /// Comprueba las firmas de un PDF.
    Verify(commands::sign::VerifyArgs),
    /// Flujo de firmas local con varios firmantes en orden.
    Signflow(commands::signflow::SignflowArgs),
    /// Compara dos versiones de un documento.
    Compare(commands::compare::CompareArgs),
    /// Reduce el tamaño de un PDF o analiza dónde se va el espacio.
    Optimize(commands::optimize::OptimizeArgs),
    /// Combina varios PDFs en uno.
    Merge(commands::pages::MergeArgs),
    /// Divide un PDF en varios.
    Split(commands::pages::SplitArgs),
    /// Operaciones sobre páginas: extraer, insertar, borrar, girar, mover, recortar, etiquetas.
    Pages(commands::pages::PagesArgs),
    /// Sellos de documento: encabezado y pie, marca de agua, fondo y numeración Bates.
    Stamp(commands::stamp::StampArgs),
    /// Cifrar, permisos y saneado.
    #[command(subcommand)]
    Protect(commands::protect::ProtectCommand),
    /// Anotaciones: listar, añadir, borrar.
    #[command(subcommand)]
    Annot(commands::annot::AnnotCommand),
    /// Formularios: campos, rellenar, aplanar, importar/exportar, relleno masivo.
    #[command(subcommand)]
    Form(commands::form::FormCommand),
    /// Diseñador de formularios: añadir, modificar y borrar campos, orden de tabulación.
    #[command(subcommand)]
    Formdesign(commands::formdesign::FormDesignCommand),
    /// Crea un PDF desde imágenes.
    Create(commands::convert::CreateArgs),
    /// Exporta un PDF a imágenes o texto.
    Export(commands::convert::ExportArgs),
    /// Producción de impresión: tintas, preflight y separaciones.
    #[command(subcommand)]
    Print(commands::print::PrintCommand),
    /// Asistente local: resumir, preguntar y datos clave.
    #[command(subcommand)]
    Assistant(commands::assistant::AssistantCommand),
    /// Busca datos personales y los redacta de forma irreversible.
    Redact(commands::redact::RedactArgs),
    /// Accesibilidad: comprobar y corregir idioma, título y texto alternativo.
    #[command(subcommand)]
    A11y(commands::a11y::A11yCommand),
    /// PDF/A: validar (y próximamente convertir).
    #[command(subcommand)]
    Pdfa(commands::pdfa::PdfaCommand),
    // ── Ranuras: añade tu línea justo debajo de tu marca (evita conflictos entre flujos) ──
    // [W06 ocr]

    // [W11 edit]

    // [W14 office]

    // [W15 run e índice]

    // [W13/W17/W12 extra]

    // [libre A]

    // [libre B]
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Info(a) => commands::view::info(a, cli.json),
        Command::Render(a) => commands::view::render(a, cli.json),
        Command::Text(a) => commands::view::text(a, cli.json),
        Command::Search(a) => commands::view::search(a, cli.json),
        Command::Sign(a) => commands::sign::sign(a, cli.json),
        Command::Verify(a) => commands::sign::verify(a, cli.json),
        Command::Signflow(a) => commands::signflow::run(a, cli.json),
        Command::Compare(a) => commands::compare::compare(a, cli.json),
        Command::Optimize(a) => commands::optimize::optimize(a, cli.json),
        Command::Merge(a) => commands::pages::merge(a, cli.json),
        Command::Split(a) => commands::pages::split(a, cli.json),
        Command::Pages(a) => commands::pages::pages(a, cli.json),
        Command::Stamp(a) => commands::stamp::run(a, cli.json),
        Command::Protect(c) => commands::protect::run(c, cli.json),
        Command::Annot(c) => commands::annot::run(c, cli.json),
        Command::Form(c) => commands::form::run(c, cli.json),
        Command::Formdesign(c) => commands::formdesign::run(c, cli.json),
        Command::Create(a) => commands::convert::create(a, cli.json),
        Command::Export(a) => commands::convert::export(a, cli.json),
        Command::Print(c) => commands::print::run(c, cli.json),
        Command::Assistant(c) => commands::assistant::run(c, cli.json),
        Command::Redact(a) => commands::redact::run(a, cli.json),
        Command::A11y(c) => commands::a11y::run(c, cli.json),
        Command::Pdfa(c) => commands::pdfa::run(c, cli.json),
        // ── Ranuras: añade tu línea justo debajo de tu marca (evita conflictos entre flujos) ──
        // [W06 ocr]

        // [W11 edit]

        // [W14 office]

        // [W15 run e índice]

        // [W13/W17/W12 extra]

        // [libre A]

        // [libre B]
    }
}
