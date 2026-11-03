use std::io::Read;
use std::path::{Path, PathBuf};

use pliego_core::{CancelToken, Error, JobContext, NoProgress};
use pliego_office_export::*;

fn corpus(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../tests/corpus/office/{name}.pdf"))
}

fn names() -> Vec<String> {
    let mut v = Vec::new();
    for k in ["carta", "factura", "articulo", "tabla"] {
        for i in 1..=5 {
            v.push(format!("{k}-{i}"));
        }
    }
    v
}

fn read_part(file: &Path, part: &str) -> Option<String> {
    let mut z = zip::ZipArchive::new(std::fs::File::open(file).unwrap()).unwrap();
    let mut f = z.by_name(part).ok()?;
    let mut s = String::new();
    f.read_to_string(&mut s).unwrap();
    Some(s)
}

/// Todas las partes XML del paquete están bien formadas.
fn assert_well_formed(file: &Path) {
    let mut z = zip::ZipArchive::new(std::fs::File::open(file).unwrap()).unwrap();
    for i in 0..z.len() {
        let mut f = z.by_index(i).unwrap();
        let name = f.name().to_string();
        if name.ends_with(".xml") || name.ends_with(".rels") {
            let mut s = String::new();
            f.read_to_string(&mut s).unwrap();
            roxmltree::Document::parse(&s).unwrap_or_else(|e| panic!("{} / {name}: {e}", file.display()));
        }
    }
}

fn layout(name: &str) -> Layout {
    analyze_layout(&corpus(name), &CommonOptions::default(), &JobContext::silent()).unwrap()
}

#[test]
fn corpus_exports_to_well_formed_docx_xlsx_pptx() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = JobContext::silent();
    for n in names() {
        let d = dir.path().join(format!("{n}.docx"));
        let r = to_docx(&corpus(&n), &d, &DocxOptions::default(), &ctx).unwrap();
        assert!(r.warnings.is_empty(), "{n}: {:?}", r.warnings);
        assert_well_formed(&d);
        let x = dir.path().join(format!("{n}.xlsx"));
        to_xlsx(&corpus(&n), &x, &XlsxOptions::default(), &ctx).unwrap();
        assert_well_formed(&x);
        let p = dir.path().join(format!("{n}.pptx"));
        to_pptx(&corpus(&n), &p, &PptxOptions::default(), &ctx).unwrap();
        assert_well_formed(&p);
    }
}

#[test]
fn invoices_become_real_tables() {
    for i in 1..=5 {
        let l = layout(&format!("factura-{i}"));
        let main = l
            .pages
            .iter()
            .flat_map(|p| p.tables())
            .max_by_key(|t| t.rows.len())
            .expect("la factura tiene tabla");
        assert_eq!(main.cols.len(), 5, "factura-{i}");
        assert!(main.rows.len() >= 6, "factura-{i}: {} filas", main.rows.len());
        assert_eq!(main.rows[0][0].text(), "Descripción");
        assert_eq!(main.rows[0][4].text(), "Importe");
        // Los importes van alineados a la derecha.
        assert_eq!(main.rows[1][4].align, Align::Right);
    }
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path().join("f.docx");
    to_docx(&corpus("factura-1"), &d, &DocxOptions::default(), &JobContext::silent()).unwrap();
    assert!(read_part(&d, "word/document.xml").unwrap().contains("<w:tbl>"));
    let x = dir.path().join("f.xlsx");
    to_xlsx(&corpus("factura-1"), &x, &XlsxOptions::default(), &JobContext::silent()).unwrap();
    let sheet = read_part(&x, "xl/worksheets/sheet1.xml").unwrap();
    // Los números son numéricos, no texto.
    assert!(sheet.contains("<c r=\"E2\" s=\"1\"><v>859.8</v></c>"), "{sheet}");
}

#[test]
fn two_column_articles_keep_reading_order() {
    for i in 1..=5 {
        let l = layout(&format!("articulo-{i}"));
        let cols: Vec<&Section> = l.pages[0].sections.iter().filter(|s| s.columns.len() == 2).collect();
        assert_eq!(cols.len(), 1, "articulo-{i}: debe haber una sección a dos columnas");
        assert!(cols[0].columns[0].x1 < cols[0].columns[1].x0);
        // Título y resumen van antes que las columnas.
        let first = &l.pages[0].sections[0];
        assert_eq!(first.columns.len(), 1);
        let Element::Paragraph(title) = &first.content[0][0] else {
            panic!()
        };
        assert!(title.text().starts_with("Un estudio sobre"));
    }
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path().join("a.docx");
    to_docx(
        &corpus("articulo-1"),
        &d,
        &DocxOptions::default(),
        &JobContext::silent(),
    )
    .unwrap();
    let doc = read_part(&d, "word/document.xml").unwrap();
    assert!(doc.contains("<w:cols w:num=\"2\""));
    assert!(doc.contains("w:br w:type=\"column\""));
}

