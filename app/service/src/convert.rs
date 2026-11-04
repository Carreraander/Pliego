//! Convertir desde la app (flujo W24): exportar a imágenes o texto y exportar a Office. Siempre guarda una copia
//! junto al original (archivo o carpeta) y no lo modifica.

use std::path::Path;

use pliego_appdata::copyjob::{unique_in, unique_sibling};
use pliego_convert::{
    ExportImageFormat, ExtractImagesOptions, PdfToImagesOptions, TextExportOptions, extract_images, pdf_to_html,
    pdf_to_images, pdf_to_svg, pdf_to_text,
};
use pliego_core::{Error, JobContext, PageRange, Report, Result};
use pliego_office_export::{CommonOptions, DocxOptions, PptxOptions, ScannedPolicy, XlsxOptions};

/// Lo que pide el panel «Exportar a imágenes o texto».
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ExportRequest {
    /// `png`, `jpg`, `tiff`, `svg`, `images` (las incrustadas), `txt` o `html`.
    pub format: String,
    /// Páginas desde 1: `all`, `1-3,5`, `last`.
    pub pages: String,
    pub dpi: Option<f32>,
    pub jpeg_quality: Option<u8>,
    pub password: Option<String>,
    /// Etiqueta del nombre de la copia, ya traducida («imágenes», «texto»).
    pub label: String,
}

fn page_range(text: &str) -> Result<PageRange> {
    let text = if text.trim().is_empty() { "all" } else { text.trim() };
    text.parse()
        .map_err(|e| Error::InvalidInput(format!("rango de páginas no válido ({text:?}): {e}")))
}

/// Exporta `input` a una carpeta (formatos de imagen) o a un archivo (texto, HTML) junto al original.
pub fn export_copy(input: &Path, req: &ExportRequest, ctx: &JobContext) -> Result<Report> {
    let pages = page_range(&req.pages)?;
    let pw = req.password.as_deref();
    let dpi = req.dpi.unwrap_or(150.0);
    if !(36.0..=1200.0).contains(&dpi) {
        return Err(Error::InvalidInput(format!("resolución fuera de rango: {dpi}")));
    }
    let quality = req.jpeg_quality.unwrap_or(90);
    if !(1..=100).contains(&quality) {
        return Err(Error::InvalidInput(format!("calidad JPEG fuera de rango: {quality}")));
    }
    let folder = || {
        let dir = input
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let stem = input
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "documento".into());
        let out = unique_in(dir, &stem, &req.label, "");
        std::fs::create_dir_all(&out)
            .map(|()| out.clone())
            .map_err(|e| Error::io_at(&out, e))
    };
    match req.format.as_str() {
        "png" | "jpg" | "tiff" => {
            let format: ExportImageFormat = req.format.parse()?;
            let opts = PdfToImagesOptions {
                format,
                dpi,
                jpeg_quality: quality,
            };
            pdf_to_images(input, pw, &pages, &folder()?, &opts, ctx)
        }
        "svg" => pdf_to_svg(input, pw, &pages, &folder()?, ctx),
        "images" => extract_images(input, pw, &pages, &folder()?, &ExtractImagesOptions::default(), ctx),
        "txt" => pdf_to_text(
            input,
            pw,
            &pages,
            &unique_sibling(input, &req.label, "txt"),
            &TextExportOptions::default(),
            ctx,
        ),
        "html" => pdf_to_html(input, pw, &pages, &unique_sibling(input, &req.label, "html"), ctx),
        other => Err(Error::InvalidInput(format!(
            "formato de exportación desconocido: {other:?}"
        ))),
    }
}

/// Lo que pide el panel «Exportar a Word, Excel o PowerPoint».
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OfficeRequest {
    /// `docx`, `xlsx` o `pptx`.
    pub format: String,
    pub pages: String,
    pub image_dpi: Option<f32>,
    /// Con páginas escaneadas: `true` = fallar y sugerir OCR; `false` = exportarlas como imagen y avisar.
    pub fail_on_scanned: bool,
    pub password: Option<String>,
    pub label: String,
}

