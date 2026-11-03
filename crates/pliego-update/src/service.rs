//! Servicio de actualizaciones: la máquina de estados que usan la app y la CLI.
//!
//! No sabe nada de Tauri ni de Windows. La app le da la configuración resuelta, el tipo de instalación y un
//! [`Stager`] (en Windows, `pliego_install::stage`), y consulta [`UpdateService::status`] para pintar la pastilla,
//! el popover y Ajustes > Actualizaciones.

use std::path::{Path, PathBuf};

use pliego_core::JobContext;
use semver::Version;
use serde::Serialize;
use time::OffsetDateTime;

use crate::client::{CheckOutcome, UpdateClient, Verdict};
use crate::config::{UpdateConfig, UpdateMode};
use crate::error::{Result, UpdateError};
use crate::install::InstallKind;
use crate::manifest::{format_time, parse_time};
use crate::schedule::{self, Attempt};
use crate::state::{PendingUpdate, UpdateState};

/// Prepara un paquete ya verificado para activarlo en el próximo arranque (extraer a `app-<versión>` y escribir
/// `pending`). Lo implementa la app con `pliego_install::stage`; en las pruebas, un doble.
pub trait Stager {
    fn stage(&self, package: &Path, version: &Version) -> std::result::Result<(), String>;
}

/// Fase visible en la interfaz.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "phase", rename_all = "kebab-case")]
pub enum Phase {
    /// Sin URL o sin claves compiladas: no se hace ninguna petición.
    NotConfigured,
    /// Modo desactivado (usuario o directiva) o instalación MSI.
    Disabled,
    UpToDate,
    /// Hay versión nueva que aún no se ha descargado (modo "solo avisar", conexión medida o instalación portátil).
    Available {
        version: Version,
        mandatory: bool,
        notes: Option<String>,
        notes_url: Option<String>,
        /// `true` si la app puede descargarla y prepararla; si no, solo se enlaza la página de descarga.
        can_download: bool,
    },
    Downloading {
        done: u64,
        total: u64,
    },
    /// Preparada: se activa al cerrar Pliego o con "Reiniciar ahora".
    Ready {
        version: Version,
        mandatory: bool,
        notes: Option<String>,
        needs_elevation: bool,
    },
    /// Error en lenguaje llano; `retry_at` es cuándo se reintentará (RFC 3339).
    Failed {
        message: String,
        retry_at: String,
    },
}

/// Instantánea para la interfaz (evento `update-state-changed` y comando `update_status`).
#[derive(Debug, Clone, Serialize)]
pub struct Status {
    #[serde(flatten)]
    pub phase: Phase,
    pub current_version: Version,
    pub channel: String,
    pub mode: UpdateMode,
    /// "Gestionado por tu organización": los controles de Ajustes quedan bloqueados.
    pub managed: bool,
    pub install_kind: InstallKind,
    pub last_check: Option<String>,
    /// Servidor en uso (Ajustes > Avanzado, solo lectura).
    pub server: Option<String>,
    pub history: Vec<(Version, String)>,
    /// Novedades de la versión recién instalada, para la tarjeta de Inicio.
    pub whats_new: Option<Version>,
    /// "Se volvió a X porque Y no arrancaba."
    pub rollback: Option<(Version, Version)>,
}

pub struct UpdateService<S: Stager> {
    config: UpdateConfig,
    kind: InstallKind,
    state: UpdateState,
    state_path: PathBuf,
    downloads: PathBuf,
    stager: S,
    phase: Phase,
    started: OffsetDateTime,
    last_attempt: Option<Attempt>,
}

impl<S: Stager> UpdateService<S> {
    /// Abre el servicio: carga el estado, lo concilia con la versión en marcha y decide la fase inicial.
    /// `rolled_back_from`: versión que el lanzador abandonó por no arrancar, si la hubo.
    pub fn open(
        config: UpdateConfig,
        kind: InstallKind,
        state_path: PathBuf,
        downloads: PathBuf,
        stager: S,
        rolled_back_from: Option<&Version>,
    ) -> Result<Self> {
        let now = OffsetDateTime::now_utc();
        let mut state = UpdateState::load_or_create(&state_path)?;
        if state.reconcile_launch(&config.current_version, &format_time(now), rolled_back_from) {
            state.save(&state_path)?;
        }
        let mut svc = Self {
            config,
            kind,
            state,
            state_path,
            downloads,
            stager,
            phase: Phase::UpToDate,
            started: now,
            last_attempt: None,
        };
        svc.phase = svc.initial_phase();
        Ok(svc)
    }

    fn active(&self) -> bool {
        self.config.enabled() && self.kind.checks()
    }

    fn initial_phase(&self) -> Phase {
        if let Some(p) = &self.state.pending {
            return self.ready(p);
        }
        if self.config.base_url.is_none() || self.config.trusted_keys.is_empty() {
            Phase::NotConfigured
        } else if !self.active() {
            Phase::Disabled
        } else {
            Phase::UpToDate
        }
    }

    fn ready(&self, p: &PendingUpdate) -> Phase {
        Phase::Ready {
            version: p.version.clone(),
            mandatory: p.mandatory,
            notes: p.notes.clone(),
            needs_elevation: self.kind.needs_elevation(),
        }
    }

    pub fn status(&self) -> Status {
        Status {
            phase: self.phase.clone(),
            current_version: self.config.current_version.clone(),
            channel: self.config.channel.clone(),
            mode: self.config.mode,
            managed: self.config.managed,
            install_kind: self.kind,
            last_check: self.state.last_check.clone(),
            server: self.config.base_url.clone(),
            history: self
                .state
                .history
                .iter()
                .map(|h| (h.version.clone(), h.at.clone()))
                .collect(),
            whats_new: self.state.whats_new.clone(),
            rollback: self
                .state
                .rollback_notice
                .as_ref()
                .map(|r| (r.failed.clone(), r.restored.clone())),
        }
    }

