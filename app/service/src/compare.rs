//! Comparar dos documentos desde la app (flujo W24, `compare.compare`): guarda el informe junto al primero.
//!
//! La lógica vive aquí, sin Tauri, para probarla en Linux; `app/src-tauri/src/commands/compare.rs` solo traduce.
//! No modifica ninguno de los dos documentos: el informe es un PDF nuevo `<primero> (<etiqueta>).pdf`.

use serde::{Deserialize, Serialize};
use std::path::Path;

use pliego_appdata::copyjob::unique_sibling;
use pliego_compare::CompareOptions;
pub use pliego_compare::Comparison;
use pliego_core::{Error, JobContext, Report, Result};

/// Lo que pide el panel «Comparar».
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompareRequest {
    /// Versión nueva (la que se compara con la abierta).
    pub other: std::path::PathBuf,
    pub password_a: Option<String>,
    pub password_b: Option<String>,
    /// Comparar el texto palabra por palabra.
    pub text: bool,
    /// Comparar las páginas por píxeles.
    pub visual: bool,
    /// Etiqueta del nombre del informe, ya traducida («comparación»).
    pub label: String,
}

/// Una línea legible con lo que cambió, para el centro de tareas.
pub fn describe(c: &Comparison) -> String {
    if c.is_identical() {
        return "Sin diferencias".into();
    }
    let s = &c.summary;
    format!(
        "{} página(s) con cambios, {} insertada(s), {} eliminada(s); palabras +{} −{} ~{}",
        s.changed_pages, s.inserted_pages, s.deleted_pages, s.added_words, s.removed_words, s.changed_words
    )
}

/// Compara `first` con `req.other` sin escribir nada: lo que enseña la vista lado a lado (páginas emparejadas, cambios de
/// texto con su posición y regiones distintas). `req.label` no se usa.
pub fn compare_view(first: &Path, req: &CompareRequest, ctx: &JobContext) -> Result<Comparison> {
    if !req.text && !req.visual {
        return Err(Error::InvalidInput(
            "elige comparar el texto, el aspecto o ambos".into(),
        ));
    }
    if same_file(first, &req.other) {
        return Err(Error::InvalidInput("elige un documento distinto del abierto".into()));
    }
    let opts = CompareOptions {
        text: req.text,
        visual: req.visual,
        password_a: req.password_a.clone(),
        password_b: req.password_b.clone(),
        ..CompareOptions::default()
    };
    pliego_compare::compare(first, &req.other, &opts, ctx)
}

/// Compara `first` con `req.other` y guarda el informe PDF junto a `first`.
pub fn compare_copy(first: &Path, req: &CompareRequest, ctx: &JobContext) -> Result<Report> {
    let result = compare_view(first, req, ctx)?;
    let out = unique_sibling(first, &req.label, "pdf");
    if let Err(e) = pliego_compare::write_report_with(&result, &out, ctx) {
        let _ = std::fs::remove_file(&out);
        return Err(e);
    }
    Ok(Report::with_output(out, describe(&result)))
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn corpus(rel: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/corpus")
            .join(rel)
    }

    fn request(other: &Path) -> CompareRequest {
        CompareRequest {
            other: other.to_path_buf(),
            password_a: None,
            password_b: None,
            text: true,
            visual: false,
            label: "comparación".into(),
        }
    }

    #[test]
    fn compares_two_versions_and_writes_a_report_beside_the_first() {
        let _serial = crate::test_support::serial();
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.pdf");
        let b = dir.path().join("b.pdf");
        std::fs::copy(corpus("organize/a.pdf"), &a).unwrap();
        std::fs::copy(corpus("organize/b.pdf"), &b).unwrap();
        let (before_a, before_b) = (std::fs::read(&a).unwrap(), std::fs::read(&b).unwrap());
        let r = compare_copy(&a, &request(&b), &JobContext::silent()).unwrap();
        assert_eq!(r.outputs, vec![dir.path().join("a (comparación).pdf")]);
        assert!(std::fs::metadata(&r.outputs[0]).unwrap().len() > 0);
        assert!(!r.summary.is_empty());
        assert_eq!(std::fs::read(&a).unwrap(), before_a);
        assert_eq!(std::fs::read(&b).unwrap(), before_b);
        // Una segunda comparación no pisa el informe anterior.
        let r2 = compare_copy(&a, &request(&b), &JobContext::silent()).unwrap();
        assert_ne!(r.outputs, r2.outputs);
    }

    #[test]
    fn view_returns_the_comparison_and_writes_nothing() {
        let _serial = crate::test_support::serial();
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.pdf");
        let b = dir.path().join("b.pdf");
        std::fs::copy(corpus("organize/a.pdf"), &a).unwrap();
        std::fs::copy(corpus("organize/b.pdf"), &b).unwrap();
        let c = compare_view(&a, &request(&b), &JobContext::silent()).unwrap();
        assert_eq!(
            c.summary.pages_a as usize + c.summary.inserted_pages as usize - c.summary.deleted_pages as usize,
            c.summary.pages_b as usize
        );
        assert!(!c.pages.is_empty());
        // Se serializa tal cual para la interfaz.
        let json = serde_json::to_value(&c).unwrap();
        assert!(json["pages"].is_array());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
        let mut none = request(&b);
        none.text = false;
        assert!(matches!(
            compare_view(&a, &none, &JobContext::silent()),
            Err(Error::InvalidInput(_))
        ));
    }

    #[test]
    fn identical_documents_say_so() {
        let _serial = crate::test_support::serial();
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.pdf");
        let b = dir.path().join("copia.pdf");
        std::fs::copy(corpus("basic/hola.pdf"), &a).unwrap();
        std::fs::copy(corpus("basic/hola.pdf"), &b).unwrap();
        let r = compare_copy(&a, &request(&b), &JobContext::silent()).unwrap();
        assert_eq!(r.summary, "Sin diferencias");
    }

    #[test]
    fn rejects_bad_requests_without_creating_anything() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.pdf");
        std::fs::copy(corpus("basic/hola.pdf"), &a).unwrap();
        let ctx = JobContext::silent();
        let mut none = request(&a);
        none.text = false;
        assert!(matches!(compare_copy(&a, &none, &ctx), Err(Error::InvalidInput(_))));
        assert!(matches!(
            compare_copy(&a, &request(&a), &ctx),
            Err(Error::InvalidInput(_))
        ));
        assert!(compare_copy(&a, &request(&dir.path().join("no-existe.pdf")), &ctx).is_err());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}
