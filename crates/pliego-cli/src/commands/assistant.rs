//! `pliego assistant`: asistente local (resumir, preguntar, datos clave). Sin red.

use std::path::PathBuf;

use anyhow::Context;
use clap::{Args, Subcommand};
use pliego_assistant::{
    Assistant, DocIndex, DocumentText, ModelSpec, catalog, default_models_dir, download_model, extract_key_data,
    installed_path,
};
use pliego_core::JobContext;

use super::view::OpenArgs;

#[derive(Subcommand)]
pub enum AssistantCommand {
    /// Lista los modelos disponibles con su licencia y requisitos.
    Models,
    /// Descarga un modelo del catálogo (única operación con red; comprueba tamaño y SHA-256).
    Download(DownloadArgs),
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
pub struct DownloadArgs {
    /// Identificador del modelo (ver `pliego assistant models`).
    pub model: String,
    /// Carpeta de destino (por defecto, la carpeta de modelos de Pliego).
    #[arg(long)]
    pub dir: Option<PathBuf>,
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
        AssistantCommand::Download(a) => download(a, json),
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
    let dir = default_models_dir();
    if json {
        let v: Vec<_> = catalog()
            .iter()
            .map(|m| {
                let mut v = serde_json::to_value(m).unwrap_or_default();
                v["installed_path"] = serde_json::json!(installed_path(m, &dir));
                v
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&v)?);
    } else {
        for m in catalog() {
            let state = if installed_path(m, &dir).is_some() {
                "instalado"
            } else {
                ""
            };
            println!(
                "{:<28} {:<10} {:>5} MB  RAM {:.0} GB  {}{} {state}",
                m.id,
                m.license,
                m.requirements.download_mb,
                m.requirements.ram_gb,
                m.name,
                if m.recommended { " ★" } else { "" }
            );
        }
        println!("\nCarpeta de modelos: {}", dir.display());
    }
    Ok(())
}

fn download(a: DownloadArgs, json: bool) -> anyhow::Result<()> {
    let spec = ModelSpec::find(&a.model).with_context(|| format!("no existe el modelo «{}»", a.model))?;
    let dir = a.dir.unwrap_or_else(default_models_dir);
    let progress = Stderr(json);
    let ctx = JobContext::new(&progress, pliego_core::CancelToken::new());
    let path = download_model(spec, &dir, &ctx)?;
    if json {
        println!("{}", serde_json::json!({ "model": spec.id, "path": path }));
    } else {
        eprintln!();
        println!("{}", path.display());
    }
    Ok(())
}

/// Progreso de la descarga en la terminal (no se muestra con `--json`).
struct Stderr(bool);

impl pliego_core::Progress for Stderr {
    fn report(&self, done: u64, total: u64, message: &str) {
        if !self.0 && total > 0 {
            eprint!("\r{message}: {} %", done * 100 / total);
        }
    }
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
