//! `pliego-release`: herramienta de publicación de versiones de Pliego.
//!
//! - `keygen`: crea el par de claves de firma de publicaciones.
//! - `publish`: copia los artefactos al árbol del servidor y escribe el manifiesto firmado del canal.
//! - `resign`: vuelve a firmar el manifiesto vigente con nueva caducidad (sin publicar versión).
//! - `verify`: comprueba el manifiesto de un árbol con una clave pública.
//! - `serve`: servidor HTTP estático de pruebas en local (en producción, Caddy o nginx en el VPS).
//! - `check`: hace lo mismo que la app (comprobar y, si se pide, descargar) contra un servidor.

mod publish;
mod serve;

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use pliego_core::{JobContext, Progress};
use pliego_update::sign::ReleaseKey;
use pliego_update::{CheckOutcome, PublicKey, UpdateClient, UpdateConfig, UpdateState, Version};

#[derive(Parser)]
#[command(
    name = "pliego-release",
    version,
    about = "Publicación de versiones y servidor de actualizaciones de Pliego"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Genera una clave de firma nueva (archivo secreto + archivo público).
    Keygen {
        /// Carpeta de salida. La clave secreta NUNCA debe subirse al repositorio ni al VPS.
        #[arg(long, default_value = ".")]
        out: PathBuf,
        /// Nombre base de los archivos.
        #[arg(long, default_value = "pliego-update")]
        name: String,
    },
    /// Publica una versión en un canal.
    Publish(Box<publish::PublishArgs>),
    /// Vuelve a firmar el manifiesto vigente con un serial y una caducidad nuevos.
    Resign {
        #[arg(long)]
        root: PathBuf,
        #[arg(long, default_value = "stable")]
        channel: String,
        #[arg(long = "key", required = true)]
        keys: Vec<PathBuf>,
        #[arg(long, default_value_t = 90)]
        expires_days: i64,
    },
    /// Verifica el manifiesto de un canal y los artefactos locales.
    Verify {
        #[arg(long)]
        root: PathBuf,
        #[arg(long, default_value = "stable")]
        channel: String,
        /// Clave pública (`ed25519:…`) o ruta a un archivo `.pub`.
        #[arg(long = "pubkey", required = true)]
        pubkeys: Vec<String>,
    },
    /// Sirve un árbol de actualizaciones por HTTP en local, para pruebas.
    Serve {
        #[arg(long)]
        root: PathBuf,
        #[arg(long, default_value = "127.0.0.1:8787")]
        addr: String,
    },
    /// Comprueba actualizaciones como lo haría la app.
    Check {
        /// URL base del servidor (p. ej. http://127.0.0.1:8787).
        #[arg(long)]
        url: String,
        #[arg(long, default_value = "stable")]
        channel: String,
        #[arg(long = "pubkey", required = true)]
        pubkeys: Vec<String>,
        /// Versión instalada que se simula.
        #[arg(long)]
        current: Version,
        /// Archivo de estado (se crea si no existe).
        #[arg(long)]
        state: Option<PathBuf>,
        /// Si hay versión nueva, descargar el paquete a esta carpeta.
        #[arg(long)]
        download_to: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Keygen { out, name } => keygen(&out, &name),
        Command::Publish(args) => publish::publish(&args),
        Command::Resign {
            root,
            channel,
            keys,
            expires_days,
        } => publish::resign(&root, &channel, &load_keys(&keys)?, expires_days),
        Command::Verify { root, channel, pubkeys } => publish::verify(&root, &channel, &parse_pubkeys(&pubkeys)?),
        Command::Serve { root, addr } => serve::serve(&root, &addr),
        Command::Check {
            url,
            channel,
            pubkeys,
            current,
            state,
            download_to,
        } => check(&url, &channel, parse_pubkeys(&pubkeys)?, current, state, download_to),
    }
}

