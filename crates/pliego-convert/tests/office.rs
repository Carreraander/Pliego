//! Office a PDF. Necesita Microsoft Office o LibreOffice: sin ninguno, las pruebas de conversión se omiten.

use pliego_convert::{OfficeBackend, office_backend, office_to_pdf};
use pliego_core::{Error, JobContext};

#[test]
fn unknown_extension_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("a.xyz");
    std::fs::write(&f, "x").unwrap();
    let e = office_to_pdf(&f, &dir.path().join("o.pdf"), &JobContext::silent()).unwrap_err();
    assert!(matches!(e, Error::InvalidInput(_)), "{e}");
}

#[test]
fn missing_file_is_io_error() {
    let dir = tempfile::tempdir().unwrap();
    let e = office_to_pdf(
        &dir.path().join("no.docx"),
        &dir.path().join("o.pdf"),
        &JobContext::silent(),
    )
    .unwrap_err();
    assert!(matches!(e, Error::Io { .. }));
}

#[test]
fn converts_csv_when_a_backend_exists() {
    let Some(backend) = office_backend() else {
        eprintln!("sin Office ni LibreOffice: prueba omitida");
        return;
    };
    eprintln!("backend: {backend:?}");
    let dir = tempfile::tempdir().unwrap();
    let csv = dir.path().join("datos.csv");
    std::fs::write(&csv, "Nombre,Total\nManzanas,12\nPeras,7\n").unwrap();
    let pdf = dir.path().join("datos.pdf");
    office_to_pdf(&csv, &pdf, &JobContext::silent()).unwrap();
    let text_out = dir.path().join("datos.txt");
    pliego_convert::pdf_to_text(
        &pdf,
        None,
        &pliego_core::PageRange::all(),
        &text_out,
        &Default::default(),
        &JobContext::silent(),
    )
    .unwrap();
    let t = std::fs::read_to_string(text_out).unwrap();
    assert!(t.contains("Manzanas") && t.contains("Peras"), "{t}");
    if let OfficeBackend::LibreOffice { path } = backend {
        assert!(path.is_file());
    }
}