/// Exporta `input` a `<nombre> (<etiqueta>).docx|xlsx|pptx` junto al original.
pub fn office_copy(input: &Path, req: &OfficeRequest, ctx: &JobContext) -> Result<Report> {
    page_range(&req.pages)?;
    let common = CommonOptions {
        password: req.password.clone(),
        pages: if req.pages.trim().is_empty() {
            "all".into()
        } else {
            req.pages.trim().into()
        },
        scanned: if req.fail_on_scanned {
            ScannedPolicy::Fail
        } else {
            ScannedPolicy::Warn
        },
    };
    let dpi = req.image_dpi.unwrap_or(150.0);
    match req.format.as_str() {
        "docx" => {
            let out = unique_sibling(input, &req.label, "docx");
            pliego_office_export::to_docx(
                input,
                &out,
                &DocxOptions {
                    common,
                    image_dpi: dpi,
                    ..Default::default()
                },
                ctx,
            )
        }
        "xlsx" => {
            let out = unique_sibling(input, &req.label, "xlsx");
            pliego_office_export::to_xlsx(
                input,
                &out,
                &XlsxOptions {
                    common,
                    ..Default::default()
                },
                ctx,
            )
        }
        "pptx" => {
            let out = unique_sibling(input, &req.label, "pptx");
            pliego_office_export::to_pptx(input, &out, &PptxOptions { common, image_dpi: dpi }, ctx)
        }
        other => Err(Error::InvalidInput(format!("formato de Office desconocido: {other:?}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Option<(tempfile::TempDir, std::path::PathBuf)> {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/basic/hola.pdf");
        if !src.exists() {
            return None;
        }
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("doc.pdf");
        std::fs::copy(src, &input).unwrap();
        Some((dir, input))
    }

    fn export(format: &str, label: &str) -> ExportRequest {
        ExportRequest {
            format: format.into(),
            pages: "all".into(),
            label: label.into(),
            ..Default::default()
        }
    }

    #[test]
    fn images_go_to_a_new_folder_next_to_the_original() {
        let Some((dir, input)) = sample() else { return };
        let r = export_copy(&input, &export("png", "imágenes"), &JobContext::silent()).unwrap();
        let folder = dir.path().join("doc (imágenes)");
        assert!(r.outputs[0].starts_with(&folder) && r.outputs[0].exists());
        // Una segunda exportación usa otra carpeta.
        let r2 = export_copy(&input, &export("jpg", "imágenes"), &JobContext::silent()).unwrap();
        assert!(r2.outputs[0].starts_with(dir.path().join("doc (imágenes 2)")));
    }

    #[test]
    fn text_and_html_are_single_files() {
        let Some((dir, input)) = sample() else { return };
        let t = export_copy(&input, &export("txt", "texto"), &JobContext::silent()).unwrap();
        assert_eq!(t.outputs, vec![dir.path().join("doc (texto).txt")]);
        assert!(
            std::fs::read_to_string(&t.outputs[0])
                .unwrap()
                .to_lowercase()
                .contains("hola")
        );
        let h = export_copy(&input, &export("html", "texto"), &JobContext::silent()).unwrap();
        assert_eq!(h.outputs, vec![dir.path().join("doc (texto).html")]);
    }

    #[test]
    fn bad_requests_fail_without_creating_anything() {
        let Some((dir, input)) = sample() else { return };
        let ctx = JobContext::silent();
        assert!(matches!(
            export_copy(&input, &export("bmp", "x"), &ctx),
            Err(Error::InvalidInput(_))
        ));
        let mut r = export("png", "x");
        r.pages = "abc".into();
        assert!(matches!(export_copy(&input, &r, &ctx), Err(Error::InvalidInput(_))));
        r.pages = "all".into();
        r.dpi = Some(5.0);
        assert!(matches!(export_copy(&input, &r, &ctx), Err(Error::InvalidInput(_))));
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1, "solo el original");
    }

    #[test]
    fn office_formats_save_a_copy() {
        let Some((dir, input)) = sample() else { return };
        for ext in ["docx", "xlsx", "pptx"] {
            let req = OfficeRequest {
                format: ext.into(),
                pages: "all".into(),
                label: "office".into(),
                ..Default::default()
            };
            let r = office_copy(&input, &req, &JobContext::silent()).unwrap();
            assert_eq!(r.outputs, vec![dir.path().join(format!("doc (office).{ext}"))]);
            assert!(std::fs::metadata(&r.outputs[0]).unwrap().len() > 0);
        }
        let bad = OfficeRequest {
            format: "odt".into(),
            label: "x".into(),
            ..Default::default()
        };
        assert!(matches!(
            office_copy(&input, &bad, &JobContext::silent()),
            Err(Error::InvalidInput(_))
        ));
    }
}
