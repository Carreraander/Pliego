use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use clap::{Args, Subcommand};
use pliego_a11y::{AutotagOptions, FixOptions, Severity};
use pliego_core::JobContext;

#[derive(Subcommand)]
pub enum A11yCommand {
    /// Comprueba la accesibilidad del documento y sugiere correcciones.
    Check(CheckArgs),
    /// Corrige idioma, título, texto alternativo y orden de tabulación.
    Fix(FixArgs),
    /// Etiqueta un documento sin etiquetar (encabezados, listas, párrafos y figuras).
    Autotag(AutotagArgs),
}

#[derive(Args)]
pub struct AutotagArgs {
    /// PDF de entrada (sin etiquetar).
    pub input: PathBuf,
    /// PDF de salida (no se sobrescribe la entrada).
    #[arg(short, long)]
    pub output: PathBuf,
    /// Idioma principal (es, es-ES, en-US...).
    #[arg(long)]
    pub lang: Option<String>,
    /// Título del documento.
    #[arg(long)]
    pub title: Option<String>,
}

#[derive(Args)]
pub struct CheckArgs {
    /// PDF de entrada.
    pub input: PathBuf,
    /// Contraseña del documento, si la necesita.
    #[arg(long)]
    pub password: Option<String>,
}

#[derive(Args)]
pub struct FixArgs {
    /// PDF de entrada.
    pub input: PathBuf,
    /// PDF de salida (no se sobrescribe la entrada).
    #[arg(short, long)]
    pub output: PathBuf,
    /// Idioma principal (es, es-ES, en-US...).
    #[arg(long)]
    pub lang: Option<String>,
    /// Título del documento.
    #[arg(long)]
    pub title: Option<String>,
    /// Texto alternativo: `OBJETO=texto` (el objeto lo indica `a11y check`). Repetible.
    #[arg(long = "alt", value_name = "OBJETO=TEXTO")]
    pub alt: Vec<String>,
    /// Fija el orden de tabulación de las páginas a «estructura».
    #[arg(long)]
    pub tab_order: bool,
    /// Contraseña del documento, si la necesita.
    #[arg(long)]
    pub password: Option<String>,
}

fn parse_alt(s: &str) -> anyhow::Result<(i32, String)> {
    let (n, text) = s
        .split_once('=')
        .with_context(|| format!("«{s}»: usa el formato OBJETO=texto"))?;
    let n = n
        .trim()
        .parse::<i32>()
        .with_context(|| format!("«{n}» no es un número de objeto"))?;
    Ok((n, text.to_string()))
}

pub fn run(cmd: A11yCommand, json: bool) -> anyhow::Result<()> {
    match cmd {
        A11yCommand::Check(a) => check(&a.input, a.password.as_deref(), json),
        A11yCommand::Autotag(a) => {
            let opts = AutotagOptions {
                language: a.lang,
                title: a.title,
            };
            let rep = pliego_a11y::autotag_file(&a.input, &a.output, &opts, &JobContext::silent())?;
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
        A11yCommand::Fix(a) => {
            let alt_texts = a.alt.iter().map(|s| parse_alt(s)).collect::<anyhow::Result<Vec<_>>>()?;
            let opts = FixOptions {
                language: a.lang,
                title: a.title,
                alt_texts,
                tab_order: a.tab_order,
            };
            let rep = pliego_a11y::fix_file(&a.input, &a.output, &opts, a.password.as_deref())?;
            if json {
                println!("{}", serde_json::to_string_pretty(&rep)?);
            } else {
                println!("{}", rep.summary);
                println!("-> {}", a.output.display());
            }
            Ok(())
        }
    }
}

fn check(input: &Path, password: Option<&str>, json: bool) -> anyhow::Result<()> {
    let rep = pliego_a11y::check_file(input, password)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&rep)?);
    } else {
        println!(
            "{}: {} página(s), {}.",
            input.display(),
            rep.pages,
            if rep.tagged { "etiquetado" } else { "sin etiquetar" }
        );
        for i in &rep.issues {
            let tag = match i.severity {
                Severity::Error => "error",
                Severity::Warning => "aviso",
            };
            let place = match (i.page, i.xref) {
                (Some(p), Some(x)) => format!(" (página {}, objeto {x})", p + 1),
                (Some(p), None) => format!(" (página {})", p + 1),
                (None, Some(x)) => format!(" (objeto {x})"),
                (None, None) => String::new(),
            };
            println!("[{tag}]{place} {}\n    Cómo arreglarlo: {}", i.message, i.suggestion);
        }
        println!("{} error(es), {} aviso(s).", rep.errors(), rep.warnings());
    }
    if !rep.passes() {
        bail!("el documento no cumple las comprobaciones de accesibilidad");
    }
    Ok(())
}
