//! `pliego merge`, `pliego split` y `pliego pages ...`.

use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use clap::{Args, Subcommand};
use pliego_core::{JobContext, PageRange, Report};
use pliego_organize::{
    CropSpec, LabelStyle, MergeOptions, PageLabelSpec, PdfDocument, SplitRule, crop, delete, duplicate, extract,
    insert_blank, insert_from, move_page, page_count, reorder, rotate, set_page_labels,
};

#[derive(Args)]
pub struct MergeArgs {
    /// PDFs a combinar, en orden.
    #[arg(required = true)]
    pub inputs: Vec<PathBuf>,
    /// Archivo de salida.
    #[arg(short, long)]
    pub output: PathBuf,
    /// No conservar los marcadores de los archivos originales.
    #[arg(long)]
    pub no_bookmarks: bool,
    /// Añadir un marcador por archivo con su nombre.
    #[arg(long)]
    pub bookmark_per_file: bool,
    /// Contraseña común de los archivos protegidos.
    #[arg(long)]
    pub password: Option<String>,
}

#[derive(Args)]
pub struct SplitArgs {
    pub input: PathBuf,
    /// Carpeta de salida.
    #[arg(short, long, default_value = ".")]
    pub output: PathBuf,
    /// Un archivo cada N páginas.
    #[arg(long, group = "rule")]
    pub every: Option<u32>,
    /// Tramos "1-3,4-6" (páginas desde 1).
    #[arg(long, group = "rule")]
    pub ranges: Option<String>,
    /// Un archivo por marcador de primer nivel.
    #[arg(long, group = "rule")]
    pub bookmarks: bool,
    /// Tamaño máximo por archivo, por ejemplo 500k o 5MB.
    #[arg(long, group = "rule")]
    pub max_size: Option<String>,
    #[arg(long)]
    pub password: Option<String>,
}

#[derive(Args)]
pub struct PagesArgs {
    #[command(subcommand)]
    pub op: PagesOp,
}

/// Archivo de entrada y destino de las operaciones que modifican un documento.
#[derive(Args)]
pub struct Target {
    pub input: PathBuf,
    /// Archivo de salida.
    #[arg(short, long, conflicts_with = "in_place")]
    pub output: Option<PathBuf>,
    /// Sobrescribir el archivo de entrada (por defecto nunca se sobrescribe).
    #[arg(long)]
    pub in_place: bool,
    #[arg(long)]
    pub password: Option<String>,
}

impl Target {
    fn out(&self) -> anyhow::Result<&Path> {
        match (&self.output, self.in_place) {
            (Some(o), false) => Ok(o),
            (None, true) => Ok(&self.input),
            _ => bail!("indica -o <salida> o --in-place"),
        }
    }
    fn open(&self) -> anyhow::Result<PdfDocument> {
        pliego_organize::open(&self.input, self.password.as_deref())
            .with_context(|| format!("no se pudo abrir {}", self.input.display()))
    }
}

