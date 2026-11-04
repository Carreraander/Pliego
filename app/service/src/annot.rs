//! Comentar (W26), sin Tauri para probarlo en Linux; `app/src-tauri/src/commands/annot.rs` solo traduce.
//! Operaciones sobre archivos para la app: abrir con contraseña, resumen, exportar e importar XFDF.
//!
//! Son las funciones que llaman los comandos de la app (que no tienen pruebas propias): reciben rutas y nunca
//! tocan el original; las que escriben un PDF lo hacen a una ruta distinta, de forma atómica.

use std::path::Path;

use mupdf::Document;
use mupdf::pdf::{PdfDocument, PdfWriteOptions};
use pliego_core::{Error, Result};

use pliego_appdata::copyjob::unique_sibling;
use pliego_core::Report;

pub use pliego_annot::CommentSummary;
use pliego_annot::{export_xfdf, import_xfdf, summary};

fn pe(e: mupdf::Error) -> Error {
    Error::Pdf(e.to_string())
}

/// Abre un PDF para anotarlo. Se lee a memoria para no dejar el archivo abierto (en Windows impediría sustituirlo).
pub fn open(path: &Path, password: Option<&str>) -> Result<PdfDocument> {
    let bytes = std::fs::read(path).map_err(|e| Error::io_at(path, e))?;
    let mut doc = Document::from_bytes(&bytes, "application/pdf").map_err(pe)?;
    if doc.needs_password().map_err(pe)? && !password.is_some_and(|p| doc.authenticate(p).unwrap_or(false)) {
        return Err(Error::Password);
    }
    PdfDocument::try_from(doc).map_err(|_| Error::InvalidInput(format!("{} no es un PDF", path.display())))
}

/// Guarda a un temporal junto al destino y lo renombra.
pub fn save(doc: &PdfDocument, out: &Path) -> Result<()> {
    let dir = out
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let tmp = tempfile::Builder::new()
        .prefix(".pliego-")
        .suffix(".tmp")
        .tempfile_in(dir)
        .map_err(|e| Error::io_at(dir, e))?
        // Se suelta el manejador abierto: en Windows impide renombrar mientras exista.
        .into_temp_path();
    let path = tmp
        .to_str()
        .ok_or_else(|| Error::InvalidInput("ruta no válida".into()))?;
    let mut opts = PdfWriteOptions::default();
    opts.set_garbage_level(3).set_compress(true);
    doc.save_with_options(path, opts).map_err(pe)?;
    // En Windows un antivirus o el indexador pueden tener el archivo recién escrito unos milisegundos.
    let mut tmp = tmp;
    for attempt in 0..5 {
        match tmp.persist(out) {
            Ok(()) => return Ok(()),
            Err(e) if attempt < 4 => {
                tmp = e.path;
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Err(e) => return Err(Error::io_at(out, e.error)),
        }
    }
    unreachable!("el bucle devuelve en el último intento")
}

/// Resumen de los comentarios de `src`.
pub fn summary_of(src: &Path, password: Option<&str>) -> Result<CommentSummary> {
    summary(&mut open(src, password)?)
}

/// Exporta los comentarios de `src` a `<nombre> (<suffix>).xfdf` junto a él.
pub fn export_xfdf_copy(src: &Path, password: Option<&str>, suffix: &str) -> Result<Report> {
    let mut doc = open(src, password)?;
    let total = summary(&mut doc)?.total;
    let out = unique_sibling(src, suffix, "xfdf");
    std::fs::write(&out, export_xfdf(&mut doc)?).map_err(|e| Error::io_at(&out, e))?;
    Ok(Report::with_output(out, format!("{total} comentario(s) exportados")))
}

/// Importa el XFDF `xfdf` en una copia `<nombre> (<suffix>).pdf` de `src`; el original no cambia.
pub fn import_xfdf_copy(src: &Path, password: Option<&str>, xfdf: &Path, suffix: &str) -> Result<Report> {
    let text = std::fs::read_to_string(xfdf).map_err(|e| Error::io_at(xfdf, e))?;
    let mut doc = open(src, password)?;
    let added = import_xfdf(&mut doc, &text)?;
    let out = unique_sibling(src, suffix, "pdf");
    save(&doc, &out)?;
    Ok(Report::with_output(out, format!("{added} comentario(s) importados")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mupdf::Size;
    use pliego_annot::{AnnotSpec, Geometry, add};

    fn pdf_with_notes(path: &Path, n: usize) {
        let mut doc = PdfDocument::new();
        doc.new_page(Size::A4).unwrap();
        for i in 0..n {
            let mut s = AnnotSpec::new(0, Geometry::Note { at: [50.0, 50.0] });
            s.author = Some("Ana".into());
            s.contents = Some(format!("nota {i}"));
            add(&mut doc, &s).unwrap();
        }
        save(&doc, path).unwrap();
    }

    #[test]
    fn summary_counts_comments() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.pdf");
        pdf_with_notes(&p, 2);
        let s = summary_of(&p, None).unwrap();
        assert_eq!(s.total, 2);
        assert_eq!(s.by_author.get("Ana"), Some(&2));
    }

    #[test]
    fn xfdf_roundtrip_goes_to_a_copy_and_leaves_the_original() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.pdf");
        pdf_with_notes(&src, 2);
        let r = export_xfdf_copy(&src, None, "comentarios").unwrap();
        let xfdf = r.outputs[0].clone();
        assert_eq!(xfdf, dir.path().join("a (comentarios).xfdf"));
        assert!(std::fs::read_to_string(&xfdf).unwrap().contains("nota 0"));

        let mut blank = PdfDocument::new();
        blank.new_page(Size::A4).unwrap();
        let clean = dir.path().join("b.pdf");
        save(&blank, &clean).unwrap();
        let before = std::fs::read(&clean).unwrap();
        let r = import_xfdf_copy(&clean, None, &xfdf, "con comentarios").unwrap();
        assert_eq!(r.outputs[0], dir.path().join("b (con comentarios).pdf"));
        assert_eq!(std::fs::read(&clean).unwrap(), before);
        assert_eq!(summary_of(&r.outputs[0], None).unwrap().total, 2);
        // Una segunda importación no pisa la anterior.
        let r2 = import_xfdf_copy(&clean, None, &xfdf, "con comentarios").unwrap();
        assert_eq!(r2.outputs[0], dir.path().join("b (con comentarios 2).pdf"));
    }

    #[test]
    fn missing_file_is_an_error() {
        assert!(open(Path::new("/no/existe.pdf"), None).is_err());
    }
}
