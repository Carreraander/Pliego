use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use clap::{Args, Subcommand};
use pliego_annot::{AnnotId, AnnotKind, AnnotSpec};
use pliego_pdf::PdfDocument;

#[derive(Subcommand)]
pub enum AnnotCommand {
    /// Lista las anotaciones.
    List(ListArgs),
    /// Añade una anotación descrita en un archivo JSON (o `-` para la entrada estándar).
    Add(AddArgs),
    /// Borra una anotación (y sus respuestas).
    Delete(DeleteArgs),
    /// Aplana las anotaciones en el contenido de las páginas.
    Flatten(FlattenArgs),
    /// Resumen de comentarios (texto o JSON con `--json`).
    Summary(ListArgs),
    /// Exporta los comentarios a XFDF.
    Export(ExportArgs),
    /// Importa comentarios desde XFDF (los ya existentes se omiten).
    Import(ImportArgs),
}

#[derive(Args)]
pub struct ExportArgs {
    #[command(flatten)]
    pub doc: DocArgs,
    /// Archivo XFDF de salida; sin él, la salida estándar.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

#[derive(Args)]
pub struct ImportArgs {
    #[command(flatten)]
    pub doc: DocArgs,
    #[command(flatten)]
    pub out: OutArgs,
    /// Archivo XFDF.
    pub xfdf: PathBuf,
}

#[derive(Args)]
pub struct FlattenArgs {
    #[command(flatten)]
    pub doc: DocArgs,
    #[command(flatten)]
    pub out: OutArgs,
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
        AnnotCommand::Flatten(a) => flatten(a, json),
        AnnotCommand::Summary(a) => summary(a, json),
        AnnotCommand::Export(a) => export(a),
        AnnotCommand::Import(a) => import(a, json),
    }
}

fn open(a: &DocArgs) -> anyhow::Result<PdfDocument> {
    // Se lee a memoria para no dejar el archivo abierto: en Windows `--in-place` no puede renombrar sobre él.
    let bytes = std::fs::read(&a.input).with_context(|| format!("no se pudo leer {}", a.input.display()))?;
    let mut doc = PdfDocument::from_bytes(&bytes).with_context(|| format!("no se pudo abrir {}", a.input.display()))?;
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

fn flatten(a: FlattenArgs, json: bool) -> anyhow::Result<()> {
    let mut doc = open(&a.doc)?;
    let n = pliego_annot::flatten(&mut doc)?;
    let dest = save(&doc, &a.doc.input, &a.out)?;
    if json {
        println!("{}", serde_json::json!({ "flattened": n, "output": dest }));
    } else {
        println!("{n} anotación(es) aplanada(s): {}", dest.display());
    }
    Ok(())
}

fn summary(a: ListArgs, json: bool) -> anyhow::Result<()> {
    let mut doc = open(&a.doc)?;
    let s = pliego_annot::summary(&mut doc)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&s)?);
    } else {
        print!("{}", s.to_text());
    }
    Ok(())
}

fn export(a: ExportArgs) -> anyhow::Result<()> {
    let mut doc = open(&a.doc)?;
    let xml = pliego_annot::export_xfdf(&mut doc)?;
    match a.output {
        Some(p) => std::fs::write(&p, xml).with_context(|| format!("no se pudo escribir {}", p.display()))?,
        None => print!("{xml}"),
    }
    Ok(())
}

fn import(a: ImportArgs, json: bool) -> anyhow::Result<()> {
    let mut doc = open(&a.doc)?;
    let xml = std::fs::read_to_string(&a.xfdf).with_context(|| format!("no se pudo leer {}", a.xfdf.display()))?;
    let n = pliego_annot::import_xfdf(&mut doc, &xml)?;
    let dest = save(&doc, &a.doc.input, &a.out)?;
    if json {
        println!("{}", serde_json::json!({ "imported": n, "output": dest }));
    } else {
        println!("{n} comentario(s) importado(s): {}", dest.display());
    }
    Ok(())
}
