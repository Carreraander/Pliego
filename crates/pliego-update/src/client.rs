//! Comprobación y descarga.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use pliego_core::JobContext;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use ureq::Agent;
use ureq::tls::{TlsConfig, TlsProvider};

use crate::config::{UpdateConfig, check_url};
use crate::error::{Result, UpdateError};
use crate::manifest::{Artifact, ArtifactKind, Envelope, Manifest, Release, format_time};
use crate::rollout::bucket;
use crate::state::UpdateState;

/// Tolerancia de reloj al comprobar la caducidad.
const CLOCK_SKEW: time::Duration = time::Duration::hours(24);
/// Tamaño máximo aceptado para el manifiesto.
const MAX_MANIFEST_BYTES: u64 = 256 * 1024;
/// Tamaño máximo aceptado para un artefacto.
const MAX_ARTIFACT_BYTES: u64 = 1024 * 1024 * 1024;

/// Una versión nueva que se puede instalar.
#[derive(Debug, Clone, PartialEq)]
pub struct Verdict {
    pub release: Release,
    pub package: Artifact,
    /// Crítica, por debajo de la mínima soportada o versión bloqueada: la interfaz no deja posponerla sin fin.
    pub mandatory: bool,
    /// Sobre firmado tal como lo sirvió el servidor (lo llena `UpdateClient::check`). Se guarda junto al paquete para
    /// que `pliego-setup --update --manifest` vuelva a verificar la firma con privilegios.
    pub envelope: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::large_enum_variant)] // Se crea una vez por comprobación; no merece una caja.
pub enum CheckOutcome {
    UpToDate,
    Available(Verdict),
    /// Hay versión nueva pero esta instalación aún no está en el porcentaje del despliegue gradual.
    NotYet {
        version: semver::Version,
    },
    /// El usuario decidió omitir esta versión.
    Skipped {
        version: semver::Version,
    },
    /// La versión nueva necesita un Windows más reciente o no hay paquete para esta arquitectura.
    Incompatible {
        version: semver::Version,
        reason: String,
    },
}

/// Decide qué hacer con un manifiesto ya verificado. Función pura: no toca red ni disco.
pub fn evaluate(
    manifest: &Manifest,
    config: &UpdateConfig,
    state: &UpdateState,
    now: OffsetDateTime,
) -> Result<CheckOutcome> {
    if manifest.product != "pliego" || manifest.channel != config.channel {
        return Err(UpdateError::WrongTarget);
    }
    if now > manifest.expires()? + CLOCK_SKEW {
        return Err(UpdateError::Expired);
    }
    if let Some(&seen) = state.last_serial.get(&config.channel)
        && manifest.serial < seen
    {
        return Err(UpdateError::Replayed);
    }
    let current = &config.current_version;
    let release = &manifest.latest;
    if release.version <= *current {
        return Ok(CheckOutcome::UpToDate);
    }
    let Some(package) = release.artifact(ArtifactKind::Package, &config.arch) else {
        return Ok(CheckOutcome::Incompatible {
            version: release.version.clone(),
            reason: format!("no hay paquete para {}", config.arch),
        });
    };
    if let (Some(min), Some(build)) = (release.min_windows_build, config.os_build)
        && build < min
    {
        return Ok(CheckOutcome::Incompatible {
            version: release.version.clone(),
            reason: format!("necesita Windows compilación {min} o posterior"),
        });
    }
    let mandatory = release.critical
        || manifest.min_supported_version.as_ref().is_some_and(|min| current < min)
        || manifest.blocked_versions.contains(current);
    if !mandatory {
        if state.skipped_version.as_ref() == Some(&release.version) {
            return Ok(CheckOutcome::Skipped {
                version: release.version.clone(),
            });
        }
        let percent = release.rollout_percent.clamp(1, 100);
        if bucket(&state.install_id, &release.version) >= percent {
            return Ok(CheckOutcome::NotYet {
                version: release.version.clone(),
            });
        }
    }
    Ok(CheckOutcome::Available(Verdict {
        release: release.clone(),
        package: package.clone(),
        mandatory,
        envelope: Vec::new(),
    }))
}

