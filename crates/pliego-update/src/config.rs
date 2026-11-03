//! Configuración del actualizador: de dónde sale cada valor y quién manda.
//!
//! Precedencia (de más a menos fuerte):
//! 1. Directiva de la organización (`HKLM\SOFTWARE\Policies\Pliego\Updates`, la lee la app en Windows).
//! 2. Variable de entorno `PLIEGO_UPDATE_URL` en tiempo de ejecución (pruebas con servidor local).
//! 3. Ajustes del usuario (modo y canal).
//! 4. Valor de compilación `PLIEGO_UPDATE_URL` (la URL del VPS se fija al compilar la versión oficial).
//!
//! Sin URL en ninguna fuente, las actualizaciones quedan desactivadas con el motivo "no configurado".

use semver::Version;
use serde::{Deserialize, Serialize};

use crate::error::{Result, UpdateError};
use crate::keys::PublicKey;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UpdateMode {
    /// Descarga en segundo plano y aplica al cerrar Pliego (o con "Reiniciar ahora").
    #[default]
    Automatic,
    /// Avisa de que hay versión nueva; descarga solo si el usuario lo pide.
    NotifyOnly,
    /// No comprueba nada.
    Disabled,
}

/// Lo que el usuario elige en Ajustes > Actualizaciones.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UserSettings {
    #[serde(default)]
    pub mode: UpdateMode,
    /// `stable` (por defecto) o `beta`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
}

/// Directiva de la organización. Cualquier campo presente gana a los ajustes del usuario y bloquea su control.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Policy {
    #[serde(default)]
    pub disabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<UpdateMode>,
}

impl Policy {
    pub fn is_empty(&self) -> bool {
        *self == Policy::default()
    }
}

/// Configuración efectiva.
#[derive(Debug, Clone)]
pub struct UpdateConfig {
    /// URL base del servidor, sin barra final (p. ej. `https://actualizaciones.ejemplo.es/pliego`).
    pub base_url: Option<String>,
    pub channel: String,
    pub mode: UpdateMode,
    /// `true` si alguna directiva fija valores: la interfaz muestra "Gestionado por tu organización".
    pub managed: bool,
    pub current_version: Version,
    /// `x86_64` o `aarch64`.
    pub arch: String,
    /// Compilación de Windows del equipo (para `min_windows_build`). `None` fuera de Windows.
    pub os_build: Option<u32>,
    pub trusted_keys: Vec<PublicKey>,
    pub user_agent: String,
}

impl UpdateConfig {
    /// Configuración mínima para pruebas y herramientas.
    pub fn new(base_url: &str, channel: &str, current_version: Version, trusted_keys: Vec<PublicKey>) -> Self {
        Self {
            base_url: Some(base_url.trim_end_matches('/').to_owned()),
            channel: channel.to_owned(),
            mode: UpdateMode::Automatic,
            managed: false,
            user_agent: format!("Pliego/{current_version}"),
            current_version,
            arch: std::env::consts::ARCH.to_owned(),
            os_build: None,
            trusted_keys,
        }
    }

    pub fn enabled(&self) -> bool {
        self.mode != UpdateMode::Disabled && self.base_url.is_some() && !self.trusted_keys.is_empty()
    }
}

/// Combina las fuentes según la precedencia del módulo.
pub fn resolve(
    build_url: Option<&str>,
    env_url: Option<&str>,
    user: &UserSettings,
    policy: &Policy,
    current_version: Version,
    trusted_keys: Vec<PublicKey>,
) -> UpdateConfig {
    let pick = |s: Option<&str>| {
        s.map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.trim_end_matches('/').to_owned())
    };
    let base_url = pick(policy.url.as_deref())
        .or_else(|| pick(env_url))
        .or_else(|| pick(build_url));
    let channel = policy
        .channel
        .clone()
        .or_else(|| user.channel.clone())
        .unwrap_or_else(|| "stable".to_owned());
    let mode = if policy.disabled {
        UpdateMode::Disabled
    } else {
        policy.mode.unwrap_or(user.mode)
    };
    UpdateConfig {
        base_url,
        channel,
        mode,
        managed: !policy.is_empty(),
        user_agent: format!("Pliego/{current_version} ({})", std::env::consts::OS),
        current_version,
        arch: std::env::consts::ARCH.to_owned(),
        os_build: None,
        trusted_keys,
    }
}

/// HTTPS obligatorio salvo en el propio equipo (`localhost`, `127.0.0.1`, `[::1]`), para pruebas.
pub(crate) fn check_url(url: &str) -> Result<bool> {
    if let Some(rest) = url.strip_prefix("https://") {
        if rest.is_empty() {
            return Err(UpdateError::InsecureUrl(url.to_owned()));
        }
        return Ok(false);
    }
    if let Some(rest) = url.strip_prefix("http://") {
        let host = rest.split(['/', '?']).next().unwrap_or("");
        let host = if host.starts_with('[') {
            host.split(']').next().map(|h| format!("{h}]")).unwrap_or_default()
        } else {
            host.split(':').next().unwrap_or("").to_owned()
        };
        if matches!(host.as_str(), "localhost" | "127.0.0.1" | "[::1]") {
            return Ok(true);
        }
    }
    Err(UpdateError::InsecureUrl(url.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v() -> Version {
        Version::new(0, 1, 0)
    }

    #[test]
    fn policy_beats_env_and_user() {
        let policy = Policy {
            url: Some("https://empresa.local/pliego/".into()),
            channel: Some("stable".into()),
            ..Policy::default()
        };
        let user = UserSettings {
            mode: UpdateMode::NotifyOnly,
            channel: Some("beta".into()),
        };
        let c = resolve(
            Some("https://oficial"),
            Some("http://127.0.0.1:8787"),
            &user,
            &policy,
            v(),
            vec![],
        );
        assert_eq!(c.base_url.as_deref(), Some("https://empresa.local/pliego"));
        assert_eq!(c.channel, "stable");
        assert_eq!(c.mode, UpdateMode::NotifyOnly);
        assert!(c.managed);
    }

    #[test]
    fn env_beats_build_and_disabled_policy_wins() {
        let c = resolve(
            Some("https://oficial"),
            Some(" http://127.0.0.1:8787 "),
            &UserSettings::default(),
            &Policy::default(),
            v(),
            vec![],
        );
        assert_eq!(c.base_url.as_deref(), Some("http://127.0.0.1:8787"));
        assert!(!c.managed);
        let off = Policy {
            disabled: true,
            ..Policy::default()
        };
        assert_eq!(
            resolve(
                Some("https://oficial"),
                None,
                &UserSettings::default(),
                &off,
                v(),
                vec![]
            )
            .mode,
            UpdateMode::Disabled
        );
    }

    #[test]
    fn no_url_or_no_keys_means_disabled() {
        let c = resolve(None, None, &UserSettings::default(), &Policy::default(), v(), vec![]);
        assert!(!c.enabled());
    }

    #[test]
    fn https_required_except_loopback() {
        assert!(!check_url("https://actualizaciones.ejemplo.es").unwrap());
        assert!(check_url("http://127.0.0.1:8787/x").unwrap());
        assert!(check_url("http://localhost").unwrap());
        assert!(check_url("http://[::1]:9000").unwrap());
        assert!(check_url("http://ejemplo.es").is_err());
        assert!(check_url("http://127.0.0.1.ejemplo.es").is_err());
        assert!(check_url("ftp://x").is_err());
    }
}
