use std::path::PathBuf;

use anyhow::{Context, bail};
use clap::Args;
use pliego_core::{JobContext, NoProgress};
use pliego_optimize::{OptimizeOptions, Preset, SpaceUsage, human};

#[derive(Args)]
pub struct OptimizeArgs {
    /// PDF de entrada.
    pub input: PathBuf,
    /// Archivo de salida (obligatorio salvo con --analyze; nunca puede ser la entrada).
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    /// Preajuste: pantalla (96 ppp), ebook (150 ppp), impresion (300 ppp) o sin-perdida.
    #[arg(long, default_value = "ebook")]
    pub preset: Preset,
    /// Solo muestra cómo se reparte el espacio del archivo, sin escribir nada.
    #[arg(long)]
    pub analyze: bool,
    /// Resolución objetivo de las imágenes (sustituye a la del preajuste).
    #[arg(long)]
    pub dpi: Option<f32>,
    /// Calidad JPEG de 1 a 100 (sustituye a la del preajuste).
    #[arg(long)]
    pub quality: Option<u8>,
    /// Pedir linealización para web (aún no disponible; se avisa).
    #[arg(long)]
    pub linearize: bool,
    /// Contraseña del documento, si la tiene.
    #[arg(long)]
    pub password: Option<String>,
}

fn print_usage(label: &str, u: &SpaceUsage) {
    println!("{label}: {}", human(u.total_bytes));
    for (name, b) in [
        ("Imágenes", u.images_bytes),
        ("Fuentes", u.fonts_bytes),
        ("Contenido de páginas", u.content_bytes),
        ("Otros flujos", u.other_streams_bytes),
        ("Estructura", u.structure_bytes),
    ] {
        println!(
            "  {name:<22} {:>10}  {:>5.1} %",
            human(b),
            b as f64 * 100.0 / u.total_bytes.max(1) as f64
        );
    }
}

pub fn optimize(a: OptimizeArgs, json: bool) -> anyhow::Result<()> {
    if a.analyze {
        let u = pliego_optimize::analyze_with_password(&a.input, a.password.as_deref())
            .with_context(|| format!("no se pudo analizar {}", a.input.display()))?;
        if json {
            println!("{}", serde_json::to_string_pretty(&u)?);
        } else {
            print_usage("Tamaño", &u);
            println!("Imágenes: {} · objetos: {}", u.image_count, u.object_count);
            for i in &u.largest_images {
                println!(
                    "  xref {:>5}  {}×{}  {}  {}",
                    i.xref,
                    i.width,
                    i.height,
                    i.filter.as_deref().unwrap_or("sin filtro"),
                    human(i.bytes)
                );
            }
        }
        return Ok(());
    }
    let Some(output) = a.output.clone() else {
        bail!("falta el archivo de salida: usa -o salida.pdf (o --analyze para solo analizar)");
    };
    let opts = OptimizeOptions {
        preset: a.preset,
        target_dpi: a.dpi,
        jpeg_quality: a.quality,
        linearize: a.linearize,
        password: a.password,
        ..OptimizeOptions::default()
    };
    let ctx = JobContext::new(&NoProgress, Default::default());
    let (report, stats) = pliego_optimize::optimize_detailed(&a.input, &output, &opts, &ctx)
        .with_context(|| format!("no se pudo optimizar {}", a.input.display()))?;
    if json {
        println!("{}", serde_json::json!({ "report": report, "stats": stats }));
        return Ok(());
    }
    print_usage("Antes", &stats.before);
    print_usage("Después", &stats.after);
    println!(
        "Imágenes: {} · recomprimidas {} (reducidas {}) · sin tocar {}",
        stats.images.total, stats.images.recompressed, stats.images.downsampled, stats.images.skipped
    );
    for w in &report.warnings {
        eprintln!("Aviso: {w}");
    }
    println!("{}: {}", output.display(), report.summary);
    Ok(())
}
