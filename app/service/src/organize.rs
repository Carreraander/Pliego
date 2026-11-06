//! Organizar y combinar desde la app (flujo W23): cada operación guarda una **copia** junto al original.
//!
//! La lógica vive aquí, sin Tauri, para probarla en Linux; `app/src-tauri/src/commands/organize.rs` solo traduce.

use std::path::{Path, PathBuf};

use pliego_appdata::copyjob::{unique_in, unique_sibling};
use pliego_core::{Error, JobContext, Report, Result};
use pliego_organize::{LabelStyle, PageLabelSpec};
pub use pliego_organize::{MergeOptions, SplitRule};
use pliego_stamp::{HeaderFooter, StampContext, TextStyle};
use serde::{Deserialize, Serialize};

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

/// Una página del resultado: la página `page` del original (desde 0) con un giro extra de `rotate` grados.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageEdit {
    pub page: u32,
    #[serde(default)]
    pub rotate: i32,
}

/// Construye un documento con las páginas de `edits` en ese orden (reordenar, borrar = no incluir, duplicar = repetir,
/// girar, extraer) y guarda una copia `<nombre> (<etiqueta>).pdf`. El original no cambia.
pub fn pages_copy(
    input: &Path,
    password: Option<&str>,
    edits: &[PageEdit],
    suffix: &str,
    ctx: &JobContext,
) -> Result<Report> {
    if edits.is_empty() {
        return Err(Error::InvalidInput("no queda ninguna página".into()));
    }
    if let Some(bad) = edits.iter().find(|e| e.rotate % 90 != 0) {
        return Err(Error::InvalidInput(format!("giro no válido: {}", bad.rotate)));
    }
    let src = pliego_organize::open(input, password)?;
    ctx.step(0, 2, "Reorganizando páginas")?;
    let order: Vec<u32> = edits.iter().map(|e| e.page).collect();
    let mut doc = pliego_organize::extract(&src, &order)?;
    for (i, e) in edits.iter().enumerate() {
        if e.rotate.rem_euclid(360) != 0 {
            pliego_organize::rotate(&mut doc, &[i as u32], e.rotate)?;
        }
    }
    ctx.step(1, 2, "Guardando")?;
    let out = unique_sibling(input, suffix, "pdf");
    pliego_organize::save(&doc, &out)?;
    Ok(Report::with_output(out, format!("{} páginas", edits.len())))
}

/// Dónde se imprime el número: línea (encabezado o pie) y hueco (izquierda, centro, derecha).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NumberPosition {
    HeaderLeft,
    HeaderCenter,
    HeaderRight,
    FooterLeft,
    FooterCenter,
    FooterRight,
}

/// Lo que pide el panel de Numerar páginas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NumberRequest {
    pub position: NumberPosition,
    /// Plantilla con `{page}` y `{pages}` («{page}», «{page} de {pages}», «Página {page}»).
    pub format: String,
    /// Primera página numerada (desde 0); las anteriores quedan sin número.
    #[serde(default)]
    pub from_page: u32,
    #[serde(default = "default_size")]
    pub size: f32,
    /// Además, fija las etiquetas de página del PDF (1, 2, 3…) desde `from_page`.
    #[serde(default)]
    pub also_labels: bool,
}

fn default_size() -> f32 {
    10.0
}

fn slot(hf: &mut HeaderFooter, pos: NumberPosition) -> &mut String {
    match pos {
        NumberPosition::HeaderLeft => &mut hf.header.left,
        NumberPosition::HeaderCenter => &mut hf.header.center,
        NumberPosition::HeaderRight => &mut hf.header.right,
        NumberPosition::FooterLeft => &mut hf.footer.left,
        NumberPosition::FooterCenter => &mut hf.footer.center,
        NumberPosition::FooterRight => &mut hf.footer.right,
    }
}

