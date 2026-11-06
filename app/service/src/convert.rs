//! Convertir desde la app (flujo W24): exportar a imágenes o texto y exportar a Office. Siempre guarda una copia
//! junto al original (archivo o carpeta) y no lo modifica.

use std::path::{Path, PathBuf};

use pliego_appdata::copyjob::{unique_in, unique_sibling};
use pliego_convert::{
    ExportImageFormat, ExtractImagesOptions, ImagesToPdfOptions, Orientation, PageSizeSpec, PdfToImagesOptions,
    TextExportOptions, TextToPdfOptions, cbz_to_pdf, epub_to_pdf, extract_images, html_to_pdf, images_to_pdf,
    markdown_to_pdf, office_to_pdf, pdf_to_html, pdf_to_images, pdf_to_text, text_to_pdf, xps_to_pdf,
};
use pliego_core::{Error, JobContext, Lang, PageRange, Report, Result};
use pliego_office_export::{CommonOptions, DocxOptions, PptxOptions, ScannedPolicy, XlsxOptions};

/// Lo que pide el panel «Exportar a imágenes o texto».
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ExportRequest {
    /// `png`, `jpg`, `tiff`, `images` (las incrustadas), `txt` o `html`.
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
    /// Idioma de los textos que se escriben en el archivo (nombres de hoja, idioma de revisión): el de la interfaz.
    pub lang: Lang,
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
        lang: req.lang,
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

/// Lo que pide el panel «Crear PDF».
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CreateRequest {
    /// `fit`, `a3`, `a4`, `a5`, `letter` o `legal`.
    pub page_size: String,
    pub margin_mm: f32,
    /// `auto`, `portrait` o `landscape` (solo imágenes).
    pub orientation: String,
    pub upscale: bool,
    /// Etiqueta del nombre del PDF («PDF»).
    pub label: String,
}

type TextConverter = fn(&Path, &Path, &TextToPdfOptions, &JobContext) -> Result<Report>;

const OFFICE_EXTENSIONS: &[&str] = &[
    "doc", "docx", "docm", "dot", "dotx", "rtf", "odt", "xls", "xlsx", "xlsm", "xlsb", "csv", "ods", "ppt", "pptx",
    "pptm", "pps", "ppsx", "odp",
];

/// Crea un PDF a partir de imágenes (una página por imagen), de un `.txt`, `.md` o `.html`, de un libro `.epub`, un cómic
/// `.cbz`, un documento `.xps` o de un documento de Office. El nombre sale del primer archivo: `<primero> (<etiqueta>).pdf`.
pub fn create_copy(inputs: &[PathBuf], req: &CreateRequest, ctx: &JobContext) -> Result<Report> {
    let first = inputs
        .first()
        .ok_or_else(|| Error::InvalidInput("no se indicó ningún archivo".into()))?;
    let page_size: PageSizeSpec = if req.page_size.trim().is_empty() {
        "fit"
    } else {
        req.page_size.trim()
    }
    .parse()?;
    if !req.margin_mm.is_finite() || !(0.0..=100.0).contains(&req.margin_mm) {
        return Err(Error::InvalidInput(format!("margen fuera de rango: {}", req.margin_mm)));
    }
    let margin_pt = req.margin_mm * 72.0 / 25.4;
    let orientation = match req.orientation.as_str() {
        "" | "auto" => Orientation::Auto,
        "portrait" => Orientation::Portrait,
        "landscape" => Orientation::Landscape,
        other => return Err(Error::InvalidInput(format!("orientación desconocida: {other:?}"))),
    };
    let ext = first.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    let one_only = |what: &str| {
        if inputs.len() == 1 {
            Ok(())
        } else {
            Err(Error::InvalidInput(format!("{what} se convierten de uno en uno")))
        }
    };
    let out = unique_sibling(first, &req.label, "pdf");
    if OFFICE_EXTENSIONS.contains(&ext.as_str()) {
        one_only("los documentos de Office")?;
        return office_to_pdf(first, &out, ctx);
    }
    if ext == "xps" || ext == "oxps" {
        one_only("los documentos XPS")?;
        return xps_to_pdf(first, &out, ctx);
    }
    let text_fn: Option<TextConverter> = match ext.as_str() {
        "txt" => Some(text_to_pdf),
        "md" | "markdown" => Some(markdown_to_pdf),
        "html" | "htm" => Some(html_to_pdf),
        "epub" => Some(epub_to_pdf),
        _ => None,
    };
    if let Some(convert) = text_fn {
        one_only("los archivos de texto, Markdown, HTML y EPUB")?;
        let opts = TextToPdfOptions {
            page_size: if page_size == PageSizeSpec::Fit {
                PageSizeSpec::A4
            } else {
                page_size
            },
            margin_pt: if req.margin_mm > 0.0 {
                margin_pt
            } else {
                TextToPdfOptions::default().margin_pt
            },
            ..Default::default()
        };
        return convert(first, &out, &opts, ctx);
    }
    let opts = ImagesToPdfOptions {
        page_size,
        margin_pt,
        orientation,
        upscale: req.upscale,
        ..Default::default()
    };
    if ext == "cbz" {
        one_only("los cómics CBZ")?;
        return cbz_to_pdf(first, &out, &opts, ctx);
    }
    images_to_pdf(inputs, &out, &opts, ctx)
}

/// Guarda `<nombre> (<etiqueta>).pdf` en el nivel PDF/A pedido (`1b`, `2b`, `2u` o `3b`).
pub fn pdfa_copy(input: &Path, level: &str, label: &str, ctx: &JobContext) -> Result<Report> {
    let level: pliego_pdfa::Level = level
        .parse()
        .map_err(|_| Error::InvalidInput(format!("nivel PDF/A desconocido: {level:?} (usa 1b, 2b, 2u o 3b)")))?;
    let out = unique_sibling(input, label, "pdf");
    pliego_pdfa::convert(input, &out, level, ctx)
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
        let _serial = crate::test_support::serial();
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
        let _serial = crate::test_support::serial();
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
    fn books_comics_and_xps_become_pdf_copies() {
        let _serial = crate::test_support::serial();
        let dir = tempfile::tempdir().unwrap();
        let ctx = JobContext::silent();
        for (file, label) in [("mini.cbz", "cómic"), ("mini.epub", "libro"), ("mini.xps", "xps")] {
            let src = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/corpus/packages")
                .join(file);
            let input = dir.path().join(file);
            std::fs::copy(src, &input).unwrap();
            let req = CreateRequest {
                label: label.into(),
                ..Default::default()
            };
            let r = create_copy(std::slice::from_ref(&input), &req, &ctx).unwrap();
            let expected = dir
                .path()
                .join(format!("{} ({label}).pdf", file.split('.').next().unwrap()));
            assert_eq!(r.outputs, vec![expected.clone()], "{file}");
            assert!(std::fs::metadata(&expected).unwrap().len() > 200, "{file}");
            // Varios a la vez no se admiten.
            assert!(matches!(
                create_copy(&[input.clone(), input], &req, &ctx),
                Err(Error::InvalidInput(_))
            ));
        }
    }

    #[test]
    fn bad_requests_fail_without_creating_anything() {
        let _serial = crate::test_support::serial();
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
        let _serial = crate::test_support::serial();
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

    #[test]
    fn creates_a_pdf_from_text_and_rejects_bad_requests() {
        let _serial = crate::test_support::serial();
        let dir = tempfile::tempdir().unwrap();
        let txt = dir.path().join("nota.txt");
        std::fs::write(&txt, "Hola desde Pliego").unwrap();
        let req = CreateRequest {
            page_size: "a4".into(),
            orientation: "auto".into(),
            label: "PDF".into(),
            ..Default::default()
        };
        let r = create_copy(std::slice::from_ref(&txt), &req, &JobContext::silent()).unwrap();
        assert_eq!(r.outputs, vec![dir.path().join("nota (PDF).pdf")]);
        assert!(std::fs::metadata(&r.outputs[0]).unwrap().len() > 0);
        let ctx = JobContext::silent();
        assert!(matches!(create_copy(&[], &req, &ctx), Err(Error::InvalidInput(_))));
        let two = [txt.clone(), txt.clone()];
        assert!(matches!(create_copy(&two, &req, &ctx), Err(Error::InvalidInput(_))));
        let bad = CreateRequest {
            page_size: "a4".into(),
            margin_mm: -1.0,
            ..req.clone()
        };
        assert!(matches!(
            create_copy(std::slice::from_ref(&txt), &bad, &ctx),
            Err(Error::InvalidInput(_))
        ));
        let bad = CreateRequest {
            orientation: "diagonal".into(),
            ..req
        };
        assert!(matches!(create_copy(&[txt], &bad, &ctx), Err(Error::InvalidInput(_))));
    }

    #[test]
    fn pdfa_rejects_unknown_levels_and_never_overwrites() {
        let _serial = crate::test_support::serial();
        let Some((dir, input)) = sample() else { return };
        let ctx = JobContext::silent();
        assert!(matches!(
            pdfa_copy(&input, "9z", "PDF-A", &ctx),
            Err(Error::InvalidInput(_))
        ));
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
        // El resultado (si el motor puede convertir este documento) va a una copia nueva.
        if let Ok(r) = pdfa_copy(&input, "2b", "PDF-A", &ctx) {
            assert_eq!(r.outputs, vec![dir.path().join("doc (PDF-A).pdf")]);
        }
    }
}
