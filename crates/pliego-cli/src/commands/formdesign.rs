use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use clap::{Args, Subcommand, ValueEnum};
use mupdf::pdf::PdfDocument;
use pliego_formdesign::{CalcOp, Calculation, FieldFormat, FieldPatch, FieldSpec, FieldType, NumberFormat, Validation};

#[derive(Subcommand)]
pub enum FormDesignCommand {
    /// Lista los campos con sus propiedades y su rectángulo.
    List(DocArgs),
    /// Añade un campo.
    Add(AddArgs),
    /// Modifica un campo existente.
    Update(UpdateArgs),
    /// Borra un campo.
    Delete(DeleteArgs),
    /// Fija el orden de tabulación de los campos de una página.
    TabOrder(TabOrderArgs),
    /// Define (o quita) el cálculo automático de un campo.
    Calc(CalcArgs),
    /// Define la validación de un campo.
    Validate(ValidateArgs),
    /// Define el formato de un campo.
    Format(FormatArgs),
    /// Recalcula los campos calculados y guarda los valores.
    Recalc(RecalcArgs),
}

#[derive(Clone, Copy, ValueEnum)]
pub enum Op {
    Sum,
    Product,
    Average,
    Min,
    Max,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum ValidationKind {
    None,
    Range,
    Dni,
    Iban,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum FormatKind {
    None,
    Number,
    Percent,
    Date,
    Time,
    Postal,
    Mask,
}

#[derive(Args)]
pub struct CalcArgs {
    #[command(flatten)]
    pub doc: DocArgs,
    #[command(flatten)]
    pub out: OutArgs,
    /// Campo calculado.
    #[arg(long)]
    pub field: String,
    /// Operación.
    #[arg(long, value_enum, required_unless_present = "clear")]
    pub op: Option<Op>,
    /// Campos de entrada, separados por comas.
    #[arg(long, value_delimiter = ',', required_unless_present = "clear")]
    pub inputs: Vec<String>,
    /// Decimales del resultado.
    #[arg(long)]
    pub decimals: Option<u8>,
    /// Quita el cálculo del campo.
    #[arg(long)]
    pub clear: bool,
}

#[derive(Args)]
pub struct ValidateArgs {
    #[command(flatten)]
    pub doc: DocArgs,
    #[command(flatten)]
    pub out: OutArgs,
    #[arg(long)]
    pub field: String,
    #[arg(long, value_enum)]
    pub kind: ValidationKind,
    /// Mínimo del rango.
    #[arg(long, allow_hyphen_values = true)]
    pub min: Option<f64>,
    /// Máximo del rango.
    #[arg(long, allow_hyphen_values = true)]
    pub max: Option<f64>,
}

#[derive(Args)]
pub struct FormatArgs {
    #[command(flatten)]
    pub doc: DocArgs,
    #[command(flatten)]
    pub out: OutArgs,
    #[arg(long)]
    pub field: String,
    #[arg(long, value_enum)]
    pub kind: FormatKind,
    /// Decimales (número y porcentaje).
    #[arg(long, default_value_t = 2)]
    pub decimals: u8,
    /// Símbolo de moneda (número).
    #[arg(long)]
    pub currency: Option<String>,
    /// Patrón de fecha (`dd/mm/yyyy`), hora (`HH:MM`) o máscara (`999-AA`).
    #[arg(long)]
    pub pattern: Option<String>,
}

#[derive(Args)]
pub struct RecalcArgs {
    #[command(flatten)]
    pub doc: DocArgs,
    #[command(flatten)]
    pub out: OutArgs,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum Kind {
    Text,
    Checkbox,
    Radio,
    Combo,
    List,
    Button,
    Signature,
}

impl From<Kind> for FieldType {
    fn from(k: Kind) -> Self {
        match k {
            Kind::Text => FieldType::Text,
            Kind::Checkbox => FieldType::CheckBox,
            Kind::Radio => FieldType::RadioButton,
            Kind::Combo => FieldType::ComboBox,
            Kind::List => FieldType::ListBox,
            Kind::Button => FieldType::PushButton,
            Kind::Signature => FieldType::Signature,
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
pub struct AddArgs {
    #[command(flatten)]
    pub doc: DocArgs,
    #[command(flatten)]
    pub out: OutArgs,
    /// Nombre del campo (sin puntos).
    #[arg(long)]
    pub name: String,
    /// Tipo de campo.
    #[arg(long = "type", value_enum)]
    pub kind: Kind,
    /// Página (desde 1).
    #[arg(long, default_value_t = 1)]
    pub page: usize,
    /// Rectángulo `x0,y0,x1,y1` en puntos, con el origen abajo a la izquierda.
    #[arg(long, value_parser = parse_rect, allow_hyphen_values = true)]
    pub rect: [f32; 4],
    #[arg(long)]
    pub required: bool,
    #[arg(long)]
    pub read_only: bool,
    /// Valor por defecto.
    #[arg(long)]
    pub value: Option<String>,
    /// Opción de combo/lista (repetible).
    #[arg(long = "option")]
    pub options: Vec<String>,
    /// Estado «marcado» de casilla/radio.
    #[arg(long)]
    pub export_value: Option<String>,
    #[arg(long)]
    pub multiline: bool,
    #[arg(long)]
    pub password_field: bool,
    #[arg(long)]
    pub max_len: Option<u32>,
    /// Cuerpo de letra en puntos (0 = automático).
    #[arg(long, default_value_t = 0.0)]
    pub font_size: f32,
    #[arg(long)]
    pub tooltip: Option<String>,
    /// Rótulo de un botón.
    #[arg(long)]
    pub caption: Option<String>,
}

#[derive(Args)]
pub struct UpdateArgs {
    #[command(flatten)]
    pub doc: DocArgs,
    #[command(flatten)]
    pub out: OutArgs,
    /// Nombre actual del campo.
    #[arg(long)]
    pub field: String,
    /// Nombre nuevo.
    #[arg(long)]
    pub rename: Option<String>,
    #[arg(long, value_parser = parse_rect, allow_hyphen_values = true)]
    pub rect: Option<[f32; 4]>,
    #[arg(long)]
    pub required: Option<bool>,
    #[arg(long)]
    pub read_only: Option<bool>,
    #[arg(long)]
    pub value: Option<String>,
    /// Sustituye las opciones de combo/lista (repetible).
    #[arg(long = "option")]
    pub options: Vec<String>,
    #[arg(long)]
    pub multiline: Option<bool>,
    #[arg(long)]
    pub password_field: Option<bool>,
    /// Longitud máxima (0 la quita).
    #[arg(long)]
    pub max_len: Option<u32>,
    #[arg(long)]
    pub font_size: Option<f32>,
    #[arg(long)]
    pub tooltip: Option<String>,
}

#[derive(Args)]
pub struct DeleteArgs {
    #[command(flatten)]
    pub doc: DocArgs,
    #[command(flatten)]
    pub out: OutArgs,
    /// Campo que se borra.
    #[arg(long)]
    pub field: String,
}

#[derive(Args)]
pub struct TabOrderArgs {
    #[command(flatten)]
    pub doc: DocArgs,
    #[command(flatten)]
    pub out: OutArgs,
    /// Página (desde 1).
    #[arg(long, default_value_t = 1)]
    pub page: usize,
    /// Nombres de campo en el orden deseado, separados por comas.
    #[arg(long, value_delimiter = ',', required = true)]
    pub order: Vec<String>,
}

fn parse_rect(s: &str) -> Result<[f32; 4], String> {
    let v: Result<Vec<f32>, _> = s.split(',').map(|p| p.trim().parse::<f32>()).collect();
    match v {
        Ok(v) if v.len() == 4 => Ok([v[0], v[1], v[2], v[3]]),
        _ => Err("usa cuatro números separados por comas: x0,y0,x1,y1".into()),
    }
}

fn open(a: &DocArgs) -> anyhow::Result<PdfDocument> {
    let path = a.input.to_str().context("la ruta no es válida")?;
    let mut doc = PdfDocument::open(path).with_context(|| format!("no se pudo abrir {}", a.input.display()))?;
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

fn page0(p: usize) -> anyhow::Result<usize> {
    p.checked_sub(1).context("las páginas empiezan en 1")
}

fn done(msg: &str, dest: &Path, json: bool) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::json!({ "ok": true, "output": dest }));
    } else {
        println!("{msg}: {}", dest.display());
    }
    Ok(())
}

pub fn run(cmd: FormDesignCommand, json: bool) -> anyhow::Result<()> {
    match cmd {
        FormDesignCommand::List(a) => {
            let fields = pliego_formdesign::list_fields(&open(&a)?)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&fields)?);
            } else if fields.is_empty() {
                println!("El documento no tiene campos de formulario.");
            } else {
                for f in &fields {
                    let kind = serde_json::to_value(f.kind)?;
                    println!(
                        "{:<24} {:<12} pág. {:<3} [{:.0} {:.0} {:.0} {:.0}]{}",
                        f.name,
                        kind.as_str().unwrap_or(""),
                        f.page + 1,
                        f.rect[0],
                        f.rect[1],
                        f.rect[2],
                        f.rect[3],
                        if f.required { " obligatorio" } else { "" }
                    );
                }
            }
            Ok(())
        }
        FormDesignCommand::Add(a) => {
            let mut doc = open(&a.doc)?;
            let spec = FieldSpec {
                name: a.name,
                kind: a.kind.into(),
                page: page0(a.page)?,
                rect: a.rect,
                required: a.required,
                read_only: a.read_only,
                value: a.value,
                options: a.options,
                export_value: a.export_value,
                multiline: a.multiline,
                password: a.password_field,
                max_len: a.max_len,
                font_size: a.font_size,
                tooltip: a.tooltip,
                caption: a.caption,
                ..Default::default()
            };
            pliego_formdesign::add_field(&mut doc, &spec)?;
            let dest = save(&doc, &a.doc.input, &a.out)?;
            done("Campo añadido", &dest, json)
        }
        FormDesignCommand::Update(a) => {
            let mut doc = open(&a.doc)?;
            let patch = FieldPatch {
                name: a.rename,
                rect: a.rect,
                required: a.required,
                read_only: a.read_only,
                value: a.value,
                options: (!a.options.is_empty()).then_some(a.options),
                multiline: a.multiline,
                password: a.password_field,
                max_len: a.max_len,
                font_size: a.font_size,
                tooltip: a.tooltip,
            };
            pliego_formdesign::update_field(&mut doc, &a.field, &patch)?;
            let dest = save(&doc, &a.doc.input, &a.out)?;
            done("Campo modificado", &dest, json)
        }
        FormDesignCommand::Delete(a) => {
            let mut doc = open(&a.doc)?;
            pliego_formdesign::delete_field(&mut doc, &a.field)?;
            let dest = save(&doc, &a.doc.input, &a.out)?;
            done("Campo borrado", &dest, json)
        }
        FormDesignCommand::Calc(a) => {
            let mut doc = open(&a.doc)?;
            let calc = (!a.clear).then(|| Calculation {
                op: match a.op.unwrap_or(Op::Sum) {
                    Op::Sum => CalcOp::Sum,
                    Op::Product => CalcOp::Product,
                    Op::Average => CalcOp::Average,
                    Op::Min => CalcOp::Min,
                    Op::Max => CalcOp::Max,
                },
                fields: a.inputs.clone(),
                decimals: a.decimals,
            });
            pliego_formdesign::set_calculation(&mut doc, &a.field, calc.as_ref())?;
            let dest = save(&doc, &a.doc.input, &a.out)?;
            done("Cálculo guardado", &dest, json)
        }
        FormDesignCommand::Validate(a) => {
            let mut doc = open(&a.doc)?;
            let v = match a.kind {
                ValidationKind::None => Validation::None,
                ValidationKind::Range => Validation::Range { min: a.min, max: a.max },
                ValidationKind::Dni => Validation::Dni,
                ValidationKind::Iban => Validation::Iban,
            };
            pliego_formdesign::set_validation(&mut doc, &a.field, &v)?;
            let dest = save(&doc, &a.doc.input, &a.out)?;
            done("Validación guardada", &dest, json)
        }
        FormDesignCommand::Format(a) => {
            let mut doc = open(&a.doc)?;
            let pattern = |what: &str| a.pattern.clone().with_context(|| format!("{what} necesita --pattern"));
            let f = match a.kind {
                FormatKind::None => FieldFormat::None,
                FormatKind::Number => FieldFormat::Number(NumberFormat {
                    decimals: a.decimals,
                    currency: a.currency.clone(),
                    ..Default::default()
                }),
                FormatKind::Percent => FieldFormat::Percent { decimals: a.decimals },
                FormatKind::Date => FieldFormat::Date {
                    pattern: pattern("la fecha")?,
                },
                FormatKind::Time => FieldFormat::Time {
                    pattern: pattern("la hora")?,
                },
                FormatKind::Postal => FieldFormat::PostalCode,
                FormatKind::Mask => FieldFormat::Mask {
                    mask: pattern("la máscara")?,
                },
            };
            pliego_formdesign::set_format(&mut doc, &a.field, &f)?;
            let dest = save(&doc, &a.doc.input, &a.out)?;
            done("Formato guardado", &dest, json)
        }
        FormDesignCommand::Recalc(a) => {
            let mut doc = open(&a.doc)?;
            let names = pliego_formdesign::recalculate(&mut doc)?;
            let dest = save(&doc, &a.doc.input, &a.out)?;
            if json {
                println!("{}", serde_json::json!({ "recalculated": names, "output": dest }));
            } else {
                println!("{} campo(s) recalculado(s): {}", names.len(), dest.display());
            }
            Ok(())
        }
        FormDesignCommand::TabOrder(a) => {
            let mut doc = open(&a.doc)?;
            pliego_formdesign::set_tab_order(&mut doc, page0(a.page)?, &a.order)?;
            let dest = save(&doc, &a.doc.input, &a.out)?;
            done("Orden de tabulación fijado", &dest, json)
        }
    }
}
