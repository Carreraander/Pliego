//! Acciones en lote desde la app (flujo W29, `batch.actions`): aplica una acción guardada (`.toml` o `.json`) a
//! archivos y carpetas de PDF, con los mismos pasos que `pliego run` (`pliego-steps`).
//!
//! La lógica vive aquí, sin Tauri, para probarla en Linux; `app/src-tauri/src/commands/actions.rs` solo traduce.
//! Los originales no se tocan: los resultados van a una carpeta de salida (por defecto `<primera entrada> (lote)`,
//! junto a ella y con nombre libre) y nunca puede ser la de un original.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub use pliego_actions::Action;
use pliego_actions::{RunOptions, collect_inputs};
use pliego_appdata::copyjob::unique_in;
use pliego_core::{Error, JobContext, Report, Result};

/// Lo que pide el asistente de acciones en lote.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BatchRequest {
    /// Acción guardada (`.toml` o `.json`).
    pub action: PathBuf,
    /// PDF o carpetas (se recorren las subcarpetas).
    pub inputs: Vec<PathBuf>,
    /// Carpeta de salida; si falta se crea `<primera entrada> (<suffix>)` junto a la primera entrada.
    pub output: Option<PathBuf>,
    /// Retomar un lote anterior en la misma carpeta de salida.
    pub resume: bool,
    /// Etiqueta de la carpeta de salida por defecto, ya traducida («lote»).
    pub suffix: String,
}

/// Pasos disponibles (para el asistente).
pub fn step_kinds() -> Vec<String> {
    pliego_steps::builtin_registry().kinds().map(str::to_owned).collect()
}

/// Lee y valida una acción guardada (el asistente la muestra antes de ejecutar).
pub fn load_action(path: &Path) -> Result<Action> {
    let action = Action::load(path)?;
    let registry = pliego_steps::builtin_registry();
    for (i, step) in action.steps.iter().enumerate() {
        registry.get(&step.kind).map_err(|_| {
            Error::InvalidInput(format!(
                "paso {} ({}): este paso no está disponible en la app",
                i + 1,
                step.kind
            ))
        })?;
    }
    Ok(action)
}

/// Carpeta de salida por defecto para `first` (la primera entrada).
pub fn default_output(first: &Path, suffix: &str) -> PathBuf {
    let dir = first
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let stem = first
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "lote".into());
    // Si la entrada es una carpeta, la salida va a su lado, no dentro.
    if first.is_dir() {
        let parent = first
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let name = first
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "lote".into());
        return unique_in(parent, &name, suffix, "");
    }
    unique_in(dir, &stem, suffix, "")
}

/// Ejecuta el lote. El informe lleva las rutas de salida y un aviso por cada archivo que falló.
pub fn run_batch(req: &BatchRequest, ctx: &JobContext) -> Result<Report> {
    let first = req
        .inputs
        .first()
        .ok_or_else(|| Error::InvalidInput("indica al menos un PDF o carpeta de entrada".into()))?;
    let action = load_action(&req.action)?;
    let output_dir = match &req.output {
        Some(o) => o.clone(),
        None => default_output(first, &req.suffix),
    };
    let inputs = collect_inputs(&req.inputs, Some(&output_dir))?;
    if inputs.is_empty() {
        return Err(Error::InvalidInput(
            "no hay ningún PDF en las entradas indicadas".into(),
        ));
    }
    let registry = pliego_steps::builtin_registry();
    let run = pliego_actions::run(
        &action,
        &registry,
        &inputs,
        &RunOptions {
            output_dir: output_dir.clone(),
            resume: req.resume,
        },
        ctx,
    )?;
    let mut report = run.report;
    if report.summary.is_empty() {
        report.summary = format!("{} procesados, {} con error", run.processed, run.failures.len());
    }
    // El centro de tareas abre el primer PDF de `outputs`; la carpeta completa se indica en el resumen.
    report.summary = format!("{} (en {})", report.summary, output_dir.display());
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn action_file(dir: &Path, body: &str) -> PathBuf {
        let p = dir.join("accion.toml");
        std::fs::write(&p, body).unwrap();
        p
    }

    #[test]
    fn lists_the_shared_steps() {
        let kinds = step_kinds();
        assert!(kinds.iter().any(|k| k == "stamp.bates"));
        assert!(kinds.iter().any(|k| k == "optimize"));
    }

    #[test]
    fn rejects_unknown_steps_and_empty_inputs() {
        let dir = tempfile::tempdir().unwrap();
        let bad = action_file(dir.path(), "[[steps]]\nkind = \"no.existe\"\n");
        assert!(load_action(&bad).is_err());
        let err = run_batch(
            &BatchRequest {
                action: bad,
                ..Default::default()
            },
            &JobContext::silent(),
        )
        .unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)));
    }

    #[test]
    fn default_output_is_free_and_next_to_the_input() {
        let dir = tempfile::tempdir().unwrap();
        let pdf = dir.path().join("a.pdf");
        std::fs::write(&pdf, b"x").unwrap();
        assert_eq!(default_output(&pdf, "lote"), dir.path().join("a (lote)"));
        std::fs::create_dir(dir.path().join("a (lote)")).unwrap();
        assert_eq!(default_output(&pdf, "lote"), dir.path().join("a (lote 2)"));
        let sub = dir.path().join("facturas");
        std::fs::create_dir(&sub).unwrap();
        assert_eq!(default_output(&sub, "lote"), dir.path().join("facturas (lote)"));
    }

    #[test]
    fn empty_folder_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let act = action_file(dir.path(), "[[steps]]\nkind = \"pages.delete\"\npages = \"1\"\n");
        let docs = dir.path().join("docs");
        std::fs::create_dir(&docs).unwrap();
        let err = run_batch(
            &BatchRequest {
                action: act,
                inputs: vec![docs],
                suffix: "lote".into(),
                ..Default::default()
            },
            &JobContext::silent(),
        )
        .unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)));
    }
}
