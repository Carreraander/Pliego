//! Oráculo de redacción por línea de órdenes.
//!
//! ```text
//! redaction-oracle redactado.pdf --needle 12345678Z --needle "ES91 2100 0418" [--region 0,50,100,300,130]
//! ```
//! Sale con código 1 y lista los hallazgos si queda algún rastro (ver `pliego_qa::oracle`).

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use pliego_qa::engines;
use pliego_qa::oracle::{OracleConfig, Region, check_redacted};

#[derive(Parser)]
#[command(
    name = "redaction-oracle",
    about = "Comprueba que un PDF redactado no conserva los datos"
)]
struct Args {
    file: PathBuf,
    /// Dato que no debe aparecer (repetible).
    #[arg(long = "needle")]
    needles: Vec<String>,
    /// Zona que debe quedar de un solo color: `pagina_desde_0,x0,y0,x1,y1` en puntos (repetible).
    #[arg(long = "region", value_parser = parse_region)]
    regions: Vec<Region>,
    #[arg(long, default_value = "mupdf")]
    engine: String,
    /// Contraseña del PDF (solo para el motor; ver `OracleConfig::password`).
    #[arg(long)]
    password: Option<String>,
}

fn parse_region(s: &str) -> Result<Region, String> {
    let p: Vec<&str> = s.split(',').collect();
    if p.len() != 5 {
        return Err("formato: pagina,x0,y0,x1,y1".into());
    }
    let n = |i: usize| p[i].trim().parse::<f32>().map_err(|e| e.to_string());
    Ok(Region {
        page: n(0)? as u32,
        x0: n(1)?,
        y0: n(2)?,
        x1: n(3)?,
        y1: n(4)?,
    })
}

fn main() -> ExitCode {
    let a = Args::parse();
    let run = || -> anyhow::Result<Vec<_>> {
        let engine = engines::by_name(&a.engine)?;
        Ok(check_redacted(
            engine.as_ref(),
            &a.file,
            &OracleConfig {
                needles: a.needles.clone(),
                regions: a.regions.clone(),
                password: a.password.clone(),
            },
        )?)
    };
    match run() {
        Ok(f) if f.is_empty() => {
            println!("oráculo: sin rastro de los datos en {}", a.file.display());
            ExitCode::SUCCESS
        }
        Ok(f) => {
            for x in &f {
                println!("[{}] {}", x.check, x.detail);
            }
            println!("oráculo: {} hallazgo(s)", f.len());
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::from(2)
        }
    }
}
