//! Diseño de formularios sobre el lienzo (W29b), sin Tauri para probarlo en Linux;
//! `app/src-tauri/src/commands/formdesign.rs` solo traduce argumentos.
//!
//! La interfaz trabaja en el espacio de página del visor (puntos, origen arriba a la izquierda, con el giro de la
//! página aplicado); `pliego-formdesign` usa el espacio de usuario del PDF (origen abajo a la izquierda). Aquí se
//! convierten los rectángulos con la matriz de la página. Los cambios van al propio PDF con copia de seguridad y
//! comparten la pila de deshacer de [`Editor`].

use std::path::Path;

use pliego_core::{Error, Result};
use pliego_pdf::{Matrix, PdfDocument};

pub use pliego_formdesign::{FieldPatch, FieldSpec, FieldType};

use crate::annot::{Editor, open};

/// Caja `[x0, y0, x1, y1]` normalizada de los cuatro vértices transformados.
fn bounds_of(pts: [(f32, f32); 4]) -> [f32; 4] {
    let xs = pts.map(|p| p.0);
    let ys = pts.map(|p| p.1);
    let min = |v: [f32; 4]| v.into_iter().fold(f32::INFINITY, f32::min);
    let max = |v: [f32; 4]| v.into_iter().fold(f32::NEG_INFINITY, f32::max);
    [min(xs), min(ys), max(xs), max(ys)]
}

fn map_rect(r: [f32; 4], m: &Matrix) -> [f32; 4] {
    let at = |x: f32, y: f32| (m.a * x + m.c * y + m.e, m.b * x + m.d * y + m.f);
    bounds_of([at(r[0], r[1]), at(r[2], r[1]), at(r[0], r[3]), at(r[2], r[3])])
}

/// Rectángulo del espacio de usuario del PDF al de página (el del visor).
fn user_to_page(doc: &PdfDocument, page: usize, r: [f32; 4]) -> Result<[f32; 4]> {
    let ctm = doc.load_pdf_page(page as i32)?.ctm()?;
    Ok(map_rect(r, &ctm))
}

/// Rectángulo del espacio de página del visor al de usuario del PDF.
fn page_to_user(doc: &PdfDocument, page: usize, r: [f32; 4]) -> Result<[f32; 4]> {
    let ctm = doc.load_pdf_page(page as i32)?.ctm()?;
    let inv = ctm
        .invert()
        .ok_or_else(|| Error::Pdf("la transformación de la página no es invertible".into()))?;
    Ok(map_rect(r, &inv))
}

/// Campos del formulario de `path`, con el rectángulo en el espacio de página del visor.
pub fn fields(path: &Path, password: Option<&str>) -> Result<Vec<FieldSpec>> {
    let doc = open(path, password)?;
    let mut list = pliego_formdesign::list_fields(&doc)?;
    for f in &mut list {
        f.rect = user_to_page(&doc, f.page, f.rect)?;
    }
    Ok(list)
}

/// Añade un campo (`spec.rect` en espacio de página) al propio PDF y devuelve el contenido nuevo.
pub fn add(editor: &Editor, path: &Path, spec: &FieldSpec, backup_suffix: &str) -> Result<Vec<u8>> {
    editor.edit_pdf(path, backup_suffix, |doc| {
        let mut spec = spec.clone();
        spec.rect = page_to_user(doc, spec.page, spec.rect)?;
        pliego_formdesign::add_field(doc, &spec)
    })
}

/// Cambia propiedades de un campo; `patch.rect`, si viene, está en espacio de página.
pub fn update(editor: &Editor, path: &Path, name: &str, patch: &FieldPatch, backup_suffix: &str) -> Result<Vec<u8>> {
    editor.edit_pdf(path, backup_suffix, |doc| {
        let mut patch = patch.clone();
        if let Some(r) = patch.rect {
            let page = pliego_formdesign::list_fields(doc)?
                .into_iter()
                .find(|f| f.name == name)
                .map(|f| f.page)
                .ok_or_else(|| Error::InvalidInput(format!("no existe el campo «{name}»")))?;
            patch.rect = Some(page_to_user(doc, page, r)?);
        }
        pliego_formdesign::update_field(doc, name, &patch)
    })
}

/// Borra un campo.
pub fn delete(editor: &Editor, path: &Path, name: &str, backup_suffix: &str) -> Result<Vec<u8>> {
    editor.edit_pdf(path, backup_suffix, |doc| pliego_formdesign::delete_field(doc, name))
}

