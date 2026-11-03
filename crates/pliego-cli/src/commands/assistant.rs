//! `pliego assistant`: asistente local (resumir, preguntar, datos clave). Sin red.

use std::path::PathBuf;

use anyhow::Context;
use clap::{Args, Subcommand};
use pliego_assistant::{Assistant, DocIndex, DocumentText, catalog, extract_key_data};
use pliego_core::JobContext;

use super::view::OpenArgs;

#[derive(Subcommand)]
pub enum AssistantCommand {
    /// Lista los modelos disponibles con su licencia y requisitos.
    Models,
    /// Fechas e importes del documento, con su página (no necesita modelo).
    Keydata(DocArgs),
    /// Pasajes más relevantes para una pregunta (búsqueda local, sin modelo).
    Passages(PassagesArgs),
    /// Responde una pregunta citando páginas (requiere un modelo GGUF).
    Ask(AskArgs),
    /// Resume el documento (requiere un modelo GGUF).
    Summarize(SummarizeArgs),
}

#[derive(Args)]
pub struct DocArgs {
    #[command(flatten)]
    pub open: OpenArgs,
}

#[derive(Args)]
pub struct PassagesArgs {
    #[command(flatten)]
    pub open: OpenArgs,
    pub question: String,
    #[arg(short = 'k', long, default_value_t = 5)]
    pub top: usize,
}

#[derive(Args)]
pub struct AskArgs {
    #[command(flatten)]
    pub open: OpenArgs,
    pub question: String,
    /// Archivo del modelo (.gguf).
    #[arg(long)]
    pub model: PathBuf,
}

#[derive(Args)]
pub struct SummarizeArgs {
    #[command(flatten)]
    pub open: OpenArgs,
    /// Archivo del modelo (.gguf).
    #[arg(long)]
    pub model: PathBuf,
}

pub fn run(cmd: AssistantCommand, json: bool) -> anyhow::Result<()> {
    match cmd {
        AssistantCommand::Models => models(json),
        AssistantCommand::Keydata(a) => keydata(a, json),
        AssistantCommand::Passages(a) => passages(a, json),
        AssistantCommand::Ask(a) => ask(a, json),
        AssistantCommand::Summarize(a) => summarize(a, json),
    }
}

fn text_of(open: &OpenArgs) -> anyhow::Result<DocumentText> {
    let doc = open.open()?;
    Ok(DocumentText::from_engine(&doc, &JobContext::silent())?)
}

fn models(json: bool) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(catalog())?);
    } else {
        for m in catalog() {
            println!(
                "{:<28} {:<10} {:>5} MB  RAM {:.0} GB  {}",
                m.id, m.license, m.requirements.download_mb, m.requirements.ram_gb, m.name
            );
        }
    }
    Ok(())
}

fn keydata(a: DocArgs, json: bool) -> anyhow::Result<()> {
    let items = extract_key_data(&text_of(&a.open)?);
    if json {
        println!("{}", serde_json::to_string_pretty(&items)?);
    } else {
        for d in items {
            println!("p. {:<4} {:?}\t{}", d.page + 1, d.kind, d.value);
        }
    }
    Ok(())
}

fn passages(a: PassagesArgs, json: bool) -> anyhow::Result<()> {
    let found = DocIndex::lexical(&text_of(&a.open)?).search(&a.question, None, a.top);
    if json {
        let v: Vec<_> = found
            .iter()
            .map(|p| serde_json::json!({ "page": p.page + 1, "score": p.score, "text": p.text }))
            .collect();
        println!("{}", serde_json::to_string_pretty(&v)?);
    } else {
        for p in found {
            println!("[p. {}] {}\n", p.page + 1, p.text);
        }
    }
    Ok(())
}

fn ask(a: AskArgs, json: bool) -> anyhow::Result<()> {
    let text = text_of(&a.open)?;
    let assistant = Assistant::load(&a.model).context("no se pudo cargar el modelo")?;
    let ans = assistant.ask(&text, &a.question)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&ans)?);
    } else {
        println!("{}", ans.text);
        for c in ans.citations {
            println!("  · p. {}: {}", c.page + 1, c.snippet);
        }
    }
    Ok(())
}

fn summarize(a: SummarizeArgs, json: bool) -> anyhow::Result<()> {
    let text = text_of(&a.open)?;
    let assistant = Assistant::load(&a.model).context("no se pudo cargar el modelo")?;
    let s = assistant.summarize(&text, &JobContext::silent())?;
    if json {
        println!("{}", serde_json::to_string_pretty(&s)?);
    } else {
        println!("{}", s.text);
    }
    Ok(())
}
