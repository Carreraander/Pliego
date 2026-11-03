use std::path::PathBuf;

use anyhow::{Context, bail};
use clap::{Args, Subcommand};
use pliego_core::JobContext;
use pliego_print::{InkKind, Profile, Severity};

#[derive(Subcommand)]
pub enum PrintCommand {
    /// Lista las tintas (de proceso y colores directos) que usa el documento.
    Inks(InksArgs),
    /// Comprueba el documento para imprenta y genera un informe.
    Preflight(PreflightArgs),
    /// Genera las planchas CMYK de una página como imágenes.
    Separations(SeparationsArgs),
}

#[derive(Args)]
pub struct InksArgs {
    pub input: PathBuf,
    #[arg(long)]
    pub password: Option<String>,
}

#[derive(Args)]
pub struct PreflightArgs {
    pub input: PathBuf,
    /// Perfil: basic, print o pdfx.
    #[arg(long, default_value = "print")]
    pub profile: String,
    #[arg(long)]
    pub password: Option<String>,
}

#[derive(Args)]
pub struct SeparationsArgs {
    pub input: PathBuf,
    /// Página (desde 1).
    #[arg(long, default_value_t = 1)]
    pub page: usize,
    /// Resolución de las imágenes.
    #[arg(long, default_value_t = 72.0)]
    pub dpi: f32,
    /// Carpeta donde se escriben las cuatro planchas PNG.
    #[arg(short, long)]
    pub output: PathBuf,
    #[arg(long)]
    pub password: Option<String>,
}

pub fn run(cmd: PrintCommand, json: bool) -> anyhow::Result<()> {
    match cmd {
        PrintCommand::Inks(a) => inks(a, json),
        PrintCommand::Preflight(a) => preflight(a, json),
        PrintCommand::Separations(a) => separations(a, json),
    }
}

fn inks(a: InksArgs, json: bool) -> anyhow::Result<()> {
    let doc = pliego_print::open(&a.input, a.password.as_deref())?;
    let list = pliego_print::inks(&doc)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&list)?);
        return Ok(());
    }
    if list.is_empty() {
        println!("El documento no usa tintas de proceso ni colores directos (solo RGB o grises).");
    }
    for ink in list {
        let kind = match ink.kind {
            InkKind::Process => "proceso",
            InkKind::Spot => "directo",
        };
        let pages: Vec<String> = ink.pages.iter().map(|p| (p + 1).to_string()).collect();
        println!("{:<24} {kind:<8} páginas {}", ink.name, pages.join(", "));
    }
    Ok(())
}

fn preflight(a: PreflightArgs, json: bool) -> anyhow::Result<()> {
    let profile = Profile::by_name(&a.profile)
        .with_context(|| format!("perfil desconocido «{}» (usa basic, print o pdfx)", a.profile))?;
    let doc = pliego_print::open(&a.input, a.password.as_deref())?;
    let report = pliego_print::preflight(&doc, &profile, &JobContext::silent())?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        for f in &report.findings {
            let tag = match f.severity {
                Severity::Error => "ERROR",
                Severity::Warning => "AVISO",
                Severity::Info => "INFO ",
            };
            println!("{tag} {}", f.message);
        }
        println!(
            "Perfil «{}»: {} error(es), {} aviso(s) en {} página(s).",
            report.profile,
            report.errors(),
            report.warnings(),
            report.pages
        );
    }
    if !report.passed() {
        bail!("el documento no cumple el perfil «{}»", report.profile);
    }
    Ok(())
}

fn separations(a: SeparationsArgs, json: bool) -> anyhow::Result<()> {
    if a.page == 0 {
        bail!("las páginas se numeran desde 1");
    }
    let doc = pliego_print::open(&a.input, a.password.as_deref())?;
    let seps = pliego_print::separations(&doc, a.page - 1, a.dpi)?;
    std::fs::create_dir_all(&a.output).with_context(|| format!("no se pudo crear {}", a.output.display()))?;
    let stem = a
        .input
        .file_stem()
        .map_or("pagina".into(), |s| s.to_string_lossy().into_owned());
    let mut files = Vec::new();
    for plate in &seps.plates {
        let path = a.output.join(format!(
            "{stem}-p{}-{}.png",
            a.page,
            plate.channel.name().to_lowercase()
        ));
        std::fs::write(&path, plate.to_png()?).with_context(|| format!("no se pudo escribir {}", path.display()))?;
        files.push(serde_json::json!({
            "channel": plate.channel.name(), "file": path, "coverage": plate.coverage(),
        }));
    }
    if json {
        let out = serde_json::json!({ "plates": files, "max_total_ink": seps.max_total_ink });
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        for f in &files {
            println!(
                "{:<8} {:>5.1} %  {}",
                f["channel"].as_str().unwrap_or(""),
                f["coverage"].as_f64().unwrap_or(0.0) * 100.0,
                f["file"].as_str().unwrap_or("")
            );
        }
        println!("Cobertura total máxima de tinta: {:.0} %", seps.max_total_ink * 100.0);
    }
    Ok(())
}