pub struct UpdateClient {
    config: UpdateConfig,
    base_url: String,
    agent: Agent,
}

impl UpdateClient {
    pub fn new(config: UpdateConfig) -> Result<Self> {
        let base_url = config
            .base_url
            .clone()
            .ok_or_else(|| UpdateError::InsecureUrl("no hay servidor de actualizaciones configurado".into()))?;
        let loopback = check_url(&base_url)?;
        let mut builder = Agent::config_builder()
            .user_agent(&config.user_agent)
            .http_status_as_error(false)
            .timeout_connect(Some(Duration::from_secs(15)))
            .timeout_recv_response(Some(Duration::from_secs(30)))
            .tls_config(TlsConfig::builder().provider(TlsProvider::NativeTls).build());
        if loopback {
            // El servidor local de pruebas nunca va por el proxy del sistema.
            builder = builder.proxy(None);
        }
        Ok(Self {
            agent: builder.build().into(),
            base_url,
            config,
        })
    }

    pub fn config(&self) -> &UpdateConfig {
        &self.config
    }

    pub fn manifest_url(&self) -> String {
        format!("{}/v1/{}/manifest.json", self.base_url, self.config.channel)
    }

    /// URL absoluta de un artefacto (las relativas cuelgan de la URL base). Exige HTTPS salvo en local.
    pub fn artifact_url(&self, artifact: &Artifact) -> Result<String> {
        let url = if artifact.url.contains("://") {
            artifact.url.clone()
        } else {
            format!("{}/{}", self.base_url, artifact.url.trim_start_matches('/'))
        };
        check_url(&url)?;
        Ok(url)
    }

    fn get(&self, url: &str) -> Result<ureq::http::Response<ureq::Body>> {
        let resp = self
            .agent
            .get(url)
            .call()
            .map_err(|e| UpdateError::Network(e.to_string()))?;
        let status = resp.status().as_u16();
        if status != 200 {
            return Err(UpdateError::Http(status));
        }
        Ok(resp)
    }

    /// Descarga y verifica la firma del manifiesto del canal.
    pub fn fetch_manifest(&self) -> Result<Manifest> {
        self.fetch_envelope().map(|(_, m)| m)
    }

    /// Igual que [`Self::fetch_manifest`], devolviendo también los bytes del sobre firmado.
    pub fn fetch_envelope(&self) -> Result<(Vec<u8>, Manifest)> {
        let mut resp = self.get(&self.manifest_url())?;
        let bytes = resp
            .body_mut()
            .with_config()
            .limit(MAX_MANIFEST_BYTES)
            .read_to_vec()
            .map_err(|e| UpdateError::Network(e.to_string()))?;
        let manifest = Envelope::from_json(&bytes)?.open(&self.config.trusted_keys)?;
        Ok((bytes, manifest))
    }

    /// Comprueba si hay versión nueva y actualiza `state` (serial y hora) si el manifiesto es válido.
    /// El llamador guarda `state` después.
    pub fn check(&self, state: &mut UpdateState) -> Result<CheckOutcome> {
        let (bytes, manifest) = self.fetch_envelope()?;
        let now = OffsetDateTime::now_utc();
        let mut outcome = evaluate(&manifest, &self.config, state, now)?;
        if let CheckOutcome::Available(v) = &mut outcome {
            v.envelope = bytes;
        }
        state.last_serial.insert(self.config.channel.clone(), manifest.serial);
        state.last_check = Some(format_time(now));
        Ok(outcome)
    }