    /// Instante de la próxima comprobación automática; `None` si no hay que comprobar nunca.
    pub fn next_check_at(&self) -> Option<OffsetDateTime> {
        if !self.active() || matches!(self.phase, Phase::Ready { .. }) {
            return None;
        }
        let last_ok = self.state.last_check.as_deref().and_then(|s| parse_time(s).ok());
        Some(schedule::next_check(self.started, last_ok, self.last_attempt))
    }

    /// Una vuelta completa: comprueba y, según modo y conexión, descarga y prepara.
    /// `metered`: conexión de uso medido de Windows (solo comprobar, salvo obligatorias).
    pub fn run_cycle(&mut self, metered: bool, ctx: &JobContext) -> &Phase {
        if !self.active() || matches!(self.phase, Phase::Ready { .. }) {
            return &self.phase;
        }
        let now = OffsetDateTime::now_utc();
        match self.check() {
            Ok(CheckOutcome::Available(verdict)) => {
                self.last_attempt = Some(Attempt { at: now, ok: true });
                self.on_available(verdict, metered, ctx);
            }
            Ok(_) => {
                self.last_attempt = Some(Attempt { at: now, ok: true });
                self.phase = Phase::UpToDate;
            }
            Err(e) => self.fail(e, now),
        }
        &self.phase
    }

    fn check(&mut self) -> Result<CheckOutcome> {
        let client = UpdateClient::new(self.config.clone())?;
        let outcome = client.check(&mut self.state)?;
        self.state.save(&self.state_path)?;
        Ok(outcome)
    }

    fn on_available(&mut self, verdict: Verdict, metered: bool, ctx: &JobContext) {
        let can_download = self.kind.can_stage();
        self.phase = Phase::Available {
            version: verdict.release.version.clone(),
            mandatory: verdict.mandatory,
            notes: verdict.release.notes_for("es").map(str::to_owned),
            notes_url: verdict.release.notes_url.clone(),
            can_download,
        };
        // Las obligatorias se descargan también en "solo avisar"; la conexión medida solo las deja pasar a ellas.
        let auto = match self.config.mode {
            UpdateMode::Automatic => !metered || verdict.mandatory,
            UpdateMode::NotifyOnly => verdict.mandatory,
            UpdateMode::Disabled => false,
        };
        if auto && can_download {
            self.download_and_stage(&verdict, ctx);
        }
    }

    /// Descarga pedida por el usuario ("Descargar" en modo "solo avisar" o con conexión medida).
    pub fn download(&mut self, ctx: &JobContext) -> &Phase {
        if !self.active() {
            return &self.phase;
        }
        let verdict = match self.check() {
            Ok(CheckOutcome::Available(v)) if self.kind.can_stage() => v,
            Ok(_) => return &self.phase,
            Err(e) => {
                self.fail(e, OffsetDateTime::now_utc());
                return &self.phase;
            }
        };
        self.download_and_stage(&verdict, ctx);
        &self.phase
    }

    fn download_and_stage(&mut self, verdict: &Verdict, ctx: &JobContext) {
        let version = verdict.release.version.clone();
        let total = verdict.package.size;
        self.phase = Phase::Downloading { done: 0, total };
        let result = UpdateClient::new(self.config.clone())
            .and_then(|c| c.download(&verdict.package, &self.downloads, ctx))
            .and_then(|path| {
                self.stager
                    .stage(&path, &version)
                    .map_err(|e| UpdateError::Malformed(format!("no se pudo preparar la versión {version}: {e}")))
            });
        match result {
            Ok(()) => {
                let pending = PendingUpdate {
                    version,
                    notes: verdict.release.notes_for("es").map(str::to_owned),
                    mandatory: verdict.mandatory,
                };
                self.phase = self.ready(&pending);
                self.state.pending = Some(pending);
                if let Err(e) = self.state.save(&self.state_path) {
                    self.fail(e, OffsetDateTime::now_utc());
                }
            }
            Err(e) => self.fail(e, OffsetDateTime::now_utc()),
        }
    }

    fn fail(&mut self, e: UpdateError, now: OffsetDateTime) {
        self.last_attempt = Some(Attempt { at: now, ok: false });
        let retry = now + schedule::MIN_GAP;
        self.phase = match e {
            UpdateError::Cancelled => Phase::UpToDate,
            e => Phase::Failed {
                message: format!("No se pudo comprobar: {e}"),
                retry_at: format_time(retry),
            },
        };
    }

    /// "Omitir esta versión": no vuelve a avisar de ella (sí de las siguientes). Las obligatorias no se omiten.
    pub fn skip_version(&mut self) -> Result<()> {
        if let Phase::Available {
            version,
            mandatory: false,
            ..
        } = &self.phase
        {
            self.state.skipped_version = Some(version.clone());
            self.state.save(&self.state_path)?;
            self.phase = Phase::UpToDate;
        }
        Ok(())
    }

    /// El usuario cerró la tarjeta de novedades o el aviso de vuelta atrás.
    pub fn dismiss_notices(&mut self) -> Result<()> {
        self.state.whats_new = None;
        self.state.rollback_notice = None;
        self.state.save(&self.state_path)
    }

    /// Aplica ajustes nuevos (modo y canal desde Ajustes o cambios de directiva).
    pub fn set_config(&mut self, config: UpdateConfig) {
        let reset = config.channel != self.config.channel;
        self.config = config;
        if reset {
            // Otro canal: lo preparado del anterior ya no vale.
            self.state.pending = None;
        }
        self.phase = self.initial_phase();
    }

    pub fn config(&self) -> &UpdateConfig {
        &self.config
    }
}
