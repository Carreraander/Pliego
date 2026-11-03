//! `pliego run accion.toml entrada... -o salida`: aplica una acción guardada a archivos y carpetas de PDFs.
//!
//! Aquí se registran los pasos disponibles: cada uno traduce las opciones serializables de su módulo.

use std::path::{Path, PathBuf};

use anyhow::Context;
use clap::Args;
use pliego_actions::{Action, Registry, RunOptions, StepRunner, collect_inputs};
use pliego_core::{Error, JobContext, PageRange, Progress, Report, Result};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::Value;

#[derive(Args)]
pub struct RunArgs {
    /// Acción guardada (.toml o .json).
    #[arg(required_unless_present = "list_steps")]
    pub action: Option<PathBuf>,
    /// PDFs o carpetas (se recorren las subcarpetas).
    pub inputs: Vec<PathBuf>,
    /// Carpeta de salida; se conserva la estructura de subcarpetas.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    /// Retomar un lote anterior en la misma carpeta de salida: se saltan los archivos ya procesados.
    #[arg(long)]
    pub resume: bool,
    /// Lista los pasos disponibles y sale.
    #[arg(long)]
    pub list_steps: bool,
}

fn parse<T: DeserializeOwned>(v: &Value) -> Result<T> {
    serde_json::from_value(v.clone()).map_err(|e| Error::InvalidInput(format!("opciones no válidas: {e}")))
}

/// Paso definido por una función con sus opciones ya tipadas.
struct Typed<T, F>(F, std::marker::PhantomData<fn() -> T>);

fn typed<T, F>(f: F) -> Typed<T, F>
where
    T: DeserializeOwned,
    F: Fn(&Path, &Path, T, &JobContext) -> Result<Report> + Send + Sync,
{
    Typed(f, std::marker::PhantomData)
}

impl<T, F> StepRunner for Typed<T, F>
where
    T: DeserializeOwned,
    F: Fn(&Path, &Path, T, &JobContext) -> Result<Report> + Send + Sync,
{
    fn validate(&self, options: &Value) -> Result<()> {
        parse::<T>(options).map(|_| ())
    }
    fn run(&self, input: &Path, output: &Path, options: &Value, ctx: &JobContext) -> Result<Report> {
        (self.0)(input, output, parse(options)?, ctx)
    }
}

/// Opciones de los pasos sobre páginas y sellos.
#[derive(Deserialize)]
struct OnPages<T> {
    #[serde(default)]
    pages: Option<String>,
    #[serde(default)]
    password: Option<String>,
    #[serde(flatten)]
    inner: T,
}

#[derive(Deserialize)]
struct Degrees {
    degrees: i32,
}

#[derive(Deserialize)]
struct Nothing {}

#[derive(Deserialize)]
struct Encrypt {
    #[serde(default)]
    input_password: Option<String>,
    #[serde(default)]
    cipher: Option<pliego_protect::Cipher>,
    #[serde(default)]
    user_password: String,
    #[serde(default)]
    owner_password: String,
    #[serde(default)]
    permissions: Option<pliego_protect::Permissions>,
}

impl Encrypt {
    fn encryption(self) -> (Option<String>, pliego_protect::Encryption) {
        let d = pliego_protect::Encryption::default();
        let enc = pliego_protect::Encryption {
            cipher: self.cipher.unwrap_or(d.cipher),
            user_password: self.user_password,
            owner_password: self.owner_password,
            permissions: self.permissions.unwrap_or(d.permissions),
        };
        (self.input_password, enc)
    }
}

#[derive(Deserialize)]
struct Sanitize {
    #[serde(default)]
    password: Option<String>,
    #[serde(default)]
    options: Option<pliego_protect::SanitizeOptions>,
}

fn page_range(pages: &Option<String>) -> Result<PageRange> {
    pages
        .as_deref()
        .unwrap_or("all")
        .parse()
        .map_err(|e: Error| Error::InvalidInput(format!("rango de páginas: {e}")))
}

