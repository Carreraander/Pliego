//! Organizar y combinar desde la app (flujo W23): cada operación guarda una **copia** junto al original.
//!
//! La lógica vive aquí, sin Tauri, para probarla en Linux; `app/src-tauri/src/commands/organize.rs` solo traduce.

use std::path::{Path, PathBuf};

use pliego_appdata::copyjob::{unique_in, unique_sibling};
use pliego_core::{Error, JobContext, Report, Result};
pub use pliego_organize::{LabelStyle, MergeOptions, PageLabelSpec, SplitRule};

/// Combina `inputs` en `<primero> (<etiqueta>).pdf`.
pub fn merge_copy(inputs: &[PathBuf], options: &MergeOptions, suffix: &str, ctx: &JobContext) -> Result<Report> {
    let first = inputs
        .first()
        .ok_or_else(|| Error::InvalidInput("no hay archivos que combinar".into()))?;
    let out = unique_sibling(first, suffix, "pdf");
    pliego_organize::merge(inputs, &out, options, ctx)
}

/// Divide `input` en la carpeta `<nombre> (<etiqueta>)` junto a él.
pub fn split_copy(
    input: &Path,
    password: Option<&str>,
    rule: &SplitRule,
    suffix: &str,
    ctx: &JobContext,
) -> Result<Report> {
    let doc = pliego_organize::open(input, password)?;
    let stem = input
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "documento".into());
    let dir = input.parent().map(PathBuf::from).unwrap_or_default();
    pliego_organize::split(&doc, &stem, rule, &unique_in(&dir, &stem, suffix, ""), ctx)
}

/// De dónde salen las páginas que se insertan.
#[derive(Debug, Clone, PartialEq)]
pub enum InsertSource {
    /// `count` páginas en blanco, del tamaño de la vecina.
    Blank { count: u32 },
    /// Páginas de otro PDF (`pages` desde 0; vacío = todas).
    File {
        path: PathBuf,
        password: Option<String>,
        pages: Vec<u32>,
    },
}

/// Inserta páginas antes de `at` (desde 0; `at` = nº de páginas añade al final) y guarda una copia.
pub fn insert_copy(
    input: &Path,
    password: Option<&str>,
    source: &InsertSource,
    at: u32,
    suffix: &str,
) -> Result<Report> {
    let mut doc = pliego_organize::open(input, password)?;
    let before = pliego_organize::page_count(&doc)?;
    match source {
        InsertSource::Blank { count } => {
            if *count == 0 {
                return Err(Error::InvalidInput("indica cuántas páginas en blanco".into()));
            }
            pliego_organize::insert_blank(&mut doc, at, *count, None)?;
        }
        InsertSource::File { path, password, pages } => {
            let src = pliego_organize::open(path, password.as_deref())?;
            let all: Vec<u32>;
            let pages = if pages.is_empty() {
                all = (0..pliego_organize::page_count(&src)?).collect();
                &all
            } else {
                pages
            };
            pliego_organize::insert_from(&mut doc, &src, pages, at)?;
        }
    }
    let added = pliego_organize::page_count(&doc)? - before;
    let out = unique_sibling(input, suffix, "pdf");
    pliego_organize::save(&doc, &out)?;
    Ok(Report::with_output(out, format!("{added} páginas insertadas")))
}

/// Pone la numeración de páginas (etiquetas del PDF: 1, 2, 3 / i, ii, iii / A-1…) y guarda una copia.
pub fn number_copy(input: &Path, password: Option<&str>, rules: &[PageLabelSpec], suffix: &str) -> Result<Report> {
    if rules.is_empty() {
        return Err(Error::InvalidInput("no hay reglas de numeración".into()));
    }
    let mut doc = pliego_organize::open(input, password)?;
    let n = pliego_organize::page_count(&doc)?;
    if let Some(bad) = rules.iter().find(|r| r.start_page >= n) {
        return Err(Error::InvalidInput(format!(
            "la página {} no existe (el documento tiene {n})",
            bad.start_page + 1
        )));
    }
    pliego_organize::set_page_labels(&mut doc, rules)?;
    let out = unique_sibling(input, suffix, "pdf");
    pliego_organize::save(&doc, &out)?;
    Ok(Report::with_output(
        out,
        format!("{} reglas de numeración", rules.len()),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Option<PathBuf> {
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/basic/hola.pdf");
        p.exists().then_some(p)
    }

    fn setup() -> Option<(tempfile::TempDir, PathBuf)> {
        let src = sample()?;
        let dir = tempfile::tempdir().ok()?;
        let input = dir.path().join("doc.pdf");
        std::fs::copy(src, &input).ok()?;
        Some((dir, input))
    }

    fn pages(p: &Path) -> u32 {
        pliego_organize::page_count(&pliego_organize::open(p, None).unwrap()).unwrap()
    }

    #[test]
    fn insert_blank_saves_a_copy_and_keeps_the_original() {
        let Some((dir, input)) = setup() else { return };
        let before = std::fs::read(&input).unwrap();
        let n = pages(&input);
        let r = insert_copy(&input, None, &InsertSource::Blank { count: 2 }, 0, "insertado").unwrap();
        assert_eq!(r.outputs, vec![dir.path().join("doc (insertado).pdf")]);
        assert_eq!(pages(&r.outputs[0]), n + 2);
        assert_eq!(std::fs::read(&input).unwrap(), before);
    }

    #[test]
    fn insert_from_file_defaults_to_all_pages() {
        let Some((dir, input)) = setup() else { return };
        let n = pages(&input);
        let r = insert_copy(
            &input,
            None,
            &InsertSource::File {
                path: input.clone(),
                password: None,
                pages: vec![],
            },
            n,
            "insertado",
        )
        .unwrap();
        assert_eq!(pages(&r.outputs[0]), n * 2);
        assert!(dir.path().join("doc (insertado).pdf").exists());
    }

    #[test]
    fn insert_rejects_bad_position_and_empty_blank() {
        let Some((_dir, input)) = setup() else { return };
        let n = pages(&input);
        assert!(insert_copy(&input, None, &InsertSource::Blank { count: 1 }, n + 5, "x").is_err());
        assert!(insert_copy(&input, None, &InsertSource::Blank { count: 0 }, 0, "x").is_err());
    }

    #[test]
    fn number_pages_writes_a_copy() {
        let Some((dir, input)) = setup() else { return };
        let rule = PageLabelSpec {
            start_page: 0,
            style: LabelStyle::LowerRoman,
            prefix: String::new(),
            first_number: 1,
        };
        let r = number_copy(&input, None, std::slice::from_ref(&rule), "numerado").unwrap();
        assert_eq!(r.outputs, vec![dir.path().join("doc (numerado).pdf")]);
        let bad = PageLabelSpec {
            start_page: 999,
            ..rule
        };
        assert!(number_copy(&input, None, &[bad], "numerado").is_err());
        assert!(number_copy(&input, None, &[], "numerado").is_err());
    }

    #[test]
    fn split_and_merge_round_trip() {
        let Some((dir, input)) = setup() else { return };
        let ctx = JobContext::silent();
        let r = split_copy(&input, None, &SplitRule::EveryN { n: 1 }, "partes", &ctx).unwrap();
        assert!(!r.outputs.is_empty());
        assert!(r.outputs[0].starts_with(dir.path().join("doc (partes)")));
        let m = merge_copy(&r.outputs, &MergeOptions::default(), "combinado", &ctx).unwrap();
        assert_eq!(pages(&m.outputs[0]), pages(&input));
        assert!(merge_copy(&[], &MergeOptions::default(), "combinado", &ctx).is_err());
    }
}
