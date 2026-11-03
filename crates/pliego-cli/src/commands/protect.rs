use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use clap::{Args, Subcommand, ValueEnum};
use pliego_core::{JobContext, Report};
use pliego_protect::{Cipher, Encryption, Permissions, SanitizeOptions};

#[derive(Subcommand)]
pub enum ProtectCommand {
    /// Cifra el documento con contraseña (AES-256 por defecto).
    Encrypt(EncryptArgs),
    /// Quita la protección de un documento del que conoces la contraseña.
    Decrypt(DecryptArgs),
    /// Muestra o cambia los permisos del documento.
    Permissions(PermissionsArgs),
    /// Elimina información oculta (metadatos, JavaScript, adjuntos, comentarios...).
    Sanitize(SanitizeArgs),
}

/// Permisos que se pueden denegar.
#[derive(Clone, Copy, ValueEnum)]
pub enum Perm {
    Print,
    PrintHq,
    Copy,
    Modify,
    Annotate,
    FillForms,
    Accessibility,
    Assemble,
    /// Deniega todo.
    All,
}

fn deny(list: &[Perm]) -> Permissions {
    let mut p = Permissions::all();
    for d in list {
        match d {
            Perm::Print => p.print = false,
            Perm::PrintHq => p.print_high_quality = false,
            Perm::Copy => p.copy = false,
            Perm::Modify => p.modify = false,
            Perm::Annotate => p.annotate = false,
            Perm::FillForms => p.fill_forms = false,
            Perm::Accessibility => p.accessibility = false,
            Perm::Assemble => p.assemble = false,
            Perm::All => p = Permissions::none(),
        }
    }
    p
}

#[derive(Args)]
pub struct EncryptArgs {
    /// PDF de entrada.
    pub input: PathBuf,
    /// PDF de salida (no se sobrescribe la entrada).
    #[arg(short, long)]
    pub output: PathBuf,
    /// Contraseña para abrir el documento.
    #[arg(long, default_value = "")]
    pub password: String,
    /// Contraseña de permisos. Obligatoria si usas --deny.
    #[arg(long, default_value = "")]
    pub owner_password: String,
    /// Usa AES-128 en vez de AES-256 (para lectores antiguos).
    #[arg(long)]
    pub aes128: bool,
    /// Permisos a denegar, separados por comas.
    #[arg(long, value_delimiter = ',')]
    pub deny: Vec<Perm>,
    /// Contraseña del documento de entrada, si ya estaba cifrado.
    #[arg(long)]
    pub current_password: Option<String>,
}

#[derive(Args)]
pub struct DecryptArgs {
    pub input: PathBuf,
    #[arg(short, long)]
    pub output: PathBuf,
    /// Contraseña actual del documento.
    #[arg(long)]
    pub password: String,
}

#[derive(Args)]
pub struct PermissionsArgs {
    pub input: PathBuf,
    /// Si lo indicas, escribe un documento nuevo con los permisos cambiados; si no, solo los muestra.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    /// Contraseña actual del documento, si la tiene.
    #[arg(long)]
    pub current_password: Option<String>,
    /// Contraseña de permisos del documento nuevo (obligatoria con --output).
    #[arg(long)]
    pub owner_password: Option<String>,
    /// Contraseña para abrir el documento nuevo (vacía = se abre sin contraseña).
    #[arg(long, default_value = "")]
    pub password: String,
    /// Permisos a denegar, separados por comas.
    #[arg(long, value_delimiter = ',')]
    pub deny: Vec<Perm>,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum Keep {
    Metadata,
    Javascript,
    OpenActions,
    Attachments,
    EmbeddedFiles,
    Comments,
    FormData,
    Thumbnails,
}

#[derive(Args)]
pub struct SanitizeArgs {
    pub input: PathBuf,
    #[arg(short, long)]
    pub output: PathBuf,
    /// Contraseña del documento, si la tiene.
    #[arg(long)]
    pub password: Option<String>,
    /// Elementos que NO se eliminan, separados por comas.
    #[arg(long, value_delimiter = ',')]
    pub keep: Vec<Keep>,
}

pub fn run(cmd: ProtectCommand, json: bool) -> anyhow::Result<()> {
    match cmd {
        ProtectCommand::Encrypt(a) => {
            let enc = Encryption {
                cipher: if a.aes128 { Cipher::Aes128 } else { Cipher::Aes256 },
                user_password: a.password,
                owner_password: a.owner_password,
                permissions: deny(&a.deny),
            };
            let rep = pliego_protect::encrypt(&a.input, &a.output, a.current_password.as_deref(), &enc)?;
            print_report(&rep, json)
        }
        ProtectCommand::Decrypt(a) => {
            let rep = pliego_protect::decrypt(&a.input, &a.output, &a.password)?;
            print_report(&rep, json)
        }
        ProtectCommand::Permissions(a) => permissions(a, json),
        ProtectCommand::Sanitize(a) => {
            let mut opts = SanitizeOptions::default();
            for k in &a.keep {
                match k {
                    Keep::Metadata => opts.metadata = false,
                    Keep::Javascript => opts.javascript = false,
                    Keep::OpenActions => opts.open_actions = false,
                    Keep::Attachments => opts.attachments = false,
                    Keep::EmbeddedFiles => opts.embedded_files = false,
                    Keep::Comments => opts.comments = false,
                    Keep::FormData => opts.form_data = false,
                    Keep::Thumbnails => opts.thumbnails = false,
                }
            }
            let rep =
                pliego_protect::sanitize(&a.input, &a.output, a.password.as_deref(), &opts, &JobContext::silent())
                    .with_context(|| format!("no se pudo sanear {}", a.input.display()))?;
            print_report(&rep, json)
        }
    }
}

fn permissions(a: PermissionsArgs, json: bool) -> anyhow::Result<()> {
    match a.output {
        None => {
            let p = pliego_protect::permissions_of(&a.input, a.current_password.as_deref())?;
            if json {
                println!("{}", serde_json::to_string_pretty(&p)?);
            } else {
                print_permissions(&p);
            }
            Ok(())
        }
        Some(out) => {
            let Some(owner) = a.owner_password else {
                bail!("indica --owner-password: sin ella cualquiera podría quitar las restricciones");
            };
            let rep = pliego_protect::set_permissions(
                &a.input,
                &out,
                a.current_password.as_deref(),
                &owner,
                &a.password,
                deny(&a.deny),
            )?;
            print_report(&rep, json)
        }
    }
}

fn print_permissions(p: &Permissions) {
    let rows = [
        ("imprimir", p.print),
        ("imprimir en alta calidad", p.print_high_quality),
        ("copiar texto e imágenes", p.copy),
        ("modificar", p.modify),
        ("comentar", p.annotate),
        ("rellenar formularios", p.fill_forms),
        ("accesibilidad", p.accessibility),
        ("ensamblar páginas", p.assemble),
    ];
    for (name, on) in rows {
        println!("{:<26}{}", name, if on { "permitido" } else { "denegado" });
    }
}

fn print_report(rep: &Report, json: bool) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(rep)?);
    } else {
        println!("{}", rep.summary);
        for o in &rep.outputs {
            println!("-> {}", Path::new(o).display());
        }
        for w in &rep.warnings {
            eprintln!("aviso: {w}");
        }
    }
    Ok(())
}
