use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use clap::{Args, Subcommand};
use mupdf::pdf::PdfDocument;
use pliego_annot::{AnnotId, AnnotKind, AnnotSpec};

#[derive(Subcommand)]
pub enum AnnotCommand {
    /// Lista las anotaciones.
    List(ListArgs),
    /// Añade una anotación descrita en un archivo JSON (o `-` para la entrada estándar).
    Add(AddArgs),
    /// Borra una anotación (y sus respuestas).
    Delete(DeleteArgs),
}

#[derive(Args)]
pub struct DocArgs {
    /// Archivo PDF de entrada.
    pub input: PathBuf,
    /// Contraseña del documento, si la tiene.
    #[arg(long)]
    pub password: Option<String>,
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
pub struct ListArgs {
    #[command(flatten)]
    pub doc: DocArgs,
    /// Solo esta página (desde 1).
    #[arg(short, long)]
    pub page: Option<usize>,
}

#[derive(Args)]
pub struct AddArgs {
    #[command(flatten)]
    pub doc: DocArgs,
    #[command(flatten)]
    pub out: OutArgs,
    /// Archivo JSON con la anotación (`AnnotSpec`) o `-` para leerla de la entrada estándar.
    #[arg(long)]
    pub spec: String,
    /// Página (desde 1); sustituye a la del JSON.
    #[arg(short, long)]
    pub page: Option<usize>,
}

#[derive(Args)]
pub struct DeleteArgs {
    #[command(flatten)]
    pub doc: DocArgs,
    #[command(flatten)]
    pub out: OutArgs,
    /// Página de la anotación (desde 1).
    #[arg(short, long)]
    pub page: usize,
    /// Identificador (número de objeto) que muestra `annot list`.
    #[arg(long)]
    pub id: i32,
}

pub fn run(cmd: AnnotCommand, json: bool) -> anyhow::Result<()> {
    match cmd {
        AnnotCommand::List(a) => list(a, json),
        AnnotCommand::Add(a) => add(a, json),
        AnnotCommand::Delete(a) => delete(a, json),
    }
}

fn open(a: &DocArgs) -> anyhow::Result<PdfDocument> {
    let mut doc =
        PdfDocument::open(a.input.as_path()).with_context(|| format!("no se pudo abrir {}", a.input.display()))?;
    if doc.needs_password()? && !doc.authenticate(a.password.as_deref().unwrap_or(""))? {
        bail!("el documento necesita contraseña o la contraseña no es válida");
    }
    Ok(doc)
}

fn page0(p: usize) -> anyhow::Result<usize> {
    p.checked_sub(1).context("las páginas empiezan en 1")
}

/// Escribe a un temporal junto al destino y lo renombra, para no dejar un archivo a medias.
fn save(doc: &PdfDocument, input: &Path, out: &OutArgs) -> anyhow::Result<PathBuf> {
    let dest = match (&out.output, out.in_place) {
        (Some(o), _) => o.clone(),
        (None, true) => input.to_path_buf(),
        (None, false) => bail!("indica -o <salida.pdf> (o --in-place para modificar el original)"),
    };
    let dir = dest
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let tmp = tempfile_in(dir)?;
    let mut f = std::fs::File::create(&tmp).with_context(|| format!("no se pudo escribir {}", tmp.display()))?;
    doc.write_to(&mut f)?;
    drop(f);
    std::fs::rename(&tmp, &dest).with_context(|| format!("no se pudo guardar {}", dest.display()))?;
    Ok(dest)
}

fn tempfile_in(dir: &Path) -> anyhow::Result<PathBuf> {
    Ok(dir.join(format!(".pliego-{}.tmp", std::process::id())))
}

fn kind_name(k: AnnotKind) -> String {
    serde_json::to_value(k)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

fn list(a: ListArgs, json: bool) -> anyhow::Result<()> {
    let mut doc = open(&a.doc)?;
    let page = a.page.map(page0).transpose()?;
    let items = pliego_annot::list(&mut doc, page)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&items)?);
    } else if items.is_empty() {
        println!("Sin anotaciones.");
    } else {
        for i in &items {
            println!(
                "p.{} id={} {:<10} {} {}",
                i.id.page + 1,
                i.id.xref,
                kind_name(i.kind),
                i.author.as_deref().unwrap_or("-"),
                i.contents.as_deref().unwrap_or("").replace('\n', " ")
            );
        }
    }
    Ok(())
}

fn add(a: AddArgs, json: bool) -> anyhow::Result<()> {
    let mut doc = open(&a.doc)?;
    let text = if a.spec == "-" {
        std::io::read_to_string(std::io::stdin())?
    } else {
        std::fs::read_to_string(&a.spec).with_context(|| format!("no se pudo leer {}", a.spec))?
    };
    let mut spec: AnnotSpec = serde_json::from_str(&text).context("el JSON de la anotación no es válido")?;
    if let Some(p) = a.page {
        spec.page = page0(p)?;
    }
    let id = pliego_annot::add(&mut doc, &spec)?;
    let dest = save(&doc, &a.doc.input, &a.out)?;
    if json {
        println!("{}", serde_json::json!({ "id": id, "output": dest }));
    } else {
        println!(
            "Anotación {} añadida en la página {}: {}",
            id.xref,
            id.page + 1,
            dest.display()
        );
    }
    Ok(())
}

fn delete(a: DeleteArgs, json: bool) -> anyhow::Result<()> {
    let mut doc = open(&a.doc)?;
    pliego_annot::delete(
        &mut doc,
        AnnotId {
            page: page0(a.page)?,
            xref: a.id,
        },
    )?;
    let dest = save(&doc, &a.doc.input, &a.out)?;
    if json {
        println!("{}", serde_json::json!({ "output": dest }));
    } else {
        println!("Anotación {} borrada: {}", a.id, dest.display());
    }
    Ok(())
}