#[derive(Subcommand)]
pub enum PagesOp {
    /// Crea un PDF con las páginas indicadas.
    Extract {
        #[command(flatten)]
        t: Target,
        #[arg(short, long)]
        pages: PageRange,
    },
    /// Inserta páginas en blanco o de otro PDF antes de la posición dada.
    Insert {
        #[command(flatten)]
        t: Target,
        /// Posición (desde 1): las páginas nuevas quedan antes de ella. "end" para añadir al final.
        #[arg(long, default_value = "end")]
        at: String,
        /// PDF del que tomar las páginas.
        #[arg(long = "from", conflicts_with = "blank")]
        from: Option<PathBuf>,
        /// Páginas del PDF de origen (todas por defecto).
        #[arg(long)]
        source_pages: Option<PageRange>,
        /// Número de páginas en blanco a insertar.
        #[arg(long)]
        blank: Option<u32>,
        /// Tamaño de las páginas en blanco en puntos, "595x842" (por defecto, el de la página vecina).
        #[arg(long)]
        size: Option<String>,
    },
    /// Borra páginas.
    Delete {
        #[command(flatten)]
        t: Target,
        #[arg(short, long)]
        pages: PageRange,
    },
    /// Gira páginas.
    Rotate {
        #[command(flatten)]
        t: Target,
        #[arg(short, long, default_value = "all")]
        pages: PageRange,
        /// Grados en sentido horario (múltiplo de 90; negativo = antihorario).
        #[arg(short, long, allow_hyphen_values = true)]
        degrees: i32,
    },
    /// Mueve una página a otra posición.
    Move {
        #[command(flatten)]
        t: Target,
        #[arg(long)]
        from: u32,
        #[arg(long)]
        to: u32,
    },
    /// Reordena todas las páginas: "3,1,2" deja la 3 primera, luego la 1 y luego la 2.
    Reorder {
        #[command(flatten)]
        t: Target,
        #[arg(long)]
        order: PageRange,
    },
    /// Duplica páginas junto a sí mismas.
    Duplicate {
        #[command(flatten)]
        t: Target,
        #[arg(short, long)]
        pages: PageRange,
    },
    /// Recorta páginas (CropBox), en puntos.
    Crop {
        #[command(flatten)]
        t: Target,
        #[arg(short, long, default_value = "all")]
        pages: PageRange,
        /// Rectángulo "x0,y0,x1,y1" (origen abajo a la izquierda).
        #[arg(long, group = "area", allow_hyphen_values = true)]
        rect: Option<String>,
        /// Márgenes a quitar "izquierda,arriba,derecha,abajo".
        #[arg(long, group = "area", allow_hyphen_values = true)]
        margins: Option<String>,
    },
    /// Define etiquetas de página, p. ej. --rule 1:lower-roman --rule 5:decimal:A-:1
    Labels {
        #[command(flatten)]
        t: Target,
        /// "PÁGINA:ESTILO[:PREFIJO[:PRIMER_NÚMERO]]"; estilos: none, decimal, upper-roman, lower-roman, upper-alpha, lower-alpha.
        #[arg(long = "rule", required = true)]
        rules: Vec<String>,
    },
}

fn finish(report: Report, json: bool) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("{}", report.summary);
        for o in &report.outputs {
            println!("  {}", o.display());
        }
        for w in &report.warnings {
            eprintln!("aviso: {w}");
        }
    }
    Ok(())
}

pub fn merge(a: MergeArgs, json: bool) -> anyhow::Result<()> {
    let opts = MergeOptions {
        keep_bookmarks: !a.no_bookmarks,
        bookmark_per_file: a.bookmark_per_file,
        password: a.password,
    };
    finish(
        pliego_organize::merge(&a.inputs, &a.output, &opts, &JobContext::silent())?,
        json,
    )
}

fn parse_size(s: &str) -> anyhow::Result<u64> {
    let s = s.trim().to_lowercase();
    let (num, mult) = if let Some(n) = s.strip_suffix("mb").or_else(|| s.strip_suffix('m')) {
        (n, 1_000_000.0)
    } else if let Some(n) = s.strip_suffix("kb").or_else(|| s.strip_suffix('k')) {
        (n, 1_000.0)
    } else {
        (s.trim_end_matches('b'), 1.0)
    };
    let v: f64 = num.trim().parse().with_context(|| format!("tamaño no válido: {s}"))?;
    if v <= 0.0 {
        bail!("el tamaño debe ser mayor que 0");
    }
    Ok((v * mult) as u64)
}