/// Imprime el número de página con `pliego-stamp` (misma lógica que Encabezado y pie) y guarda una copia.
pub fn number_copy(
    input: &Path,
    password: Option<&str>,
    req: &NumberRequest,
    suffix: &str,
    ctx: &JobContext,
) -> Result<Report> {
    if !req.format.contains("{page}") {
        return Err(Error::InvalidInput("el formato debe incluir {page}".into()));
    }
    if !(4.0..=72.0).contains(&req.size) {
        return Err(Error::InvalidInput(
            "el tamaño del número debe estar entre 4 y 72 pt".into(),
        ));
    }
    let mut doc = pliego_organize::open(input, password)?;
    let n = pliego_organize::page_count(&doc)?;
    if req.from_page >= n {
        return Err(Error::InvalidInput(format!(
            "la página {} no existe (el documento tiene {n})",
            req.from_page + 1
        )));
    }
    let mut hf = HeaderFooter {
        style: TextStyle {
            size: req.size,
            ..TextStyle::default()
        },
        ..HeaderFooter::default()
    };
    *slot(&mut hf, req.position) = req.format.clone();
    let file = input
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_default();
    let pages: Vec<u32> = (req.from_page..n).collect();
    pliego_stamp::header_footer(&mut doc, &pages, &hf, &StampContext::today(file), ctx)?;
    if req.also_labels {
        let rule = PageLabelSpec {
            start_page: req.from_page,
            style: LabelStyle::Decimal,
            prefix: String::new(),
            first_number: 1,
        };
        pliego_organize::set_page_labels(&mut doc, &[rule])?;
    }
    let out = unique_sibling(input, suffix, "pdf");
    pliego_organize::save(&doc, &out)?;
    Ok(Report::with_output(out, format!("{} páginas numeradas", pages.len())))
}

/// Marcadores de `path` con todo lo que hace falta para editarlos sin perder nada (altura de destino, enlaces web).
pub fn outline_tree(path: &Path, password: Option<&str>) -> Result<Vec<pliego_organize::OutlineNode>> {
    pliego_organize::read_outline(&pliego_organize::open(path, password)?)
}

/// Sustituye los marcadores del PDF abierto (copia de seguridad la primera vez y deshacer compartido con los demás
/// modos que editan el archivo). Guardado incremental si se puede. Devuelve el contenido nuevo para recargar el visor.
pub fn outline_save(
    editor: &crate::annot::Editor,
    path: &Path,
    password: Option<&str>,
    nodes: &[pliego_organize::OutlineNode],
    backup_suffix: &str,
) -> Result<Vec<u8>> {
    if nodes.iter().any(has_empty_title) {
        return Err(Error::InvalidInput("un marcador no puede quedarse sin nombre".into()));
    }
    editor.edit_pdf_preserving(path, password, backup_suffix, |doc| {
        pliego_organize::write_outline(doc, nodes)
    })
}

