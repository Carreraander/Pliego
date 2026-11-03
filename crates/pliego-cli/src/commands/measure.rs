//! `pliego measure scale|add|list`: medidas sobre planos.

use std::path::PathBuf;

use anyhow::{Context, bail};
use clap::{Args, Subcommand};
use pliego_core::PageRange;
use pliego_measure::{Pt, Scale, Unit};

#[derive(Subcommand)]
pub enum MeasureCommand {
    /// Calibra la escala de páginas: una medida del papel equivale a una real.
    Scale(ScaleArgs),
    /// Mide una figura y la deja como anotación de medida.
    Add(AddArgs),
    /// Lista las escalas de las páginas y las medidas del documento.
    List(ListArgs),
}

#[derive(Args)]
pub struct ScaleArgs {
    pub input: PathBuf,
    #[arg(short, long)]
    pub output: PathBuf,
    /// Páginas (1, 2-4, all…).
    #[arg(short, long, default_value = "all")]
    pub pages: PageRange,
    /// Medida sobre el papel con su unidad, p. ej. «1cm» (alternativa: --points).
    #[arg(long, conflicts_with = "points")]
    pub paper: Option<String>,
    /// Medida sobre el papel en puntos PDF (1/72 in), p. ej. la longitud de un tramo conocido del plano.
    #[arg(long)]
    pub points: Option<f64>,
    /// Medida real equivalente con su unidad, p. ej. «5m».
    #[arg(long)]
    pub real: String,
    #[arg(long)]
    pub password: Option<String>,
}

#[derive(Args)]
pub struct AddArgs {
    pub input: PathBuf,
    #[arg(short, long)]
    pub output: PathBuf,
    /// Página (desde 1).
    #[arg(short, long, default_value_t = 1)]
    pub page: u32,
    /// Puntos «x,y x,y …» en puntos PDF de la página.
    #[arg(long)]
    pub points: String,
    /// Figura cerrada: perímetro y área.
    #[arg(long)]
    pub closed: bool,
    #[arg(long)]
    pub password: Option<String>,
}

#[derive(Args)]
pub struct ListArgs {
    pub input: PathBuf,
    #[arg(long)]
    pub password: Option<String>,
}

/// «5m», «2,5 cm», «10 ft» → (valor, unidad).
fn quantity(s: &str) -> anyhow::Result<(f64, Unit)> {
    let s = s.trim();
    let split = s
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == ','))
        .with_context(|| format!("«{s}» necesita una unidad (mm, cm, m, km, in, ft, pt)"))?;
    let value: f64 = s[..split]
        .replace(',', ".")
        .parse()
        .with_context(|| format!("número no válido en «{s}»"))?;
    Ok((value, s[split..].parse()?))
}

fn parse_points(s: &str) -> anyhow::Result<Vec<Pt>> {
    s.split_whitespace()
        .map(|p| {
            let (x, y) = p.split_once(',').with_context(|| format!("punto «{p}»: usa x,y"))?;
            Ok([x.parse()?, y.parse()?])
        })
        .collect()
}

pub fn run(c: MeasureCommand, json: bool) -> anyhow::Result<()> {
    match c {
        MeasureCommand::Scale(a) => {
            let (real, unit) = quantity(&a.real)?;
            let scale = match (&a.paper, a.points) {
                (Some(p), None) => {
                    let (v, u) = quantity(p)?;
                    Scale::calibrate(v * u.points(), real, unit)?
                }
                (None, Some(pts)) => Scale::calibrate(pts, real, unit)?,
                _ => bail!("indica la medida del papel con --paper o --points"),
            };
            let mut doc = pliego_organize::open(&a.input, a.password.as_deref())?;
            let sel = a.pages.resolve(pliego_organize::page_count(&doc)?)?;
            for &p in &sel {
                pliego_measure::set_scale(&mut doc, p, &scale)?;
            }
            pliego_organize::save(&doc, &a.output)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&scale)?);
            } else {
                println!("Escala fijada en {} páginas: {}", sel.len(), scale.ratio_text());
            }
        }
        MeasureCommand::Add(a) => {
            let mut doc = pliego_organize::open(&a.input, a.password.as_deref())?;
            let page = a.page.checked_sub(1).context("las páginas empiezan en 1")?;
            let scale = pliego_measure::page_scale(&doc, page)?.with_context(|| {
                format!(
                    "la página {} no tiene escala: calíbrala antes con `pliego measure scale`",
                    a.page
                )
            })?;
            let (m, _) = pliego_measure::add_measurement(&mut doc, page, &scale, &parse_points(&a.points)?, a.closed)?;
            pliego_organize::save(&doc, &a.output)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&m)?);
            } else {
                println!("{}", m.text);
            }
        }
        MeasureCommand::List(a) => {
            let doc = pliego_organize::open(&a.input, a.password.as_deref())?;
            let n = pliego_organize::page_count(&doc)?;
            let mut scales = Vec::new();
            for p in 0..n {
                if let Some(s) = pliego_measure::page_scale(&doc, p)? {
                    scales.push((p, s));
                }
            }
            let items = pliego_measure::list_measurements(&doc, None)?;
            if json {
                let sc: Vec<_> = scales
                    .iter()
                    .map(|(p, s)| serde_json::json!({ "page": p + 1, "scale": s }))
                    .collect();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({ "scales": sc, "measurements": items }))?
                );
            } else {
                for (p, s) in &scales {
                    println!("Página {}: escala {}", p + 1, s.ratio_text());
                }
                for i in &items {
                    println!("Página {}: {}", i.page + 1, i.measurement.text);
                }
                if scales.is_empty() && items.is_empty() {
                    println!("Sin escalas ni medidas.");
                }
            }
        }
    }
    Ok(())
}
