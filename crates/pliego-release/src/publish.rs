//! Publicar, volver a firmar y verificar el árbol del servidor de actualizaciones.
//!
//! Árbol (lo que se sube tal cual al VPS):
//! ```text
//! <root>/v1/<canal>/manifest.json
//! <root>/packages/pliego-<versión>-<arq>.paq
//! <root>/packages/Pliego-Setup-<versión>-<arq>.exe
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use base64::Engine;
use clap::Args;
use pliego_update::sign::{ReleaseKey, seal};
use pliego_update::{Artifact, ArtifactKind, Envelope, Manifest, PublicKey, Release, SCHEMA, Version};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

#[derive(Args)]
pub struct PublishArgs {
    /// Raíz del árbol del servidor.
    #[arg(long)]
    pub root: PathBuf,
    #[arg(long, default_value = "stable")]
    pub channel: String,
    #[arg(long)]
    pub version: Version,
    #[arg(long, default_value = "x86_64")]
    pub arch: String,
    /// Paquete de la app (`.paq`) que usa el actualizador.
    #[arg(long)]
    pub package: PathBuf,
    /// Instalador completo (opcional).
    #[arg(long)]
    pub setup: Option<PathBuf>,
    /// MSI corporativo (opcional).
    #[arg(long)]
    pub msi: Option<PathBuf>,
    /// Notas en español (texto, o `@archivo.md`).
    #[arg(long)]
    pub notes_es: Option<String>,
    /// Notas en inglés (texto, o `@archivo.md`).
    #[arg(long)]
    pub notes_en: Option<String>,
    #[arg(long)]
    pub notes_url: Option<String>,
    /// Porcentaje de instalaciones que la reciben.
    #[arg(long, default_value_t = 100, value_parser = clap::value_parser!(u8).range(1..=100))]
    pub rollout: u8,
    /// Corrige un fallo de seguridad.
    #[arg(long)]
    pub critical: bool,
    #[arg(long)]
    pub min_supported: Option<Version>,
    /// Versión bloqueada (repetible).
    #[arg(long = "block")]
    pub blocked: Vec<Version>,
    #[arg(long)]
    pub min_windows_build: Option<u32>,
    #[arg(long, default_value_t = 90)]
    pub expires_days: i64,
    /// Clave secreta de firma (repetible, para rotaciones).
    #[arg(long = "key", required = true)]
    pub keys: Vec<PathBuf>,
}

pub fn publish(a: &PublishArgs) -> Result<()> {
    let keys = crate::load_keys(&a.keys)?;
    let manifest_path = manifest_path(&a.root, &a.channel);
    let previous = read_unverified(&manifest_path)?;
    if let Some(prev) = &previous
        && prev.latest.version > a.version
    {
        eprintln!(
            "Aviso: {} es anterior a la publicada ({}); los clientes con {} no bajarán de versión.",
            a.version, prev.latest.version, prev.latest.version
        );
    }

    let mut artifacts = vec![copy_artifact(
        &a.root,
        &a.package,
        ArtifactKind::Package,
        &a.arch,
        &format!("pliego-{}-{}.paq", a.version, a.arch),
    )?];
    if let Some(setup) = &a.setup {
        artifacts.push(copy_artifact(
            &a.root,
            setup,
            ArtifactKind::Setup,
            &a.arch,
            &format!("Pliego-Setup-{}-{}.exe", a.version, a.arch),
        )?);
    }
    if let Some(msi) = &a.msi {
        artifacts.push(copy_artifact(
            &a.root,
            msi,
            ArtifactKind::Msi,
            &a.arch,
            &format!("Pliego-{}-{}.msi", a.version, a.arch),
        )?);
    }

    let mut notes = BTreeMap::new();
    for (lang, text) in [("es", &a.notes_es), ("en", &a.notes_en)] {
        if let Some(t) = text {
            notes.insert(lang.to_owned(), read_text_arg(t)?);
        }
    }
    let now = OffsetDateTime::now_utc().replace_nanosecond(0).expect("0 es válido");
    let manifest = Manifest {
        schema: SCHEMA,
        product: "pliego".into(),
        channel: a.channel.clone(),
        serial: previous.as_ref().map_or(1, |p| p.serial + 1),
        issued_at: fmt(now),
        expires_at: fmt(now + time::Duration::days(a.expires_days)),
        latest: Release {
            version: a.version.clone(),
            released_at: fmt(now),
            notes,
            notes_url: a.notes_url.clone(),
            critical: a.critical,
            rollout_percent: a.rollout,
            min_windows_build: a.min_windows_build,
            artifacts,
        },
        min_supported_version: a.min_supported.clone(),
        blocked_versions: a.blocked.clone(),
    };
    write_envelope(&manifest_path, &manifest, &keys)?;
    println!(
        "Publicada {} en «{}» (serial {}, caduca {}). Sube la carpeta {} al servidor.",
        manifest.latest.version,
        manifest.channel,
        manifest.serial,
        manifest.expires_at,
        a.root.display()
    );
    Ok(())
}

