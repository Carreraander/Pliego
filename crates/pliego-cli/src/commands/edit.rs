use std::path::PathBuf;

use anyhow::{Context, bail};
use clap::{Args, Subcommand, ValueEnum};
use pliego_core::Report;
use pliego_document::{Document, SaveOptions};
use pliego_edit::{Align, FontFamily, LinkTarget, Style};

#[derive(Subcommand)]
pub enum EditCommand {
    /// Lista los bloques de texto editables de una página.
    Blocks(BlocksArgs),
    /// Reemplaza el texto de un bloque (ver `edit blocks`).
    Replace(ReplaceArgs),
    /// Añade texto nuevo en un recuadro.
    InsertText(InsertTextArgs),
    /// Crea un enlace externo o a otra página.
    Link(LinkArgs),
}

#[derive(Args)]
pub struct DocArgs {
    /// Archivo PDF de entrada.
    pub input: PathBuf,
    /// Contraseña del documento, si la tiene.
    #[arg(long)]
    pub password: Option<String>,
    /// Página (desde 1).
    #[arg(short, long, default_value_t = 1)]
    pub page: usize,
}

#[derive(Args)]
pub struct OutArgs {
    /// Archivo de salida. Nunca se sobrescribe la entrada salvo con `--in-place`.
    #[arg(short, long, conflicts_with = "in_place")]
    pub output: Option<PathBuf>,
    /// Guarda sobre el propio archivo de entrada.
    #[arg(long)]
    pub in_place: bool,
}