/// Detecta campos en todas las páginas de `path` y, si encuentra alguno, guarda una copia
/// `<nombre> (<suffix>).pdf` junto al original con los campos creados. El original no cambia; sin campos no se crea nada.
pub fn detect_copy(
    path: &Path,
    password: Option<&str>,
    suffix: &str,
    ctx: &pliego_core::JobContext,
) -> Result<pliego_core::Report> {
    let mut doc = open(path, password)?;
    let pages = doc.page_count()?.max(0) as usize;
    let mut found = Vec::new();
    for p in 0..pages {
        ctx.step(p as u64, pages as u64, "")?;
        found.extend(pliego_formdesign::detect_fields(&doc, p, ctx)?);
    }
    if found.is_empty() {
        return Ok(pliego_core::Report {
            summary: "No se encontraron campos".into(),
            ..Default::default()
        });
    }
    for f in &found {
        pliego_formdesign::add_field(&mut doc, f)?;
    }
    let out = pliego_appdata::copyjob::unique_sibling(path, suffix, "pdf");
    crate::annot::save(&doc, &out)?;
    ctx.step(pages as u64, pages as u64, "Terminado")?;
    Ok(pliego_core::Report::with_output(
        &out,
        format!("{} campos creados", found.len()),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::annot::save;
    use pliego_pdf::Size;

    fn blank(path: &Path) {
        let mut doc = PdfDocument::new();
        doc.new_page(Size::A4).unwrap();
        save(&doc, path).unwrap();
    }

    fn text_field(name: &str, rect: [f32; 4]) -> FieldSpec {
        FieldSpec {
            name: name.into(),
            kind: FieldType::Text,
            page: 0,
            rect,
            ..Default::default()
        }
    }

    #[test]
    fn add_converts_page_space_to_pdf_space_and_back() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("f.pdf");
        blank(&p);
        let ed = Editor::default();
        // A4 = 595 × 842: un campo arriba a la izquierda en el visor está arriba (y grande) en el PDF.
        add(
            &ed,
            &p,
            &text_field("nombre", [50.0, 100.0, 250.0, 120.0]),
            "copia de seguridad",
        )
        .unwrap();
        let back = fields(&p, None).unwrap();
        assert_eq!(back.len(), 1);
        let r = back[0].rect;
        for (got, want) in r.iter().zip([50.0, 100.0, 250.0, 120.0]) {
            assert!((got - want).abs() < 0.5, "{r:?}");
        }
        // En espacio de usuario el campo queda cerca de la parte alta de la página.
        let doc = open(&p, None).unwrap();
        let raw = pliego_formdesign::list_fields(&doc).unwrap();
        assert!(raw[0].rect[1] > 700.0, "{:?}", raw[0].rect);
        assert!(dir.path().join("f (copia de seguridad).pdf").exists());
    }

    #[test]
    fn update_delete_and_undo() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("f.pdf");
        blank(&p);
        let ed = Editor::default();
        add(&ed, &p, &text_field("a", [10.0, 10.0, 110.0, 30.0]), "c").unwrap();
        let patch = FieldPatch {
            required: Some(true),
            rect: Some([20.0, 40.0, 140.0, 60.0]),
            ..Default::default()
        };
        update(&ed, &p, "a", &patch, "c").unwrap();
        let f = &fields(&p, None).unwrap()[0];
        assert!(f.required);
        assert!(
            (f.rect[0] - 20.0).abs() < 0.5 && (f.rect[1] - 40.0).abs() < 0.5,
            "{:?}",
            f.rect
        );
        delete(&ed, &p, "a", "c").unwrap();
        assert!(fields(&p, None).unwrap().is_empty());
        assert_eq!(ed.undo_depth(&p), 3);
        ed.undo(&p).unwrap().unwrap();
        assert_eq!(fields(&p, None).unwrap().len(), 1);
    }

    #[test]
    fn errors_leave_the_file_alone() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("f.pdf");
        blank(&p);
        let before = std::fs::read(&p).unwrap();
        let ed = Editor::default();
        assert!(delete(&ed, &p, "no-existe", "c").is_err());
        assert!(
            update(
                &ed,
                &p,
                "no-existe",
                &FieldPatch {
                    rect: Some([0.0, 0.0, 9.0, 9.0]),
                    ..Default::default()
                },
                "c"
            )
            .is_err()
        );
        assert_eq!(std::fs::read(&p).unwrap(), before);
        assert_eq!(ed.undo_depth(&p), 0);
    }
}