    /// Descarga `artifact` a `dir` verificando tamaño y SHA-256. Si ya existe un archivo completo y correcto,
    /// lo reutiliza. Devuelve la ruta final.
    pub fn download(&self, artifact: &Artifact, dir: &Path, ctx: &JobContext) -> Result<PathBuf> {
        if artifact.size > MAX_ARTIFACT_BYTES {
            return Err(UpdateError::Malformed("artefacto demasiado grande".into()));
        }
        let url = self.artifact_url(artifact)?;
        let name = file_name(&url)?;
        std::fs::create_dir_all(dir)?;
        let dest = dir.join(&name);
        if dest.is_file() && file_matches(&dest, artifact)? {
            return Ok(dest);
        }
        let part = dir.join(format!("{name}.part"));
        let result = self.download_to(&url, &part, artifact, ctx);
        match result {
            Ok(()) => {
                std::fs::rename(&part, &dest)?;
                Ok(dest)
            }
            Err(e) => {
                let _ = std::fs::remove_file(&part);
                Err(e)
            }
        }
    }

    fn download_to(&self, url: &str, part: &Path, artifact: &Artifact, ctx: &JobContext) -> Result<()> {
        let mut resp = self.get(url)?;
        let mut reader = resp
            .body_mut()
            .with_config()
            .limit(artifact.size.saturating_add(1))
            .reader();
        let mut file = std::fs::File::create(part)?;
        let mut hasher = Sha256::new();
        let mut buf = vec![0u8; 64 * 1024];
        let mut done = 0u64;
        let mut last_report = 0u64;
        loop {
            if ctx.cancel.is_cancelled() {
                return Err(UpdateError::Cancelled);
            }
            let n = reader.read(&mut buf).map_err(|e| UpdateError::Network(e.to_string()))?;
            if n == 0 {
                break;
            }
            done += n as u64;
            if done > artifact.size {
                return Err(UpdateError::DigestMismatch);
            }
            hasher.update(&buf[..n]);
            file.write_all(&buf[..n])?;
            if done - last_report >= 256 * 1024 || done == artifact.size {
                last_report = done;
                ctx.progress.report(done, artifact.size, "Descargando actualización");
            }
        }
        file.sync_all()?;
        if done != artifact.size || hex(&hasher.finalize()) != artifact.sha256.to_ascii_lowercase() {
            return Err(UpdateError::DigestMismatch);
        }
        Ok(())
    }
}

fn file_name(url: &str) -> Result<String> {
    let name = url
        .rsplit('/')
        .next()
        .unwrap_or("")
        .split(['?', '#'])
        .next()
        .unwrap_or("");
    let ok = !name.is_empty()
        && name != "."
        && name != ".."
        && name.chars().all(|c| c.is_ascii_alphanumeric() || "._-+".contains(c));
    if ok {
        Ok(name.to_owned())
    } else {
        Err(UpdateError::Malformed(format!("nombre de archivo no válido en {url}")))
    }
}

