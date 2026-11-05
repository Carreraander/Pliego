use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use clap::{Args, Subcommand, ValueEnum};
use pliego_core::{CancelToken, JobContext, Progress};
use pliego_forms::{FieldValue, FieldValues, Format};
use pliego_pdf::PdfDocument;

#[derive(Subcommand)]
pub enum FormCommand {
    /// Lista los campos del formulario.
    Fields(DocArgs),
    /// Rellena campos: `--set nombre=Ana` (repetible) y/o `--data datos.json`.
    Fill(FillArgs),
    /// Restablece los campos (todos o los indicados con `--field`).
    Reset(ResetArgs),
    /// Aplana el formulario: los campos pasan a ser contenido fijo.
    Flatten(FlattenArgs),
    /// Exporta los valores de los campos a JSON o CSV.
    Export(ExportArgs),
    /// Importa valores desde JSON o CSV.
    Import(ImportArgs),
    /// Genera un PDF por cada fila de un CSV a partir de una plantilla.
    Bulk(BulkArgs),
}

#[derive(Clone, Copy, ValueEnum)]
pub enum DataFormat {
    Json,
    Csv,
    Fdf,
    Xfdf,
}

impl From<DataFormat> for Format {
    fn from(f: DataFormat) -> Self {
        match f {
            DataFormat::Json => Format::Json,
            DataFormat::Csv => Format::Csv,
            DataFormat::Fdf => Format::Fdf,
            DataFormat::Xfdf => Format::Xfdf,
        }
    }
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
pub struct FillArgs {
    #[command(flatten)]
    pub doc: DocArgs,
    #[command(flatten)]
    pub out: OutArgs,
    /// Valor de un campo, `nombre=valor` (las casillas aceptan true/false).
    #[arg(long = "set", value_name = "CAMPO=VALOR")]
    pub set: Vec<String>,
    /// JSON con los valores (`{"campo": "valor", "casilla": true}`).
    #[arg(long)]
    pub data: Option<PathBuf>,
    /// Aplanar el formulario después de rellenarlo.
    #[arg(long)]
    pub flatten: bool,
}

#[derive(Args)]
pub struct ResetArgs {
    #[command(flatten)]
    pub doc: DocArgs,
    #[command(flatten)]
    pub out: OutArgs,
    /// Campo a restablecer (repetible); sin él, todos.
    #[arg(long = "field")]
    pub field: Vec<String>,
}

#[derive(Args)]
pub struct FlattenArgs {
    #[command(flatten)]
    pub doc: DocArgs,
    #[command(flatten)]
    pub out: OutArgs,
}

#[derive(Args)]
pub struct ExportArgs {
    #[command(flatten)]
    pub doc: DocArgs,
    #[arg(short, long, value_enum, default_value = "json")]
    pub format: DataFormat,
    /// Archivo de salida; sin él, la salida estándar.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

#[derive(Args)]
pub struct ImportArgs {
    #[command(flatten)]
    pub doc: DocArgs,
    #[command(flatten)]
    pub out: OutArgs,
    /// Archivo de datos.
    pub data: PathBuf,
    /// Formato; por defecto, según la extensión.
    #[arg(short, long, value_enum)]
    pub format: Option<DataFormat>,
}

#[derive(Args)]
pub struct BulkArgs {
    /// PDF plantilla con campos.
    pub template: PathBuf,
    /// CSV: cabecera con nombres de campo y una fila por PDF (columna opcional `_filename`).
    pub csv: PathBuf,
    /// Carpeta de salida.
    #[arg(short, long)]
    pub output: PathBuf,
    /// Aplanar cada PDF generado.
    #[arg(long)]
    pub flatten: bool,
}

pub fn run(cmd: FormCommand, json: bool) -> anyhow::Result<()> {
    match cmd {
        FormCommand::Fields(a) => fields(a, json),
        FormCommand::Fill(a) => fill(a, json),
        FormCommand::Reset(a) => reset(a, json),
        FormCommand::Flatten(a) => flatten(a, json),
        FormCommand::Export(a) => export(a),
        FormCommand::Import(a) => import(a, json),
        FormCommand::Bulk(a) => bulk(a, json),
    }
}

fn open(a: &DocArgs) -> anyhow::Result<PdfDocument> {
    let mut doc = PdfDocument::open(&a.input).with_context(|| format!("no se pudo abrir {}", a.input.display()))?;
    if doc.needs_password()? && !doc.authenticate(a.password.as_deref().unwrap_or(""))? {
        bail!("el documento necesita contraseña o la contraseña no es válida");
    }
    Ok(doc)
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
    let tmp = dir.join(format!(".pliego-{}.tmp", std::process::id()));
    let mut f = std::fs::File::create(&tmp).with_context(|| format!("no se pudo escribir {}", tmp.display()))?;
    doc.write_to(&mut f)?;
    drop(f);
    std::fs::rename(&tmp, &dest).with_context(|| format!("no se pudo guardar {}", dest.display()))?;
    Ok(dest)
}

fn report_fill(res: &pliego_forms::FillResult, dest: &Path, json: bool) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::json!({ "result": res, "output": dest }));
    } else {
        println!("{} campo(s) rellenado(s): {}", res.filled.len(), dest.display());
        for n in &res.not_found {
            eprintln!("Aviso: el campo «{n}» no existe.");
        }
        for n in &res.rejected {
            eprintln!("Aviso: el campo «{n}» no admitió el valor (solo lectura u opción no válida).");
        }
    }
    Ok(())
}

