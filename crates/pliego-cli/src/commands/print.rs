use std::path::PathBuf;

use anyhow::{Context, bail};
use clap::{Args, Subcommand};
use pliego_core::{JobContext, Report};
use pliego_print::{InkKind, Profile, Severity};

#[derive(Subcommand)]
pub enum PrintCommand {
    /// Lista las tintas (de proceso y colores directos) que usa el documento.
    Inks(InksArgs),
    /// Comprueba el documento para imprenta y genera un informe.
    Preflight(PreflightArgs),
    /// Genera las planchas CMYK de una página como imágenes.
    Separations(SeparationsArgs),
    /// Sube al mínimo el grosor de las líneas demasiado finas.
    Hairlines(HairlinesArgs),
    /// Añade marcas de corte y de registro, y amplía las cajas de sangrado.
    Marks(MarksArgs),
    /// Convierte a PDF/X (requiere Ghostscript y un perfil ICC CMYK).
    Pdfx(PdfxArgs),
}

#[derive(Args)]
pub struct HairlinesArgs {
    pub input: PathBuf,
    #[arg(short, long)]
    pub output: PathBuf,
    /// Grosor mínimo en puntos.
    #[arg(long, default_value_t = 0.25)]
    pub min: f32,
    #[arg(long)]
    pub password: Option<String>,
}

#[derive(Args)]
pub struct MarksArgs {
    pub input: PathBuf,
    #[arg(short, long)]
    pub output: PathBuf,
    /// Sangrado en milímetros.
    #[arg(long, default_value_t = 3.0)]
    pub bleed_mm: f32,
    /// Separación entre el sangrado y las marcas, en milímetros.
    #[arg(long, default_value_t = 3.0)]
    pub gap_mm: f32,
    /// Longitud de las marcas, en milímetros.
    #[arg(long, default_value_t = 6.0)]
    pub length_mm: f32,
    /// No dibujar marcas de registro.
    #[arg(long)]
    pub no_registration: bool,
    /// Páginas (desde 1), p. ej. `1-3,5`. Por defecto, todas.
    #[arg(long)]
    pub pages: Option<String>,
    #[arg(long)]
    pub password: Option<String>,
}

#[derive(Clone, Copy, clap::ValueEnum)]
pub enum Level {
    X1a,
    X4,
}

#[derive(Args)]
pub struct PdfxArgs {
    pub input: PathBuf,
    #[arg(short, long)]
    pub output: PathBuf,
    /// Perfil ICC CMYK de la condición de impresión.
    #[arg(long)]
    pub icc: PathBuf,
    #[arg(long, value_enum, default_value = "x1a")]
    pub level: Level,
    /// Identificador de la condición de impresión (FOGRA39, SWOP...).
    #[arg(long, default_value = "FOGRA39")]
    pub condition_id: String,
    /// Descripción de la condición de impresión.
    #[arg(long, default_value = "Impresión comercial")]
    pub condition: String,
    /// Ruta de Ghostscript si no está en el PATH.
    #[arg(long)]
    pub ghostscript: Option<PathBuf>,
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
        PrintCommand::Hairlines(a) => hairlines(a, json),
        PrintCommand::Marks(a) => marks(a, json),
        PrintCommand::Pdfx(a) => pdfx(a, json),
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

fn print_report(rep: &Report, json: bool) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(rep)?);
    } else {
        println!("{}", rep.summary);
        for w in &rep.warnings {
            eprintln!("Aviso: {w}");
        }
    }
    Ok(())
}

fn hairlines(a: HairlinesArgs, json: bool) -> anyhow::Result<()> {
    let mut doc = pliego_print::open(&a.input, a.password.as_deref())?;
    let fix = pliego_print::fix_hairlines(&mut doc, a.min, &JobContext::silent())?;
    pliego_print::save(&doc, &a.output)?;
    let summary = format!(
        "{} línea(s) corregidas en {} página(s); mínimo {} pt.",
        fix.widths_changed, fix.pages_changed, a.min
    );
    print_report(&Report::with_output(&a.output, summary), json)
}

fn marks(a: MarksArgs, json: bool) -> anyhow::Result<()> {
    const MM: f32 = 72.0 / 25.4;
    let mut doc = pliego_print::open(&a.input, a.password.as_deref())?;
    let total = doc.page_count().map_err(|e| anyhow::anyhow!(e.to_string()))?.max(0) as usize;
    let pages = match &a.pages {
        Some(spec) => Some(
            spec.parse::<pliego_core::PageRange>()?
                .resolve(total as u32)?
                .into_iter()
                .map(|i| i as usize)
                .collect::<Vec<usize>>(),
        ),
        None => None,
    };
    let opts = pliego_print::MarkOptions {
        bleed: a.bleed_mm * MM,
        gap: a.gap_mm * MM,
        length: a.length_mm * MM,
        registration_marks: !a.no_registration,
        ..Default::default()
    };
    let n = pliego_print::add_printer_marks(&mut doc, &opts, pages.as_deref(), &JobContext::silent())?;
    pliego_print::save(&doc, &a.output)?;
    let summary = format!("Marcas añadidas en {n} página(s); sangrado de {} mm.", a.bleed_mm);
    print_report(&Report::with_output(&a.output, summary), json)
}

fn pdfx(a: PdfxArgs, json: bool) -> anyhow::Result<()> {
    let opts = pliego_print::PdfxOptions {
        level: match a.level {
            Level::X1a => pliego_print::PdfxLevel::X1a,
            Level::X4 => pliego_print::PdfxLevel::X4,
        },
        icc_profile: a.icc,
        condition_identifier: a.condition_id,
        condition: a.condition,
        title: None,
        ghostscript: a.ghostscript,
    };
    let report = pliego_print::to_pdfx(&a.input, &a.output, &opts, &JobContext::silent())?;
    print_report(&report, json)
}