#[test]
fn headers_footers_and_page_fields() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path().join("a.docx");
    to_docx(
        &corpus("articulo-1"),
        &d,
        &DocxOptions::default(),
        &JobContext::silent(),
    )
    .unwrap();
    assert!(read_part(&d, "word/header1.xml").unwrap().contains("Revista"));
    assert!(read_part(&d, "word/footer1.xml").unwrap().contains(" PAGE "));
    // El texto repetido no se queda en el cuerpo.
    assert!(!read_part(&d, "word/document.xml").unwrap().contains("Revista"));
    let opts = DocxOptions {
        headers_footers: false,
        ..Default::default()
    };
    to_docx(&corpus("articulo-1"), &d, &opts, &JobContext::silent()).unwrap();
    assert!(read_part(&d, "word/header1.xml").is_none());
}

#[test]
fn lists_are_detected() {
    let l = layout("carta-5");
    let items: Vec<_> = l.pages[0]
        .elements()
        .filter_map(|e| match e {
            Element::Paragraph(p) if p.list.is_some() => Some(p),
            _ => None,
        })
        .collect();
    assert_eq!(items.len(), 3);
    assert!(items.iter().all(|p| !p.text().starts_with('•')));
    let l = layout("articulo-5");
    let numbered = l.pages[0]
        .elements()
        .filter(|e| matches!(e, Element::Paragraph(p) if p.list.is_some_and(|l| l.kind == ListKind::Decimal)))
        .count();
    assert!(numbered >= 3);
}

#[test]
fn scanned_pdf_warns_and_can_fail() {
    assert_eq!(scanned_pages(&corpus("escaneado"), None).unwrap(), vec![1]);
    assert!(scanned_pages(&corpus("carta-1"), None).unwrap().is_empty());
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path().join("e.docx");
    let r = to_docx(&corpus("escaneado"), &d, &DocxOptions::default(), &JobContext::silent()).unwrap();
    assert!(r.warnings[0].contains("OCR"), "{:?}", r.warnings);
    assert!(read_part(&d, "word/document.xml").unwrap().contains("<w:drawing>"));
    let opts = DocxOptions {
        common: CommonOptions {
            scanned: ScannedPolicy::Fail,
            ..Default::default()
        },
        ..Default::default()
    };
    let e = to_docx(
        &corpus("escaneado"),
        &dir.path().join("x.docx"),
        &opts,
        &JobContext::silent(),
    )
    .unwrap_err();
    assert!(matches!(e, Error::Unsupported(m) if m.contains("OCR")));
    assert!(!dir.path().join("x.docx").exists());
}

#[test]
fn pptx_has_one_slide_per_page_with_images() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("a.pptx");
    to_pptx(
        &corpus("articulo-4"),
        &p,
        &PptxOptions::default(),
        &JobContext::silent(),
    )
    .unwrap();
    assert!(read_part(&p, "ppt/slides/slide2.xml").is_some());
    assert!(read_part(&p, "ppt/slides/slide3.xml").is_none());
    assert!(read_part(&p, "ppt/slides/slide1.xml").unwrap().contains("<p:pic>"));
}

#[test]
fn xlsx_without_tables_falls_back_to_text() {
    let dir = tempfile::tempdir().unwrap();
    let x = dir.path().join("c.xlsx");
    let r = to_xlsx(&corpus("carta-1"), &x, &XlsxOptions::default(), &JobContext::silent()).unwrap();
    assert!(r.warnings.iter().any(|w| w.contains("ninguna tabla")));
    assert!(
        read_part(&x, "xl/worksheets/sheet1.xml")
            .unwrap()
            .contains("Estimado cliente")
    );
    let per_page = XlsxOptions {
        sheet_per_page: true,
        ..Default::default()
    };
    to_xlsx(&corpus("tabla-1"), &x, &per_page, &JobContext::silent()).unwrap();
    assert_well_formed(&x);
}

#[test]
fn invalid_input_is_rejected_and_cancel_works() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = JobContext::silent();
    let same = to_docx(&corpus("carta-1"), &corpus("carta-1"), &DocxOptions::default(), &ctx);
    assert!(matches!(same, Err(Error::InvalidInput(_))));
    let bad_pages = DocxOptions {
        common: CommonOptions {
            pages: "zz".into(),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(to_docx(&corpus("carta-1"), &dir.path().join("a.docx"), &bad_pages, &ctx).is_err());
    let bad_dpi = DocxOptions {
        image_dpi: 5.0,
        ..Default::default()
    };
    assert!(to_docx(&corpus("carta-1"), &dir.path().join("a.docx"), &bad_dpi, &ctx).is_err());
    let missing = to_docx(
        Path::new("no-existe.pdf"),
        &dir.path().join("a.docx"),
        &DocxOptions::default(),
        &ctx,
    );
    assert!(missing.is_err());
    let token = CancelToken::new();
    token.cancel();
    let cancelled = JobContext::new(&NoProgress, token);
    let r = to_docx(
        &corpus("carta-1"),
        &dir.path().join("c.docx"),
        &DocxOptions::default(),
        &cancelled,
    );
    assert!(matches!(r, Err(Error::Cancelled)));
    assert!(!dir.path().join("c.docx").exists());
}
