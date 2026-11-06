//! Preparación del PDF y apertura en PDFium para imprimir. Necesitan la biblioteca de PDFium
//! (`PLIEGO_PDFIUM_DLL=<ruta absoluta>`); sin ella se omiten con un aviso, como las de `pliego-pdf`.

use std::path::PathBuf;

use pliego_core::Error;
use pliego_pdf::print::PrintDocument;
use pliego_pdf::{Document, Encryption, Permission, WriteOptions, pdfium_available, pdfium_init_from_environment};
use pliego_printer::prepare::prepare;

fn corpus(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus")
        .join(name)
}

fn pdfium() -> bool {
    if pdfium_available() {
        return true;
    }
    match pdfium_init_from_environment() {
        Ok(_) => true,
        Err(e) => {
            eprintln!("OMITIDA: no hay PDFium ({e}); define PLIEGO_PDFIUM_DLL");
            false
        }
    }
}

#[test]
fn plain_pdf_opens_and_reports_page_sizes() {
    if !pdfium() {
        return;
    }
    let p = prepare(&corpus("basic/hola.pdf"), None).unwrap();
    assert!(p.password.is_none());
    let doc = PrintDocument::open(p.bytes, None).unwrap();
    assert!(doc.page_count() >= 1);
    let (w, h) = doc.page_size(0).unwrap();
    assert!(w > 100.0 && h > 100.0, "{w}x{h}");
    assert!(doc.page_size(doc.page_count()).is_err());
    assert!(doc.page_size(-1).is_err());
}

#[test]
fn form_fields_are_flattened_in_a_copy_and_the_file_is_untouched() {
    if !pdfium() {
        return;
    }
    let path = corpus("organize/form1.pdf");
    let before = std::fs::read(&path).unwrap();
    let p = prepare(&path, None).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let flat = Document::from_bytes_with_password(&p.bytes, "application/pdf", None).unwrap();
    assert!(
        !flat.has_acro_form().unwrap(),
        "la copia para imprimir aún tiene campos"
    );
    assert!(PrintDocument::open(p.bytes, None).unwrap().page_count() >= 1);
}

fn encrypted(dir: &tempfile::TempDir, permissions: Permission) -> PathBuf {
    let doc = Document::open(corpus("basic/hola.pdf")).unwrap();
    let mut o = WriteOptions::default();
    o.set_encryption(Encryption::Aes256)
        .set_user_password("abc")
        .set_owner_password("dueno")
        .set_permissions(permissions);
    let out = dir.path().join("cifrado.pdf");
    doc.save_with_options(&out, &o).unwrap();
    out
}

#[test]
fn encrypted_pdf_needs_the_password_and_keeps_it_for_pdfium() {
    if !pdfium() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let path = encrypted(&dir, Permission::all());
    assert!(matches!(prepare(&path, None), Err(Error::Password)));
    assert!(matches!(prepare(&path, Some("mal")), Err(Error::Password)));
    let p = prepare(&path, Some("abc")).unwrap();
    assert_eq!(p.password.as_deref(), Some("abc"));
    assert!(matches!(
        PrintDocument::open(p.bytes.clone(), None),
        Err(Error::Password)
    ));
    assert!(PrintDocument::open(p.bytes, Some("abc")).unwrap().page_count() >= 1);
}

#[test]
fn a_pdf_that_forbids_printing_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = encrypted(&dir, Permission::COPY);
    let err = prepare(&path, Some("abc")).err().expect("debía rechazarse");
    assert!(err.to_string().contains("no permite imprimirlo"), "{err}");
}