pub fn resign(root: &Path, channel: &str, keys: &[ReleaseKey], expires_days: i64) -> Result<()> {
    let path = manifest_path(root, channel);
    let mut manifest = read_unverified(&path)?.with_context(|| format!("no hay manifiesto en {}", path.display()))?;
    let now = OffsetDateTime::now_utc().replace_nanosecond(0).expect("0 es válido");
    manifest.serial += 1;
    manifest.issued_at = fmt(now);
    manifest.expires_at = fmt(now + time::Duration::days(expires_days));
    write_envelope(&path, &manifest, keys)?;
    println!(
        "Manifiesto de «{channel}» firmado de nuevo (serial {}, caduca {}).",
        manifest.serial, manifest.expires_at
    );
    Ok(())
}

pub fn verify(root: &Path, channel: &str, keys: &[PublicKey]) -> Result<()> {
    let path = manifest_path(root, channel);
    let env =
        Envelope::from_json(&std::fs::read(&path).with_context(|| format!("no se pudo leer {}", path.display()))?)?;
    let m = env.open(keys)?;
    println!(
        "Firma válida. Canal «{}», versión {}, serial {}, caduca {}.",
        m.channel, m.latest.version, m.serial, m.expires_at
    );
    let mut ok = true;
    for a in &m.latest.artifacts {
        if a.url.contains("://") {
            println!("  {:?} {}: externo, no se comprueba", a.kind, a.url);
            continue;
        }
        let file = root.join(&a.url);
        let (size, sha) = digest(&file).unwrap_or((0, String::new()));
        let good = size == a.size && sha == a.sha256;
        ok &= good;
        println!(
            "  {:?} {}: {}",
            a.kind,
            a.url,
            if good { "correcto" } else { "NO COINCIDE" }
        );
    }
    if !ok {
        bail!("hay artefactos que no coinciden con el manifiesto");
    }
    Ok(())
}

fn manifest_path(root: &Path, channel: &str) -> PathBuf {
    root.join("v1").join(channel).join("manifest.json")
}

/// Lee la carga útil sin verificar la firma (solo para continuar el serial al publicar).
fn read_unverified(path: &Path) -> Result<Option<Manifest>> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let env = Envelope::from_json(&bytes)?;
    let payload = base64::engine::general_purpose::STANDARD.decode(&env.payload)?;
    Ok(Some(serde_json::from_slice(&payload)?))
}

fn write_envelope(path: &Path, manifest: &Manifest, keys: &[ReleaseKey]) -> Result<()> {
    let refs: Vec<&ReleaseKey> = keys.iter().collect();
    let env = seal(manifest, &refs);
    std::fs::create_dir_all(path.parent().expect("el manifiesto tiene carpeta"))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(&env)?)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

fn copy_artifact(root: &Path, src: &Path, kind: ArtifactKind, arch: &str, name: &str) -> Result<Artifact> {
    let dir = root.join("packages");
    std::fs::create_dir_all(&dir)?;
    let dest = dir.join(name);
    if std::fs::canonicalize(src).ok() != std::fs::canonicalize(&dest).ok() {
        std::fs::copy(src, &dest).with_context(|| format!("no se pudo copiar {}", src.display()))?;
    }
    let (size, sha256) = digest(&dest)?;
    Ok(Artifact {
        kind,
        arch: arch.to_owned(),
        url: format!("packages/{name}"),
        size,
        sha256,
    })
}

fn digest(path: &Path) -> Result<(u64, String)> {
    let mut f = std::fs::File::open(path).with_context(|| format!("no se pudo abrir {}", path.display()))?;
    let mut h = Sha256::new();
    let size = std::io::copy(&mut f, &mut h)?;
    Ok((size, h.finalize().iter().map(|b| format!("{b:02x}")).collect()))
}

fn read_text_arg(s: &str) -> Result<String> {
    match s.strip_prefix('@') {
        Some(p) => Ok(std::fs::read_to_string(p).with_context(|| format!("no se pudo leer {p}"))?),
        None => Ok(s.to_owned()),
    }
}

fn fmt(t: OffsetDateTime) -> String {
    t.format(&Rfc3339).expect("RFC 3339 siempre formatea")
}
