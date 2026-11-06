use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, OnceLock};

use pliego_core::{Error, TileRequest};
use pliego_worker::WorkerConfig;

use super::*;

fn corpus() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/basic/hola.pdf")
}

/// Ejecutable `pliego-worker`: el de `PLIEGO_WORKER`, o el de `target/<perfil>` (compilándolo si no existe).
fn worker_exe() -> PathBuf {
    static EXE: OnceLock<PathBuf> = OnceLock::new();
    EXE.get_or_init(|| {
        if let Some(p) = std::env::var_os("PLIEGO_WORKER") {
            return PathBuf::from(p);
        }
        let test_exe = std::env::current_exe().unwrap();
        let dir = test_exe.parent().unwrap().parent().unwrap(); // target/<perfil>
        let exe = dir.join(format!("pliego-worker{}", std::env::consts::EXE_SUFFIX));
        if !exe.exists() {
            let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
            let mut cmd = Command::new(cargo);
            cmd.args(["build", "-p", "pliego-worker"]);
            if dir.file_name().is_some_and(|n| n == "release") {
                cmd.arg("--release");
            }
            assert!(cmd.status().unwrap().success(), "no se pudo compilar pliego-worker");
        }
        exe
    })
    .clone()
}

/// Configuración del worker de pruebas: límite por petición holgado, por si la máquina de CI va justa de CPU.
fn config() -> WorkerConfig {
    let mut c = WorkerConfig::new(worker_exe());
    c.timeout = std::time::Duration::from_secs(120);
    c
}

fn service() -> DocumentService {
    DocumentService::new(config())
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
    let _serial = crate::test_support::serial();
    let svc = service();
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
    let _serial = crate::test_support::serial();
    assert!(service().open_document(Path::new("no-existe.pdf"), None).is_err());
}

#[test]
fn tile_is_png_and_cached() {
    let _serial = crate::test_support::serial();
    let svc = service();
    let doc = svc.open_document(&corpus(), None).unwrap();
    let a = svc.tile(doc, req(0)).unwrap();
    assert_eq!(&a[..8], b"\x89PNG\r\n\x1a\n");
    assert_eq!(svc.cached_tiles(), 1);
    assert_eq!(svc.tile(doc, req(0)).unwrap(), a);
    assert_eq!(svc.cached_tiles(), 1, "la segunda petición sale de la caché");
}

#[test]
fn tile_out_of_range_is_error() {
    let _serial = crate::test_support::serial();
    let svc = service();
    let doc = svc.open_document(&corpus(), None).unwrap();
    assert!(svc.tile(doc, req(999)).is_err());
}

#[test]
fn tile_from_uri_works() {
    let _serial = crate::test_support::serial();
    let svc = service();
    let doc = svc.open_document(&corpus(), None).unwrap();
    let png = svc.tile_from_uri(&format!("pliego://tile/{}/0/1/0/0", doc.0)).unwrap();
    assert_eq!(&png[..4], b"\x89PNG");
    assert!(matches!(
        svc.tile_from_uri("pliego://tile/999/0/1/0/0"),
        Err(Error::InvalidInput(_))
    ));
}

#[test]
fn bump_revision_invalidates_tiles_and_emits_event() {
    let _serial = crate::test_support::serial();
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = events.clone();
    let svc = DocumentService::new(config()).on_document_changed(move |e| sink.lock().unwrap().push(e));
    let doc = svc.open_document(&corpus(), None).unwrap();
    svc.tile(doc, req(0)).unwrap();
    assert_eq!(svc.cached_tiles(), 1);
    assert_eq!(svc.bump_revision(doc).unwrap(), 1);
    assert_eq!(
        svc.cached_tiles(),
        0,
        "las teselas de la revisión anterior se descartan"
    );
    assert_eq!(svc.revision(doc).unwrap(), 1);
    assert_eq!(
        *events.lock().unwrap(),
        vec![DocumentChanged {
            doc_id: doc,
            revision: 1
        }]
    );
}

#[test]
fn reload_bytes_replaces_content() {
    let _serial = crate::test_support::serial();
    let svc = service();
    let doc = svc.open_document(&corpus(), None).unwrap();
    let bytes = std::fs::read(corpus()).unwrap();
    assert_eq!(svc.reload_bytes(doc, &bytes).unwrap(), 1);
    assert!(svc.tile(doc, req(0)).is_ok());
}

#[test]
fn search_text_and_outline() {
    let _serial = crate::test_support::serial();
    let svc = service();
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

#[test]
fn page_words_come_with_boxes() {
    let _serial = crate::test_support::serial();
    let svc = service();
    let doc = svc.open_document(&corpus(), None).unwrap();
    let words = svc.page_words(doc, 0).unwrap();
    assert!(!words.is_empty());
    assert!(
        words
            .iter()
            .all(|w| !w.text.is_empty() && w.rect[0] < w.rect[2] && w.rect[1] < w.rect[3])
    );
    assert!(svc.page_words(doc, 999).is_err());
    assert!(svc.page_links(doc, 0).is_ok());
    assert!(svc.page_links(doc, 999).is_err());
}

#[test]
fn layers_of_a_plain_document_are_empty_and_unknown_layers_fail() {
    let _serial = crate::test_support::serial();
    let svc = service();
    let doc = svc.open_document(&corpus(), None).unwrap();
    assert!(svc.layers(doc).unwrap().is_empty());
    assert!(svc.set_layer_visible(doc, 99999, false).is_err());
    assert_eq!(svc.revision(doc).unwrap(), 0, "un fallo no debe repintar");
}
