use std::path::PathBuf;

use anyhow::{Context, bail};
use clap::{Args, ValueEnum};
use pliego_core::JobContext;
use pliego_redact::{Area, Fill, ImageMode, Pattern, RedactJob, RedactOptions};

#[derive(Clone, Copy, ValueEnum)]
pub enum FillArg {
    Black,
    None,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum ImagesArg {
    Pixels,
    Remove,
    Keep,
}

#[derive(Args)]
pub struct RedactArgs {
    /// PDF de entrada.
    pub input: PathBuf,
    /// Patrones, separados por comas: dni, nie, cif, iban, email, telefono, fecha o regex:<expresión>.
    #[arg(long, value_delimiter = ',')]
    pub pattern: Vec<Pattern>,
    /// Zona a redactar: PÁGINA:X0,Y0,X1,Y1 (página desde 1; puntos; origen arriba a la izquierda).
    #[arg(long, value_parser = parse_area)]
    pub area: Vec<Area>,
    /// Aplica la redacción. Sin esto solo se muestra la vista previa de coincidencias.
    #[arg(long)]
    pub apply: bool,
    /// PDF de salida (obligatorio con --apply; no se sobrescribe la entrada).
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    /// Relleno de las zonas redactadas.
    #[arg(long, value_enum, default_value = "black")]
    pub fill: FillArg,
    /// Qué hacer con las imágenes cubiertas.
    #[arg(long, value_enum, default_value = "pixels")]
    pub images: ImagesArg,
    /// Contraseña del documento, si la tiene.
    #[arg(long)]
    pub password: Option<String>,
}

fn parse_area(s: &str) -> Result<Area, String> {
    let (page, rest) = s.split_once(':').ok_or("formato esperado PÁGINA:X0,Y0,X1,Y1")?;
    let page: u32 = page.trim().parse().map_err(|_| "la página debe ser un número")?;
    if page == 0 {
        return Err("las páginas empiezan en 1".into());
    }
    let n: Vec<f32> = rest
        .split(',')
        .map(|v| v.trim().parse::<f32>().map_err(|_| format!("«{v}» no es un número")))
        .collect::<Result<_, _>>()?;
    let [x0, y0, x1, y1] = n[..] else {
        return Err("hacen falta 4 números: X0,Y0,X1,Y1".into());
    };
    Ok(Area {
        page: page - 1,
        x0,
        y0,
        x1,
        y1,
    })
}

pub fn run(a: RedactArgs, json: bool) -> anyhow::Result<()> {
    if a.pattern.is_empty() && a.area.is_empty() {
        bail!("indica qué redactar con --pattern y/o --area");
    }
    if !a.apply {
        return preview(&a, json);
    }
    let Some(output) = &a.output else {
        bail!("con --apply indica el archivo de salida con -o");
    };
    let job = RedactJob {
        areas: a.area.clone(),
        patterns: a.pattern.clone(),
        options: RedactOptions {
            fill: match a.fill {
                FillArg::Black => Fill::Black,
                FillArg::None => Fill::None,
            },
            images: match a.images {
                ImagesArg::Pixels => ImageMode::Pixels,
                ImagesArg::Remove => ImageMode::Remove,
                ImagesArg::Keep => ImageMode::Keep,
            },
            ..Default::default()
        },
    };
    let rep = pliego_redact::apply(&a.input, output, a.password.as_deref(), &job, &JobContext::silent())
        .with_context(|| format!("no se pudo redactar {}", a.input.display()))?;
    if json {
        println!("{}", serde_json::to_string_pretty(&rep)?);
    } else {
        println!("{}", rep.summary);
        for o in &rep.outputs {
            println!("-> {}", o.display());
        }
        for w in &rep.warnings {
            eprintln!("aviso: {w}");
        }
    }
    Ok(())
}

fn preview(a: &RedactArgs, json: bool) -> anyhow::Result<()> {
    let doc = {
        let mut d = mupdf::pdf::PdfDocument::open(a.input.to_str().context("la ruta debe ser UTF-8")?)
            .with_context(|| format!("no se pudo abrir {}", a.input.display()))?;
        if d.needs_password()? {
            match &a.password {
                Some(p) if d.authenticate(p)? => {}
                _ => bail!("el documento necesita contraseña o la contraseña no es válida"),
            }
        }
        d
    };
    let matches = pliego_redact::find_patterns(&doc, &a.pattern)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&matches)?);
    } else {
        for m in &matches {
            println!("página {:<3} {:<10} {}", m.page + 1, m.pattern, m.text);
        }
        eprintln!(
            "{} coincidencia(s). Añade --apply -o <salida.pdf> para redactar.",
            matches.len()
        );
    }
    Ok(())
}
