//! Medir sobre planos desde la app (flujo W29): escala, medida en vivo y anotaciones de medida.
//!
//! Sin Tauri, para probarlo en Linux; `commands/measure.rs` solo traduce argumentos. Los puntos están en el espacio de
//! página del visor (puntos PDF, origen arriba a la izquierda), el mismo que usa `pliego-annot`. Guardar una medida
//! escribe en el propio PDF con copia de seguridad previa y comparte la pila de deshacer de Comentar.

use std::path::Path;

use pliego_core::{Lang, Result};
pub use pliego_measure::{Measurement, Pt, Scale, Unit};

use crate::annot::{Editor, Saved, open};

/// Escala guardada en la página `page` (desde 0), si tiene una.
pub fn page_scale(path: &Path, password: Option<&str>, page: u32) -> Result<Option<Scale>> {
    pliego_measure::page_scale(&open(path, password)?, page)
}

/// Escala por calibración: `page_points` puntos del papel equivalen a `real` `unit`.
pub fn calibrate(page_points: f64, real: f64, unit: Unit) -> Result<Scale> {
    Scale::calibrate(page_points, real, unit)
}

/// Escala 1:`n` expresada en `unit` (1 mm del papel = `n` mm reales).
pub fn ratio_scale(n: f64, unit: Unit) -> Result<Scale> {
    let real = n * Unit::Mm.points() / unit.points();
    Scale::calibrate(Unit::Mm.points(), real, unit)
}

/// Mide una figura sin tocar el documento.
pub fn measure(scale: &Scale, points: &[Pt], closed: bool, lang: Lang) -> Result<Measurement> {
    pliego_measure::measure_in(scale, points, closed, lang)
}

/// Guarda la escala de la página y una anotación de medida en el propio PDF, añadiendo los cambios al final
/// del archivo (no se recomprime ni se reordena el contenido original). Devuelve el contenido nuevo y la medida.
#[allow(clippy::too_many_arguments)]
pub fn save(
    editor: &Editor,
    path: &Path,
    password: Option<&str>,
    backup_suffix: &str,
    page: u32,
    scale: &Scale,
    points: &[Pt],
    closed: bool,
    lang: Lang,
) -> Result<(Vec<u8>, Measurement)> {
    let (saved, m) = save_saved(editor, path, password, backup_suffix, page, scale, points, closed, lang)?;
    Ok((saved.into_bytes()?, m))
}

/// Como [`save`], con el resultado como [`Saved`] (PDF grandes: se guarda al final del archivo).
#[allow(clippy::too_many_arguments)]
pub fn save_saved(
    editor: &Editor,
    path: &Path,
    password: Option<&str>,
    backup_suffix: &str,
    page: u32,
    scale: &Scale,
    points: &[Pt],
    closed: bool,
    lang: Lang,
) -> Result<(Saved, Measurement)> {
    let mut measured = None;
    let bytes = editor.edit_pdf_preserving_saved(path, password, backup_suffix, |doc| {
        pliego_measure::set_scale(doc, page, scale)?;
        measured = Some(pliego_measure::add_measurement_in(doc, page, scale, points, closed, lang)?.0);
        Ok(())
    })?;
    Ok((bytes, measured.expect("la edición se aplicó")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ratio_scale_matches_the_drawing_scale() {
        // 1:100 → 1 mm de papel = 100 mm reales = 0,1 m.
        let s = ratio_scale(100.0, Unit::M).unwrap();
        let m = measure(
            &s,
            &[[0.0, 0.0], [Unit::Mm.points() as f32 * 10.0, 0.0]],
            false,
            Lang::Es,
        )
        .unwrap();
        assert!((m.length - 1.0).abs() < 1e-3, "{m:?}"); // 10 mm de papel = 1 m
        assert!(ratio_scale(0.0, Unit::M).is_err());
    }

    #[test]
    fn calibration_and_polygon_area() {
        let s = calibrate(100.0, 5.0, Unit::M).unwrap(); // 100 pt = 5 m
        let sq = [[0.0, 0.0], [100.0, 0.0], [100.0, 100.0], [0.0, 100.0]];
        let m = measure(&s, &sq, true, Lang::Es).unwrap();
        assert!((m.area.unwrap() - 25.0).abs() < 1e-6);
        assert!((m.length - 20.0).abs() < 1e-6);
        assert!(measure(&s, &sq[..2], true, Lang::Es).is_err());
        assert!(calibrate(0.0, 5.0, Unit::M).is_err());
    }

    #[test]
    fn saving_keeps_the_scale_and_adds_an_annotation() {
        let dir = std::env::temp_dir().join(format!("pliego-measure-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("plano.pdf");
        std::fs::copy(
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/corpus/basic/hola.pdf"),
            &path,
        )
        .unwrap();
        assert!(page_scale(&path, None, 0).unwrap().is_none());

        let scale = calibrate(100.0, 5.0, Unit::M).unwrap();
        let editor = Editor::default();
        let (bytes, m) = save(
            &editor,
            &path,
            None,
            "copia",
            0,
            &scale,
            &[[10.0, 10.0], [110.0, 10.0]],
            false,
            Lang::Es,
        )
        .unwrap();
        assert!((m.length - 5.0).abs() < 1e-6);
        assert!(!bytes.is_empty());
        let back = page_scale(&path, None, 0)
            .unwrap()
            .expect("la escala queda en la página");
        assert!((back.units_per_point - scale.units_per_point).abs() < 1e-9);
        assert_eq!(editor.undo_depth(&path), 1);
        assert!(save(&editor, &path, None, "copia", 0, &scale, &[[0.0, 0.0]], false, Lang::Es).is_err());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn saving_appends_to_the_file_without_rewriting_its_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plano.pdf");
        let src = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/corpus/office/articulo-3.pdf");
        std::fs::copy(src, &path).unwrap();
        let original = std::fs::read(&path).unwrap();

        let scale = calibrate(100.0, 5.0, Unit::M).unwrap();
        let editor = Editor::default();
        let (bytes, _) = save(
            &editor,
            &path,
            None,
            "copia",
            0,
            &scale,
            &[[10.0, 10.0], [110.0, 10.0]],
            false,
            Lang::Es,
        )
        .unwrap();

        // El contenido original queda intacto al principio y solo crece lo añadido.
        assert_eq!(&bytes[..original.len()], &original[..]);
        assert!(
            bytes.len() - original.len() < 4096,
            "creció {} bytes",
            bytes.len() - original.len()
        );
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        // Se puede volver a medir sobre lo guardado y deshacer hasta el original.
        assert!(page_scale(&path, None, 0).unwrap().is_some());
        editor.undo(&path).unwrap().unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), original);
    }
}