/// `true` si el archivo tiene el tamaño y la huella del artefacto.
pub(crate) fn file_matches(path: &Path, artifact: &Artifact) -> Result<bool> {
    if std::fs::metadata(path)?.len() != artifact.size {
        return Ok(false);
    }
    let mut f = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut f, &mut hasher)?;
    Ok(hex(&hasher.finalize()) == artifact.sha256.to_ascii_lowercase())
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use semver::Version;

    use super::*;

    fn manifest(version: &str) -> Manifest {
        Manifest {
            schema: 1,
            product: "pliego".into(),
            channel: "stable".into(),
            serial: 5,
            issued_at: "2026-11-01T00:00:00Z".into(),
            expires_at: "2026-12-01T00:00:00Z".into(),
            latest: Release {
                version: Version::parse(version).unwrap(),
                released_at: "2026-11-01T00:00:00Z".into(),
                notes: BTreeMap::new(),
                notes_url: None,
                critical: false,
                rollout_percent: 100,
                min_windows_build: Some(19041),
                artifacts: vec![Artifact {
                    kind: ArtifactKind::Package,
                    arch: "x86_64".into(),
                    url: "packages/pliego-0.2.0-x86_64.paq".into(),
                    size: 3,
                    sha256: "00".into(),
                }],
            },
            min_supported_version: None,
            blocked_versions: vec![],
        }
    }

    fn config() -> UpdateConfig {
        let mut c = UpdateConfig::new("https://x", "stable", Version::new(0, 1, 0), vec![]);
        c.arch = "x86_64".into();
        c.os_build = Some(22631);
        c
    }

    fn now() -> OffsetDateTime {
        time::macros::datetime!(2026-11-03 12:00 UTC)
    }

    fn state() -> UpdateState {
        UpdateState::new([7; 16])
    }

    #[test]
    fn newer_version_is_available() {
        let out = evaluate(&manifest("0.2.0"), &config(), &state(), now()).unwrap();
        assert!(matches!(out, CheckOutcome::Available(Verdict { mandatory: false, .. })));
    }

    #[test]
    fn same_or_older_is_up_to_date() {
        for v in ["0.1.0", "0.0.9"] {
            assert_eq!(
                evaluate(&manifest(v), &config(), &state(), now()).unwrap(),
                CheckOutcome::UpToDate
            );
        }
    }

    #[test]
    fn expired_wrong_channel_and_replay_are_rejected() {
        let late = time::macros::datetime!(2026-12-05 00:00 UTC);
        assert!(matches!(
            evaluate(&manifest("0.2.0"), &config(), &state(), late),
            Err(UpdateError::Expired)
        ));
        let mut c = config();
        c.channel = "beta".into();
        assert!(matches!(
            evaluate(&manifest("0.2.0"), &c, &state(), now()),
            Err(UpdateError::WrongTarget)
        ));
        let mut s = state();
        s.last_serial.insert("stable".into(), 6);
        assert!(matches!(
            evaluate(&manifest("0.2.0"), &config(), &s, now()),
            Err(UpdateError::Replayed)
        ));
    }

    #[test]
    fn rollout_skip_and_mandatory() {
        let mut m = manifest("0.2.0");
        m.latest.rollout_percent = 1;
        // Con 1 % casi todas las instalaciones esperan; buscamos una que quede fuera.
        let s = (0..100u8)
            .map(|i| UpdateState::new([i; 16]))
            .find(|s| bucket(&s.install_id, &m.latest.version) >= 1)
            .unwrap();
        assert!(matches!(
            evaluate(&m, &config(), &s, now()).unwrap(),
            CheckOutcome::NotYet { .. }
        ));
        // Por debajo de la mínima soportada: obligatoria, ignora el despliegue gradual.
        m.min_supported_version = Some(Version::new(0, 2, 0));
        assert!(matches!(
            evaluate(&m, &config(), &s, now()).unwrap(),
            CheckOutcome::Available(Verdict { mandatory: true, .. })
        ));
        let mut skipped = state();
        skipped.skipped_version = Some(Version::new(0, 2, 0));
        assert!(matches!(
            evaluate(&manifest("0.2.0"), &config(), &skipped, now()).unwrap(),
            CheckOutcome::Skipped { .. }
        ));
    }

    #[test]
    fn incompatible_windows_or_arch() {
        let mut c = config();
        c.os_build = Some(17763);
        assert!(matches!(
            evaluate(&manifest("0.2.0"), &c, &state(), now()).unwrap(),
            CheckOutcome::Incompatible { .. }
        ));
        let mut c = config();
        c.arch = "aarch64".into();
        assert!(matches!(
            evaluate(&manifest("0.2.0"), &c, &state(), now()).unwrap(),
            CheckOutcome::Incompatible { .. }
        ));
    }

    #[test]
    fn file_names_are_sanitised() {
        assert_eq!(
            file_name("https://x/p/pliego-0.2.0-x86_64.paq?a=1").unwrap(),
            "pliego-0.2.0-x86_64.paq"
        );
        assert!(file_name("https://x/p/..").is_err());
        assert!(file_name("https://x/p/").is_err());
        assert!(file_name("https://x/p/a%5Cb").is_err());
    }
}
