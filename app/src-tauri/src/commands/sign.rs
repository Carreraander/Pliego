//! Firmas (W28): firma digital, validación y flujo de firmas. La lógica vive en `pliego_app_service::sign`; aquí solo
//! se traducen argumentos. Firmar y preparar el flujo son trabajos que **guardan una copia** junto al original
//! (`pliego_appdata::copyjob`) y devuelven el `JobId`; el centro de tareas ofrece «Abrir resultado».

use std::path::PathBuf;

use pliego_app_service::api::ApiResult;
use pliego_app_service::sign::{self, CertInfo, KeySource};
use pliego_appdata::copyjob::unique_sibling;
use pliego_appdata::jobs::JobId;
use pliego_sign::{SignLang, SignOptions, SignatureReport};
use pliego_signflow::{AuditEntry, PendingSigner, SignerSpec};
use tauri::State;

use crate::commands::jobs::JobsState;

/// Certificados del almacén personal de Windows (vacío fuera de Windows).
#[tauri::command]
pub fn sign_list_certificates() -> ApiResult<Vec<CertInfo>> {
    Ok(sign::list_certificates()?)
}

/// Firma `path` y guarda `<nombre> (firmado).pdf` junto al original.
#[tauri::command]
pub fn sign_pdf(
    jobs: State<'_, JobsState>,
    path: String,
    key: KeySource,
    options: SignOptions,
    suffix: Option<String>,
) -> JobId {
    let src = PathBuf::from(path);
    jobs.0.spawn(move |ctx| {
        let out = unique_sibling(&src, suffix.as_deref().unwrap_or("firmado"), "pdf");
        sign::sign_pdf(&src, &out, &key, options, ctx)
    })
}

/// Firmas del documento con su estado.
#[tauri::command]
pub fn sign_validate(path: String) -> ApiResult<Vec<SignatureReport>> {
    Ok(sign::validate(&PathBuf::from(path))?)
}

/// Prepara `path` para que lo firmen `signers` en orden: `<nombre> (para firmar).pdf`.
#[tauri::command]
pub fn signflow_prepare(
    jobs: State<'_, JobsState>,
    path: String,
    signers: Vec<SignerSpec>,
    lang: Option<SignLang>,
    suffix: Option<String>,
) -> JobId {
    let src = PathBuf::from(path);
    jobs.0.spawn(move |ctx| {
        let out = unique_sibling(&src, suffix.as_deref().unwrap_or("para firmar"), "pdf");
        sign::flow_prepare(&src, &out, signers, lang.unwrap_or_default(), ctx)
    })
}

/// A quién le toca firmar el paquete (`null` si ya firmaron todos).
#[tauri::command]
pub fn signflow_next(path: String) -> ApiResult<Option<PendingSigner>> {
    Ok(sign::flow_next(&PathBuf::from(path))?)
}

/// Añade la firma del siguiente firmante: `<nombre> (firmado).pdf`.
#[tauri::command]
pub fn signflow_sign(
    jobs: State<'_, JobsState>,
    path: String,
    key: KeySource,
    options: SignOptions,
    suffix: Option<String>,
) -> JobId {
    let src = PathBuf::from(path);
    jobs.0.spawn(move |ctx| {
        let out = unique_sibling(&src, suffix.as_deref().unwrap_or("firmado"), "pdf");
        sign::flow_sign(&src, &out, &key, options, ctx)
    })
}

/// Registro de auditoría del paquete.
#[tauri::command]
pub fn signflow_audit(path: String) -> ApiResult<Vec<AuditEntry>> {
    Ok(sign::flow_audit(&PathBuf::from(path))?)
}
