//! Proteger y redactar desde la app (W25): lógica de los comandos Tauri, sin Tauri, para poder probarla con rutas.
//!
//! Cada operación escribe una **copia** junto al original (`<nombre> (protegido).pdf`…); nunca toca el original.

use std::path::{Path, PathBuf};

use pliego_core::{Error, JobContext, Report, Result};
use pliego_protect::{Cipher, Encryption, Permissions, SanitizeOptions};
use pliego_redact::{Area, Match, Pattern, RedactJob, RedactOptions};
use serde::{Deserialize, Serialize};

/// Ruta de la copia: `<carpeta>/<nombre> (<sufijo>).pdf`, o con ` 2`, ` 3`… si ya existe.
pub fn copy_path(input: &Path, suffix: &str) -> PathBuf {
    let dir = input.parent().unwrap_or(Path::new("."));
    let stem = input
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "documento".into());
    let first = dir.join(format!("{stem} ({suffix}).pdf"));
    if !first.exists() {
        return first;
    }
    (2..)
        .map(|n| dir.join(format!("{stem} ({suffix} {n}).pdf")))
        .find(|p| !p.exists())
        .expect("rango infinito")
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
}

/// Cifra una copia del documento.
pub fn protect(req: &ProtectRequest, ctx: &JobContext<'_>) -> Result<Report> {
    ctx.step(0, 1, "Cifrando…")?;
    let out = copy_path(&req.path, "protegido");
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
pub fn unprotect(path: &Path, password: &str, ctx: &JobContext<'_>) -> Result<Report> {
    if password.is_empty() {
        return Err(Error::InvalidInput("indica la contraseña del documento".into()));
    }
    ctx.step(0, 1, "Quitando la protección…")?;
    let out = copy_path(path, "sin protección");
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
}

pub fn sanitize(req: &SanitizeRequest, ctx: &JobContext<'_>) -> Result<Report> {
    let out = copy_path(&req.path, "saneado");
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
}

/// Redacta una copia y verifica que el texto ya no se puede extraer.
pub fn redact(req: &RedactRequest, ctx: &JobContext<'_>) -> Result<Report> {
    let job = RedactJob {
        areas: req.areas.clone(),
        patterns: parse_patterns(&req.patterns)?,
        options: req.options,
    };
    let out = copy_path(&req.path, "redactado");
    pliego_redact::apply(&req.path, &out, opt(&req.password), &job, ctx)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn corpus(rel: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/corpus")
            .join(rel)
    }

    /// Copia el PDF a una carpeta temporal para que las copias de salida no ensucien el repositorio.
    fn temp_copy(rel: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(Path::new(rel).file_name().unwrap());
        std::fs::copy(corpus(rel), &path).unwrap();
        (dir, path)
    }

    #[test]
    fn copy_path_evita_colisiones() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("contrato.pdf");
        assert_eq!(
            copy_path(&input, "protegido"),
            dir.path().join("contrato (protegido).pdf")
        );
        std::fs::write(dir.path().join("contrato (protegido).pdf"), b"x").unwrap();
        assert_eq!(
            copy_path(&input, "protegido"),
            dir.path().join("contrato (protegido 2).pdf")
        );
    }

    #[test]
    fn protect_cifra_una_copia_y_respeta_el_original() {
        let (_d, path) = temp_copy("basic/hola.pdf");
        let before = std::fs::read(&path).unwrap();
        let req: ProtectRequest = serde_json::from_value(serde_json::json!({
            "path": path, "userPassword": "abre", "ownerPassword": "manda",
            "permissions": { "print": true, "print_high_quality": true, "copy": false, "modify": false,
                "annotate": true, "fill_forms": true, "accessibility": true, "assemble": true },
        }))
        .unwrap();
        let rep = protect(&req, &JobContext::silent()).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), before, "el original no cambia");
        let out = &rep.outputs[0];
        assert!(out.file_name().unwrap().to_string_lossy().contains("(protegido)"));
        assert!(pliego_protect::permissions_of(out, None).is_err(), "pide contraseña");
        let perms = pliego_protect::permissions_of(out, Some("abre")).unwrap();
        assert!(!perms.copy && !perms.modify && perms.print);
    }

    #[test]
    fn protect_sin_contrasenas_falla() {
        let (_d, path) = temp_copy("basic/hola.pdf");
        let req: ProtectRequest = serde_json::from_value(serde_json::json!({ "path": path })).unwrap();
        assert!(matches!(
            protect(&req, &JobContext::silent()),
            Err(Error::InvalidInput(_))
        ));
    }

    #[test]
    fn unprotect_quita_la_proteccion() {
        let (_d, path) = temp_copy("basic/hola.pdf");
        let req: ProtectRequest =
            serde_json::from_value(serde_json::json!({ "path": path, "userPassword": "abre" })).unwrap();
        let enc = protect(&req, &JobContext::silent()).unwrap().outputs.remove(0);
        let rep = unprotect(&enc, "abre", &JobContext::silent()).unwrap();
        assert!(
            pliego_protect::permissions_of(&rep.outputs[0], None).is_ok(),
            "ya no pide contraseña"
        );
        assert!(unprotect(&enc, "", &JobContext::silent()).is_err());
    }

    #[test]
    fn sanitize_deja_informe() {
        let (_d, path) = temp_copy("protect/sucio.pdf");
        let req = SanitizeRequest {
            path,
            password: None,
            options: SanitizeOptions::default(),
        };
        let rep = sanitize(&req, &JobContext::silent()).unwrap();
        assert!(rep.summary.starts_with("Eliminado:"), "{}", rep.summary);
        assert!(rep.outputs[0].exists());
    }

    #[test]
    fn busca_y_redacta_datos_personales() {
        let (_d, path) = temp_copy("redact/datos.pdf");
        let found = find_matches(&path, None, &["dni".into(), "iban".into()]).unwrap();
        assert!(!found.is_empty());
        assert!(found.iter().all(|m| !m.areas.is_empty()));
        assert!(find_matches(&path, None, &["nada".into()]).is_err());
        assert!(find_matches(&path, None, &[]).unwrap().is_empty());

        let req = RedactRequest {
            path: path.clone(),
            password: None,
            areas: vec![],
            patterns: vec!["dni".into()],
            options: RedactOptions::default(),
        };
        let rep = redact(&req, &JobContext::silent()).unwrap();
        let out = &rep.outputs[0];
        assert!(out.file_name().unwrap().to_string_lossy().contains("(redactado)"));
        assert!(
            find_matches(out, None, &["dni".into()]).unwrap().is_empty(),
            "el DNI ya no se puede extraer"
        );
    }
}