fn fields(a: DocArgs, json: bool) -> anyhow::Result<()> {
    let mut doc = open(&a)?;
    let fs = pliego_forms::fields(&mut doc)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&fs)?);
    } else if fs.is_empty() {
        println!("El documento no tiene campos de formulario.");
    } else {
        for f in &fs {
            let kind = serde_json::to_value(f.kind)?;
            println!(
                "p.{} {:<12} {} = {}",
                f.page + 1,
                kind.as_str().unwrap_or("?"),
                f.name,
                f.value.as_deref().unwrap_or("")
            );
        }
    }
    Ok(())
}

fn fill(a: FillArgs, json: bool) -> anyhow::Result<()> {
    let mut doc = open(&a.doc)?;
    let mut values = FieldValues::new();
    if let Some(p) = &a.data {
        let t = std::fs::read_to_string(p).with_context(|| format!("no se pudo leer {}", p.display()))?;
        values = serde_json::from_str(&t).context("el JSON de datos no es válido")?;
    }
    for s in &a.set {
        let (k, v) = s
            .split_once('=')
            .with_context(|| format!("«{s}» debe ser CAMPO=VALOR"))?;
        let val = match v {
            "true" => FieldValue::Bool(true),
            "false" => FieldValue::Bool(false),
            _ => FieldValue::Text(v.to_owned()),
        };
        values.insert(k.to_owned(), val);
    }
    if values.is_empty() {
        bail!("indica valores con --set CAMPO=VALOR o --data datos.json");
    }
    let res = pliego_forms::fill(&mut doc, &values)?;
    if a.flatten {
        pliego_forms::flatten(&mut doc)?;
    }
    let dest = save(&doc, &a.doc.input, &a.out)?;
    report_fill(&res, &dest, json)
}

fn reset(a: ResetArgs, json: bool) -> anyhow::Result<()> {
    let mut doc = open(&a.doc)?;
    let names = (!a.field.is_empty()).then_some(a.field.as_slice());
    let n = pliego_forms::reset(&mut doc, names)?;
    let dest = save(&doc, &a.doc.input, &a.out)?;
    if json {
        println!("{}", serde_json::json!({ "reset": n, "output": dest }));
    } else {
        println!("{n} campo(s) restablecido(s): {}", dest.display());
    }
    Ok(())
}

fn flatten(a: FlattenArgs, json: bool) -> anyhow::Result<()> {
    let mut doc = open(&a.doc)?;
    let n = pliego_forms::flatten(&mut doc)?;
    let dest = save(&doc, &a.doc.input, &a.out)?;
    if json {
        println!("{}", serde_json::json!({ "flattened": n, "output": dest }));
    } else {
        println!("{n} campo(s) aplanado(s): {}", dest.display());
    }
    Ok(())
}

fn export(a: ExportArgs) -> anyhow::Result<()> {
    let mut doc = open(&a.doc)?;
    let text = pliego_forms::export_data(&mut doc, a.format.into())?;
    match a.output {
        Some(p) => std::fs::write(&p, text).with_context(|| format!("no se pudo escribir {}", p.display()))?,
        None => print!("{text}"),
    }
    Ok(())
}

fn import(a: ImportArgs, json: bool) -> anyhow::Result<()> {
    let mut doc = open(&a.doc)?;
    let format = match a.format {
        Some(f) => f.into(),
        None => match a
            .data
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_lowercase)
            .as_deref()
        {
            Some("csv") => Format::Csv,
            Some("json") => Format::Json,
            Some("fdf") => Format::Fdf,
            Some("xfdf") => Format::Xfdf,
            _ => bail!("no se reconoce el formato de datos; indica --format json|csv|fdf|xfdf"),
        },
    };
    let bytes = std::fs::read(&a.data).with_context(|| format!("no se pudo leer {}", a.data.display()))?;
    let res = pliego_forms::import_data_bytes(&mut doc, &bytes, format)?;
    let dest = save(&doc, &a.doc.input, &a.out)?;
    report_fill(&res, &dest, json)
}

struct Bar;

impl Progress for Bar {
    fn report(&self, done: u64, total: u64, message: &str) {
        eprintln!("[{done}/{total}] {message}");
    }
}

fn bulk(a: BulkArgs, json: bool) -> anyhow::Result<()> {
    let ctx = JobContext::new(&Bar, CancelToken::new());
    let rep = pliego_forms::bulk_fill(&a.template, &a.csv, &a.output, a.flatten, &ctx)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&rep)?);
    } else {
        for w in &rep.warnings {
            eprintln!("Aviso: {w}");
        }
        println!("{}", rep.summary);
    }
    Ok(())
}