/// Abre, aplica `f` a las páginas elegidas y guarda.
fn edit_pages<T, F>(input: &Path, output: &Path, o: OnPages<T>, f: F) -> Result<Report>
where
    F: FnOnce(&mut pliego_organize::PdfDocument, &[u32], T) -> Result<()>,
{
    let mut doc = pliego_organize::open(input, o.password.as_deref())?;
    let sel = page_range(&o.pages)?.resolve(pliego_organize::page_count(&doc)?)?;
    f(&mut doc, &sel, o.inner)?;
    pliego_organize::save(&doc, output)?;
    Ok(Report::default())
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Registro con todos los pasos que ofrece la CLI.
pub fn builtin_registry() -> Registry {
    let mut r = Registry::new();
    r.register(
        "optimize",
        typed(
            |i: &Path, o: &Path, opts: pliego_optimize::OptimizeOptions, ctx: &JobContext| {
                pliego_optimize::optimize(i, o, &opts, ctx)
            },
        ),
    );
    r.register(
        "ocr",
        typed(|i: &Path, o: &Path, opts: pliego_ocr::OcrOptions, ctx: &JobContext| {
            pliego_ocr::ocr_document(i, o, &opts, ctx)
        }),
    );
    r.register(
        "protect.encrypt",
        typed(|i: &Path, o: &Path, e: Encrypt, _: &JobContext| {
            let (pw, enc) = e.encryption();
            pliego_protect::encrypt(i, o, pw.as_deref(), &enc)
        }),
    );
    r.register(
        "protect.sanitize",
        typed(|i: &Path, o: &Path, s: Sanitize, ctx: &JobContext| {
            pliego_protect::sanitize(i, o, s.password.as_deref(), &s.options.unwrap_or_default(), ctx)
        }),
    );
    r.register(
        "pages.rotate",
        typed(|i: &Path, o: &Path, p: OnPages<Degrees>, _: &JobContext| {
            edit_pages(i, o, p, |d, sel, x| pliego_organize::rotate(d, sel, x.degrees))
        }),
    );
    r.register(
        "pages.delete",
        typed(|i: &Path, o: &Path, p: OnPages<Nothing>, _: &JobContext| {
            edit_pages(i, o, p, |d, sel, _| pliego_organize::delete(d, sel))
        }),
    );
    r.register(
        "stamp.header_footer",
        typed(
            |i: &Path, o: &Path, p: OnPages<pliego_stamp::HeaderFooter>, ctx: &JobContext| {
                let sctx = pliego_stamp::StampContext::today(file_name(i));
                edit_pages(i, o, p, |d, sel, hf| {
                    pliego_stamp::header_footer(d, sel, &hf, &sctx, ctx)
                })
            },
        ),
    );
    r.register(
        "stamp.watermark",
        typed(
            |i: &Path, o: &Path, p: OnPages<pliego_stamp::Watermark>, ctx: &JobContext| {
                edit_pages(i, o, p, |d, sel, w| pliego_stamp::watermark(d, sel, &w, ctx))
            },
        ),
    );
    r.register(
        "stamp.bates",
        typed(
            |i: &Path, o: &Path, p: OnPages<pliego_stamp::BatesOptions>, ctx: &JobContext| {
                edit_pages(i, o, p, |d, sel, b| pliego_stamp::bates(d, sel, &b, ctx))
            },
        ),
    );
    r
}

/// Progreso por línea en stderr (no estorba a `--json` en stdout).
struct LineProgress;
impl Progress for LineProgress {
    fn report(&self, done: u64, total: u64, message: &str) {
        eprintln!("[{done}/{total}] {message}");
    }
}

pub fn run(a: RunArgs, json: bool) -> anyhow::Result<()> {
    let registry = builtin_registry();
    if a.list_steps {
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&registry.kinds().collect::<Vec<_>>())?
            );
        } else {
            for k in registry.kinds() {
                println!("{k}");
            }
        }
        return Ok(());
    }
    let action_path = a.action.context("falta la acción")?;
    let action = Action::load(&action_path)?;
    let output_dir = a
        .output
        .context("falta la carpeta de salida: usa -o carpeta (los originales nunca se modifican)")?;
    anyhow::ensure!(!a.inputs.is_empty(), "indica al menos un PDF o carpeta de entrada");
    let inputs = collect_inputs(&a.inputs, Some(&output_dir))?;
    anyhow::ensure!(!inputs.is_empty(), "no hay ningún PDF en las entradas indicadas");

    let ctx = JobContext::new(&LineProgress, Default::default());
    let r = pliego_actions::run(
        &action,
        &registry,
        &inputs,
        &RunOptions {
            output_dir,
            resume: a.resume,
        },
        &ctx,
    )?;
    if json {
        println!("{}", serde_json::to_string_pretty(&r)?);
    } else {
        for w in &r.report.warnings {
            eprintln!("aviso: {w}");
        }
        println!("{}", r.report.summary);
    }
    if !r.failures.is_empty() {
        std::process::exit(2);
    }
    Ok(())
}