fn keygen(out: &std::path::Path, name: &str) -> Result<()> {
    std::fs::create_dir_all(out)?;
    let secret = out.join(format!("{name}.key"));
    let public = out.join(format!("{name}.pub"));
    if secret.exists() {
        bail!("ya existe {}; no se sobrescribe una clave", secret.display());
    }
    let key = ReleaseKey::generate()?;
    std::fs::write(&secret, key.to_secret_text())?;
    std::fs::write(&public, format!("{}\n", key.public().to_text()))?;
    println!(
        "Clave secreta: {}  (guárdala sin conexión; no la subas al repositorio ni al VPS)",
        secret.display()
    );
    println!("Clave pública: {}", public.display());
    println!("Identificador: {}", key.public().key_id());
    println!(
        "Para compilar la app con esta clave: PLIEGO_UPDATE_PUBKEYS={}",
        key.public().to_text()
    );
    Ok(())
}

pub(crate) fn load_keys(paths: &[PathBuf]) -> Result<Vec<ReleaseKey>> {
    paths
        .iter()
        .map(|p| {
            let text = std::fs::read_to_string(p).with_context(|| format!("no se pudo leer {}", p.display()))?;
            Ok(ReleaseKey::from_secret_text(&text)?)
        })
        .collect()
}

fn parse_pubkeys(items: &[String]) -> Result<Vec<PublicKey>> {
    items
        .iter()
        .map(|s| {
            let text = if std::path::Path::new(s).is_file() {
                std::fs::read_to_string(s)?
            } else {
                s.clone()
            };
            Ok(PublicKey::parse(&text)?)
        })
        .collect()
}

struct Bar;

impl Progress for Bar {
    fn report(&self, done: u64, total: u64, message: &str) {
        let pct = (done * 100).checked_div(total).unwrap_or(100);
        eprint!("\r{message}: {pct:>3} % ({done}/{total} bytes)");
        if done >= total {
            eprintln!();
        }
    }
}

fn check(
    url: &str,
    channel: &str,
    keys: Vec<PublicKey>,
    current: Version,
    state_path: Option<PathBuf>,
    download_to: Option<PathBuf>,
) -> Result<()> {
    let config = UpdateConfig::new(url, channel, current, keys);
    let client = UpdateClient::new(config)?;
    let mut state = match &state_path {
        Some(p) => UpdateState::load(p)?,
        None => None,
    }
    .unwrap_or_else(|| {
        let mut seed = [0u8; 16];
        let _ = getrandom_seed(&mut seed);
        UpdateState::new(seed)
    });
    println!("Comprobando {}", client.manifest_url());
    let outcome = client.check(&mut state)?;
    if let Some(p) = &state_path {
        state.save(p)?;
    }
    match outcome {
        CheckOutcome::UpToDate => println!("Pliego está al día."),
        CheckOutcome::NotYet { version } => {
            println!("Hay {version}, pero esta instalación aún no está en el despliegue.")
        }
        CheckOutcome::Skipped { version } => println!("Hay {version}, omitida por el usuario."),
        CheckOutcome::Incompatible { version, reason } => println!("Hay {version}, pero {reason}."),
        CheckOutcome::Available(v) => {
            println!(
                "Nueva versión {}{} · {} bytes",
                v.release.version,
                if v.mandatory { " (obligatoria)" } else { "" },
                v.package.size
            );
            if let Some(notes) = v.release.notes_for("es") {
                println!("Novedades:\n{notes}");
            }
            if let Some(dir) = download_to {
                let ctx = JobContext::new(&Bar, pliego_core::CancelToken::new());
                let path = client.download(&v.package, &dir, &ctx)?;
                println!("Descargado y verificado: {}", path.display());
            }
        }
    }
    Ok(())
}

/// Semilla para el identificador de instalación de pruebas (hora y PID mezclados con SHA-256; la app usa el
/// generador aleatorio del sistema).
fn getrandom_seed(seed: &mut [u8; 16]) -> Result<()> {
    use sha2::{Digest, Sha256};
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut h = Sha256::new();
    h.update(nanos.to_le_bytes());
    h.update(std::process::id().to_le_bytes());
    seed.copy_from_slice(&h.finalize()[..16]);
    Ok(())
}
