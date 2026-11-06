//! Proteger y redactar desde la app (W25): lógica de los comandos Tauri, sin Tauri, para poder probarla con rutas.
//!
//! Cada operación escribe una **copia** junto al original (`<nombre> (protegido).pdf`…); nunca toca el original.

use std::path::{Path, PathBuf};

use pliego_appdata::copyjob::unique_sibling;
use pliego_core::{Error, JobContext, Report, Result};
use pliego_protect::{Cipher, Encryption, Permissions, SanitizeOptions};
use pliego_redact::{Area, Match, Pattern, RedactJob, RedactOptions};
use serde::{Deserialize, Serialize};

/// Ruta de la copia (`<nombre> (<sufijo>).pdf`, con contador si ya existe): el ayudante común de la ola 4.
pub fn copy_path(input: &Path, suffix: &str) -> PathBuf {
    unique_sibling(input, suffix, "pdf")
}

fn opt(pw: &Option<String>) -> Option<&str> {
    pw.as_deref().filter(|p| !p.is_empty())
}

/// Petición de «Proteger»: cifrar con contraseña y/o permisos.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtectRequest {
    pub path: PathBuf,
    /// Contraseña con la que se abrió el documento, si estaba cifrado.
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub user_password: String,
    #[serde(default)]
    pub owner_password: String,
    #[serde(default)]
    pub cipher: Cipher,
    #[serde(default)]
    pub permissions: Permissions,
    /// Sufijo del nombre de la copia (ya traducido); por defecto, «protegido».
    #[serde(default)]
    pub suffix: Option<String>,
}

/// Cifra una copia del documento.
pub fn protect(req: &ProtectRequest, ctx: &JobContext<'_>) -> Result<Report> {
    ctx.step(0, 1, "Cifrando…")?;
    let out = copy_path(&req.path, req.suffix.as_deref().unwrap_or("protegido"));
    let enc = Encryption {
        cipher: req.cipher,
        user_password: req.user_password.clone(),
        owner_password: req.owner_password.clone(),
        permissions: req.permissions,
    };
    let report = pliego_protect::encrypt(&req.path, &out, opt(&req.password), &enc)?;
    ctx.step(1, 1, "Listo")?;
    Ok(report)
}

/// Quita la protección de una copia (hace falta la contraseña).
/// `suffix` es el sufijo del nombre de la copia, ya traducido (por defecto, «sin protección»).
pub fn unprotect(path: &Path, password: &str, suffix: Option<&str>, ctx: &JobContext<'_>) -> Result<Report> {
    if password.is_empty() {
        return Err(Error::InvalidInput("indica la contraseña del documento".into()));
    }
    ctx.step(0, 1, "Quitando la protección…")?;
    let out = copy_path(path, suffix.unwrap_or("sin protección"));
    let report = pliego_protect::decrypt(path, &out, password)?;
    ctx.step(1, 1, "Listo")?;
    Ok(report)
}

/// Petición de «Sanear documento».
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SanitizeRequest {
    pub path: PathBuf,
    #[serde(default)]
    pub password: Option<String>,
    pub options: SanitizeOptions,
    /// Sufijo del nombre de la copia (ya traducido); por defecto, «saneado».
    #[serde(default)]
    pub suffix: Option<String>,
}

pub fn sanitize(req: &SanitizeRequest, ctx: &JobContext<'_>) -> Result<Report> {
    let out = copy_path(&req.path, req.suffix.as_deref().unwrap_or("saneado"));
    pliego_protect::sanitize(&req.path, &out, opt(&req.password), &req.options, ctx)
}

/// Qué patrones de datos personales buscar, tal como los nombra la interfaz.
pub fn parse_patterns(names: &[String]) -> Result<Vec<Pattern>> {
    names
        .iter()
        .map(|n| n.parse::<Pattern>().map_err(Error::InvalidInput))
        .collect()
}

/// Una coincidencia lista para mostrar en la lista de revisión.
#[derive(Debug, Clone, Serialize)]
pub struct FoundMatch {
    pub page: u32,
    pub pattern: String,
    pub text: String,
    pub areas: Vec<Area>,
}

impl From<Match> for FoundMatch {
    fn from(m: Match) -> Self {
        Self {
            page: m.page,
            pattern: m.pattern,
            text: m.text,
            areas: m.areas,
        }
    }
}

/// Busca datos personales para la vista previa, antes de aplicar nada.
pub fn find_matches(path: &Path, password: Option<&str>, patterns: &[String]) -> Result<Vec<FoundMatch>> {
    let patterns = parse_patterns(patterns)?;
    if patterns.is_empty() {
        return Ok(Vec::new());
    }
    Ok(pliego_redact::find_in_file(path, password, &patterns)?
        .into_iter()
        .map(FoundMatch::from)
        .collect())
}

/// Petición de «Redactar»: zonas marcadas a mano y/o patrones.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedactRequest {
    pub path: PathBuf,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub areas: Vec<Area>,
    #[serde(default)]
    pub patterns: Vec<String>,
    #[serde(default)]
    pub options: RedactOptions,
    /// Sufijo del nombre de la copia (ya traducido); por defecto, «redactado».
    #[serde(default)]
    pub suffix: Option<String>,
}

/// Redacta una copia y verifica que el texto ya no se puede extraer.
pub fn redact(req: &RedactRequest, ctx: &JobContext<'_>) -> Result<Report> {
    let job = RedactJob {
        areas: req.areas.clone(),
        patterns: parse_patterns(&req.patterns)?,
        options: req.options,
    };
    let out = copy_path(&req.path, req.suffix.as_deref().unwrap_or("redactado"));
    pliego_redact::apply(&req.path, &out, opt(&req.password), &job, ctx)
}