pub fn split(a: SplitArgs, json: bool) -> anyhow::Result<()> {
    let doc = pliego_organize::open(&a.input, a.password.as_deref())
        .with_context(|| format!("no se pudo abrir {}", a.input.display()))?;
    let rule = match (a.every, &a.ranges, a.bookmarks, &a.max_size) {
        (Some(n), None, false, None) => SplitRule::EveryN { n },
        (None, Some(r), false, None) => {
            let mut ranges = Vec::new();
            for part in r.split(',') {
                let (x, y) = part.split_once('-').unwrap_or((part, part));
                let x: u32 = x.trim().parse().with_context(|| format!("tramo no válido: {part}"))?;
                let y: u32 = y.trim().parse().with_context(|| format!("tramo no válido: {part}"))?;
                if x == 0 || y == 0 {
                    bail!("las páginas se numeran desde 1");
                }
                ranges.push((x - 1, y - 1));
            }
            SplitRule::Ranges { ranges }
        }
        (None, None, true, None) => SplitRule::Bookmarks,
        (None, None, false, Some(s)) => SplitRule::MaxBytes { bytes: parse_size(s)? },
        _ => bail!("indica exactamente una de --every, --ranges, --bookmarks o --max-size"),
    };
    let stem = a
        .input
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "parte".into());
    finish(
        pliego_organize::split(&doc, &stem, &rule, &a.output, &JobContext::silent())?,
        json,
    )
}

fn zero_based(r: &PageRange, doc: &PdfDocument) -> anyhow::Result<Vec<u32>> {
    Ok(r.resolve(page_count(doc)?)?)
}

fn floats<const N: usize>(s: &str) -> anyhow::Result<[f32; N]> {
    let v: Vec<f32> = s
        .split(',')
        .map(|x| {
            x.trim()
                .parse::<f32>()
                .with_context(|| format!("número no válido: {x}"))
        })
        .collect::<Result<_, _>>()?;
    v.try_into()
        .map_err(|_| anyhow::anyhow!("se esperaban {N} valores separados por comas"))
}

fn style(s: &str) -> anyhow::Result<LabelStyle> {
    Ok(match s {
        "none" => LabelStyle::None,
        "decimal" => LabelStyle::Decimal,
        "upper-roman" => LabelStyle::UpperRoman,
        "lower-roman" => LabelStyle::LowerRoman,
        "upper-alpha" => LabelStyle::UpperAlpha,
        "lower-alpha" => LabelStyle::LowerAlpha,
        other => bail!("estilo de etiqueta desconocido: {other}"),
    })
}

fn parse_rule(r: &str) -> anyhow::Result<PageLabelSpec> {
    let mut it = r.splitn(4, ':');
    let page: u32 = it
        .next()
        .unwrap_or("")
        .parse()
        .with_context(|| format!("regla no válida: {r}"))?;
    if page == 0 {
        bail!("las páginas se numeran desde 1");
    }
    let st = style(
        it.next()
            .ok_or_else(|| anyhow::anyhow!("falta el estilo en la regla {r}"))?,
    )?;
    let prefix = it.next().unwrap_or("").to_string();
    let first_number = match it.next() {
        Some(n) => n.parse().with_context(|| format!("número inicial no válido: {n}"))?,
        None => 1,
    };
    Ok(PageLabelSpec {
        start_page: page - 1,
        style: st,
        prefix,
        first_number,
    })
}

