use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use pliego_core::{Error, TileRequest};

use super::*;

fn corpus() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/basic/hola.pdf")
}

fn req(page: u32) -> TileRequest {
    TileRequest {
        page,
        zoom: 1.0,
        col: 0,
        row: 0,
        size: 512,
    }
}

#[test]
fn open_info_sizes_and_close() {
    let svc = DocumentService::new();
    let doc = svc.open_document(&corpus(), None).unwrap();
    let info = svc.document_info(doc).unwrap();
    assert!(info.page_count >= 1);
    assert_eq!(svc.page_sizes(doc).unwrap().len(), info.page_count as usize);
    svc.close_document(doc).unwrap();
    assert!(svc.document_info(doc).is_err());
    assert!(svc.close_document(doc).is_err());
}

#[test]
fn open_missing_file_fails() {
    let svc = DocumentService::new();
    assert!(svc.open_document(Path::new("no-existe.pdf"), None).is_err());
}

#[test]
fn tile_is_png_and_cached() {
    let svc = DocumentService::new();
    let doc = svc.open_document(&corpus(), None).unwrap();
    let a = svc.tile(doc, req(0)).unwrap();
    assert_eq!(&a[..8], b"\x89PNG\r\n\x1a\n");
    assert_eq!(svc.cached_tiles(), 1);
    let b = svc.tile(doc, req(0)).unwrap();
    assert!(Arc::ptr_eq(&a, &b), "la segunda petición sale de la caché");
}

#[test]
fn tile_out_of_range_is_error() {
    let svc = DocumentService::new();
    let doc = svc.open_document(&corpus(), None).unwrap();
    assert!(matches!(svc.tile(doc, req(999)), Err(Error::InvalidInput(_))));
}

#[test]
fn tile_from_uri_works() {
    let svc = DocumentService::new();
    let doc = svc.open_document(&corpus(), None).unwrap();
    let png = svc.tile_from_uri(&format!("pliego://tile/{}/0/1/0/0", doc.0)).unwrap();
    assert_eq!(&png[..4], b"\x89PNG");
    assert!(svc.tile_from_uri("pliego://tile/999/0/1/0/0").is_err());
}

#[test]
fn bump_revision_invalidates_tiles_and_emits_event() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = events.clone();
    let svc = DocumentService::new().on_document_changed(move |e| sink.lock().unwrap().push(e));
    let doc = svc.open_document(&corpus(), None).unwrap();
    let a = svc.tile(doc, req(0)).unwrap();
    assert_eq!(svc.bump_revision(doc).unwrap(), 1);
    let b = svc.tile(doc, req(0)).unwrap();
    assert!(!Arc::ptr_eq(&a, &b), "tras el cambio se vuelve a renderizar");
    assert_eq!(
        *events.lock().unwrap(),
        vec![DocumentChanged {
            doc_id: doc,
            revision: 1
        }]
    );
}

#[test]
fn search_text_and_outline() {
    let svc = DocumentService::new();
    let doc = svc.open_document(&corpus(), None).unwrap();
    let text = svc.page_text(doc, 0).unwrap();
    let word = text
        .split_whitespace()
        .next()
        .expect("el PDF de prueba tiene texto")
        .to_owned();
    let hits = svc.search(doc, &word, 50).unwrap();
    assert!(!hits.is_empty() && hits[0].page == 0);
    assert!(svc.search(doc, "   ", 50).unwrap().is_empty());
    assert!(svc.outline(doc).is_ok());
}