#[derive(Args)]
pub struct BlocksArgs {
    #[command(flatten)]
    pub doc: DocArgs,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum FamilyArg {
    Sans,
    Serif,
    Mono,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum AlignArg {
    Left,
    Center,
    Right,
}

#[derive(Args)]
pub struct StyleArgs {
    /// Familia de fuente estándar.
    #[arg(long)]
    pub family: Option<FamilyArg>,
    /// Tamaño en puntos.
    #[arg(long)]
    pub size: Option<f32>,
    /// Color como #RRGGBB.
    #[arg(long)]
    pub color: Option<String>,
    #[arg(long)]
    pub bold: Option<bool>,
    #[arg(long)]
    pub italic: Option<bool>,
    #[arg(long, default_value = "left")]
    pub align: AlignArg,
}

#[derive(Args)]
pub struct ReplaceArgs {
    #[command(flatten)]
    pub doc: DocArgs,
    #[command(flatten)]
    pub out: OutArgs,
    /// Identificador del bloque (el `id` que muestra `edit blocks`).
    #[arg(long)]
    pub block: usize,
    /// Texto nuevo (`\n` separa párrafos).
    #[arg(long)]
    pub text: String,
    #[command(flatten)]
    pub style: StyleArgs,
}

#[derive(Args)]
pub struct InsertTextArgs {
    #[command(flatten)]
    pub doc: DocArgs,
    #[command(flatten)]
    pub out: OutArgs,
    /// Recuadro `x0,y0,x1,y1` en puntos, origen arriba a la izquierda.
    #[arg(long)]
    pub rect: String,
    #[arg(long)]
    pub text: String,
    #[command(flatten)]
    pub style: StyleArgs,
}

#[derive(Args)]
pub struct LinkArgs {
    #[command(flatten)]
    pub doc: DocArgs,
    #[command(flatten)]
    pub out: OutArgs,
    /// Área del enlace `x0,y0,x1,y1` en puntos.
    #[arg(long)]
    pub rect: String,
    /// Dirección externa.
    #[arg(long, conflicts_with = "to_page", required_unless_present = "to_page")]
    pub uri: Option<String>,
    /// Página de destino (desde 1).
    #[arg(long)]
    pub to_page: Option<usize>,
}

pub fn run(cmd: EditCommand, json: bool) -> anyhow::Result<()> {
    match cmd {
        EditCommand::Blocks(a) => blocks(a, json),
        EditCommand::Replace(a) => replace(a, json),
        EditCommand::InsertText(a) => insert_text(a, json),
        EditCommand::Link(a) => link(a, json),
    }
}

fn open(a: &DocArgs) -> anyhow::Result<(Document, usize)> {
    let doc = Document::open(&a.input, a.password.as_deref())?;
    let page = a.page.checked_sub(1).context("las páginas empiezan en 1")?;
    Ok((doc, page))
}

fn parse_rect(s: &str) -> anyhow::Result<[f32; 4]> {
    let v: Vec<f32> = s
        .split(',')
        .map(|p| p.trim().parse::<f32>())
        .collect::<Result<_, _>>()
        .context("el recuadro debe ser x0,y0,x1,y1 con números")?;
    match v[..] {
        [a, b, c, d] => Ok([a, b, c, d]),
        _ => bail!("el recuadro debe tener cuatro números: x0,y0,x1,y1"),
    }
}

fn parse_color(s: &str) -> anyhow::Result<[f32; 3]> {
    let h = s.trim_start_matches('#');
    if h.len() != 6 {
        bail!("el color debe ser #RRGGBB");
    }
    let c = |i: usize| -> anyhow::Result<f32> {
        Ok(f32::from(u8::from_str_radix(&h[i..i + 2], 16).context("color no válido")?) / 255.0)
    };
    Ok([c(0)?, c(2)?, c(4)?])
}

fn style(a: &StyleArgs) -> anyhow::Result<Style> {
    Ok(Style {
        family: a.family.map(|f| match f {
            FamilyArg::Sans => FontFamily::Sans,
            FamilyArg::Serif => FontFamily::Serif,
            FamilyArg::Mono => FontFamily::Mono,
        }),
        size: a.size,
        color: a.color.as_deref().map(parse_color).transpose()?,
        bold: a.bold,
        italic: a.italic,
        align: match a.align {
            AlignArg::Left => Align::Left,
            AlignArg::Center => Align::Center,
            AlignArg::Right => Align::Right,
        },
    })
}

fn finish(
    mut doc: Document,
    input: &std::path::Path,
    out: &OutArgs,
    report: &Report,
    json: bool,
) -> anyhow::Result<()> {
    let dest = match (&out.output, out.in_place) {
        (Some(o), _) => o.clone(),
        (None, true) => input.to_path_buf(),
        (None, false) => bail!("indica -o <salida.pdf> (o --in-place para modificar el original)"),
    };
    doc.save(&dest, &SaveOptions::default())?;
    if json {
        println!(
            "{}",
            serde_json::json!({ "output": dest, "summary": report.summary, "warnings": report.warnings })
        );
    } else {
        println!("{}: {}", report.summary, dest.display());
        for w in &report.warnings {
            eprintln!("aviso: {w}");
        }
    }
    Ok(())
}

fn blocks(a: BlocksArgs, json: bool) -> anyhow::Result<()> {
    let (doc, page) = open(&a.doc)?;
    let blocks = pliego_edit::text_blocks(&doc, page)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&blocks)?);
    } else if blocks.is_empty() {
        println!("Sin bloques de texto editables.");
    } else {
        for b in &blocks {
            println!(
                "id={} [{:.0},{:.0},{:.0},{:.0}] {:.1} pt {}: {}",
                b.id,
                b.bbox[0],
                b.bbox[1],
                b.bbox[2],
                b.bbox[3],
                b.font_size,
                b.font_name.as_deref().unwrap_or("?"),
                b.text.replace('\n', " ⏎ ")
            );
        }
    }
    Ok(())
}

fn replace(a: ReplaceArgs, json: bool) -> anyhow::Result<()> {
    let (mut doc, page) = open(&a.doc)?;
    let report = pliego_edit::replace_text(&mut doc, page, a.block, &a.text, &style(&a.style)?)?;
    finish(doc, &a.doc.input, &a.out, &report, json)
}

fn insert_text(a: InsertTextArgs, json: bool) -> anyhow::Result<()> {
    let (mut doc, page) = open(&a.doc)?;
    let report = pliego_edit::insert_text(&mut doc, page, parse_rect(&a.rect)?, &a.text, &style(&a.style)?)?;
    finish(doc, &a.doc.input, &a.out, &report, json)
}

fn link(a: LinkArgs, json: bool) -> anyhow::Result<()> {
    let (mut doc, page) = open(&a.doc)?;
    let target = match (a.uri, a.to_page) {
        (Some(uri), _) => LinkTarget::Uri { uri },
        (None, Some(p)) => LinkTarget::Page {
            page: p.checked_sub(1).context("las páginas empiezan en 1")?,
        },
        (None, None) => bail!("indica --uri o --to-page"),
    };
    let report = pliego_edit::add_link(&mut doc, page, parse_rect(&a.rect)?, &target)?;
    finish(doc, &a.doc.input, &a.out, &report, json)
}