pub fn pages(a: PagesArgs, json: bool) -> anyhow::Result<()> {
    use PagesOp::*;
    // Cada operación devuelve (destino, documento resultante, resumen).
    let (out, doc, summary): (PathBuf, PdfDocument, String) = match a.op {
        Extract { t, pages } => {
            let src = t.open()?;
            let sel = zero_based(&pages, &src)?;
            let out = extract(&src, &sel)?;
            let n = sel.len();
            (t.out()?.to_path_buf(), out, format!("{n} páginas extraídas"))
        }
        Insert {
            t,
            at,
            from,
            source_pages,
            blank,
            size,
        } => {
            let mut doc = t.open()?;
            let total = page_count(&doc)?;
            let at = if at == "end" {
                total
            } else {
                at.parse::<u32>()
                    .ok()
                    .filter(|&n| (1..=total + 1).contains(&n))
                    .map(|n| n - 1)
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "posición no válida: {at} (usa un número de 1 a {} o \"end\")",
                            total + 1
                        )
                    })?
            };
            let summary = match (from, blank) {
                (Some(path), None) => {
                    let src = pliego_organize::open(&path, None)
                        .with_context(|| format!("no se pudo abrir {}", path.display()))?;
                    let sel = zero_based(&source_pages.unwrap_or_else(PageRange::all), &src)?;
                    insert_from(&mut doc, &src, &sel, at)?;
                    format!("{} páginas insertadas", sel.len())
                }
                (None, Some(n)) => {
                    let size = size
                        .map(|s| {
                            let (w, h) = s
                                .split_once('x')
                                .ok_or_else(|| anyhow::anyhow!("tamaño no válido: {s}"))?;
                            anyhow::Ok((w.trim().parse::<f32>()?, h.trim().parse::<f32>()?))
                        })
                        .transpose()?;
                    insert_blank(&mut doc, at, n, size)?;
                    format!("{n} páginas en blanco insertadas")
                }
                _ => bail!("indica --from <pdf> o --blank <n>"),
            };
            (t.out()?.to_path_buf(), doc, summary)
        }
        Delete { t, pages } => {
            let mut doc = t.open()?;
            let sel = zero_based(&pages, &doc)?;
            delete(&mut doc, &sel)?;
            (t.out()?.to_path_buf(), doc, format!("{} páginas borradas", sel.len()))
        }
        Rotate { t, pages, degrees } => {
            let mut doc = t.open()?;
            let sel = zero_based(&pages, &doc)?;
            rotate(&mut doc, &sel, degrees)?;
            (
                t.out()?.to_path_buf(),
                doc,
                format!("{} páginas giradas {degrees}°", sel.len()),
            )
        }
        Move { t, from, to } => {
            let mut doc = t.open()?;
            if from == 0 || to == 0 {
                bail!("las páginas se numeran desde 1");
            }
            move_page(&mut doc, from - 1, to - 1)?;
            (
                t.out()?.to_path_buf(),
                doc,
                format!("página {from} movida a la posición {to}"),
            )
        }
        Reorder { t, order } => {
            let mut doc = t.open()?;
            let sel = zero_based(&order, &doc)?;
            reorder(&mut doc, &sel)?;
            (t.out()?.to_path_buf(), doc, "páginas reordenadas".into())
        }
        Duplicate { t, pages } => {
            let mut doc = t.open()?;
            let sel = zero_based(&pages, &doc)?;
            duplicate(&mut doc, &sel)?;
            (t.out()?.to_path_buf(), doc, format!("{} páginas duplicadas", sel.len()))
        }
        Crop {
            t,
            pages,
            rect,
            margins,
        } => {
            let mut doc = t.open()?;
            let sel = zero_based(&pages, &doc)?;
            let spec = match (rect, margins) {
                (Some(r), None) => {
                    let [x0, y0, x1, y1] = floats::<4>(&r)?;
                    CropSpec::Rect { x0, y0, x1, y1 }
                }
                (None, Some(m)) => {
                    let [left, top, right, bottom] = floats::<4>(&m)?;
                    CropSpec::Margins {
                        left,
                        top,
                        right,
                        bottom,
                    }
                }
                _ => bail!("indica --rect o --margins"),
            };
            crop(&mut doc, &sel, spec)?;
            (t.out()?.to_path_buf(), doc, format!("{} páginas recortadas", sel.len()))
        }
        Labels { t, rules } => {
            let mut doc = t.open()?;
            let specs = rules.iter().map(|r| parse_rule(r)).collect::<Result<Vec<_>, _>>()?;
            set_page_labels(&mut doc, &specs)?;
            (
                t.out()?.to_path_buf(),
                doc,
                format!("{} reglas de etiquetas aplicadas", specs.len()),
            )
        }
    };
    pliego_organize::save(&doc, &out)?;
    finish(Report::with_output(out, summary), json)
}