fn has_empty_title(n: &pliego_organize::OutlineNode) -> bool {
    n.title.trim().is_empty() || n.children.iter().any(has_empty_title)
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
    fn outline_is_saved_in_place_with_backup_and_undo() {
        let Some((dir, input)) = setup() else { return };
        let before = std::fs::read(&input).unwrap();
        let editor = crate::annot::Editor::default();
        let nodes = vec![pliego_organize::OutlineNode {
            title: "Introducción".into(),
            page: Some(0),
            top: Some(700.0),
            ..Default::default()
        }];
        outline_save(&editor, &input, None, &nodes, "antes de los marcadores").unwrap();
        assert_eq!(outline_tree(&input, None).unwrap(), nodes);
        assert_eq!(
            std::fs::read(dir.path().join("doc (antes de los marcadores).pdf")).unwrap(),
            before
        );
        editor.undo(&input).unwrap();
        assert_eq!(std::fs::read(&input).unwrap(), before);
        let empty = vec![pliego_organize::OutlineNode {
            title: "  ".into(),
            ..Default::default()
        }];
        assert!(matches!(
            outline_save(&editor, &input, None, &empty, "x"),
            Err(Error::InvalidInput(_))
        ));
    }

    fn number_req() -> NumberRequest {
        NumberRequest {
            position: NumberPosition::FooterCenter,
            format: "Página {page} de {pages}".into(),
            from_page: 0,
            size: 10.0,
            also_labels: true,
        }
    }

    #[test]
    fn number_pages_prints_on_a_copy() {
        let Some((dir, input)) = setup() else { return };
        let before = std::fs::read(&input).unwrap();
        let r = number_copy(&input, None, &number_req(), "numerado", &JobContext::silent()).unwrap();
        assert_eq!(r.outputs, vec![dir.path().join("doc (numerado).pdf")]);
        assert_eq!(std::fs::read(&input).unwrap(), before);
        let out = std::fs::read(&r.outputs[0]).unwrap();
        assert_ne!(out, before);
        assert_eq!(pages(&r.outputs[0]), pages(&input));
    }

    #[test]
    fn number_pages_rejects_bad_requests() {
        let Some((_dir, input)) = setup() else { return };
        let ctx = JobContext::silent();
        let bad_format = NumberRequest {
            format: "sin variable".into(),
            ..number_req()
        };
        assert!(number_copy(&input, None, &bad_format, "n", &ctx).is_err());
        let bad_page = NumberRequest {
            from_page: 999,
            ..number_req()
        };
        assert!(number_copy(&input, None, &bad_page, "n", &ctx).is_err());
        let bad_size = NumberRequest {
            size: 1.0,
            ..number_req()
        };
        assert!(number_copy(&input, None, &bad_size, "n", &ctx).is_err());
    }

    #[test]
    fn pages_copy_reorders_deletes_duplicates_and_rotates() {
        let Some((dir, input)) = setup() else { return };
        let ctx = JobContext::silent();
        let before = std::fs::read(&input).unwrap();
        let edits = [PageEdit { page: 0, rotate: 90 }, PageEdit { page: 0, rotate: 0 }];
        let r = pages_copy(&input, None, &edits, "organizado", &ctx).unwrap();
        assert_eq!(r.outputs, vec![dir.path().join("doc (organizado).pdf")]);
        assert_eq!(pages(&r.outputs[0]), 2);
        assert_eq!(std::fs::read(&input).unwrap(), before);
        let doc = pliego_organize::open(&r.outputs[0], None).unwrap();
        assert_eq!(doc.load_pdf_page(0).unwrap().rotation().unwrap(), 90);
        assert_eq!(doc.load_pdf_page(1).unwrap().rotation().unwrap(), 0);
    }

    #[test]
    fn pages_copy_rejects_empty_bad_rotation_and_missing_pages() {
        let Some((_dir, input)) = setup() else { return };
        let ctx = JobContext::silent();
        assert!(pages_copy(&input, None, &[], "x", &ctx).is_err());
        assert!(pages_copy(&input, None, &[PageEdit { page: 0, rotate: 45 }], "x", &ctx).is_err());
        assert!(pages_copy(&input, None, &[PageEdit { page: 99, rotate: 0 }], "x", &ctx).is_err());
    }

    #[test]
    fn split_writes_parts_next_to_the_original() {
        let Some((dir, input)) = setup() else { return };
        let ctx = JobContext::silent();
        let r = split_copy(&input, None, &SplitRule::EveryN { n: 1 }, "partes", &ctx).unwrap();
        assert!(!r.outputs.is_empty());
        assert!(r.outputs[0].starts_with(dir.path().join("doc (partes)")));
    }

    #[test]
    fn merge_combines_files_in_order_into_a_copy() {
        let Some((dir, input)) = setup() else { return };
        let ctx = JobContext::silent();
        let other = dir.path().join("otro.pdf");
        std::fs::copy(&input, &other).unwrap();
        let m = merge_copy(&[input.clone(), other], &MergeOptions::default(), "combinado", &ctx).unwrap();
        assert_eq!(m.outputs, vec![dir.path().join("doc (combinado).pdf")]);
        assert_eq!(pages(&m.outputs[0]), pages(&input) * 2);
        assert!(merge_copy(&[], &MergeOptions::default(), "combinado", &ctx).is_err());
    }
}
