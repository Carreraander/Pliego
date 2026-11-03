use std::path::PathBuf;
use std::time::Duration;

use pliego_core::{EngineDocument, Error, PdfEngine, TileRequest};
use pliego_engine_mupdf::MupdfEngine;
use pliego_worker::{WorkerConfig, WorkerEngine};

fn hola() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/basic/hola.pdf")
}

fn engine() -> WorkerEngine {
    WorkerEngine::new(WorkerConfig::new(env!("CARGO_BIN_EXE_pliego-worker")))
}

fn tile() -> TileRequest {
    TileRequest {
        page: 0,
        zoom: 0.5,
        col: 0,
        row: 0,
        size: 128,
    }
}

#[test]
fn matches_in_process_engine() {
    let w = engine().open(&hola(), None).unwrap();
    let m = MupdfEngine.open(&hola(), None).unwrap();
    assert_eq!(w.info().unwrap(), m.info().unwrap());
    assert_eq!(w.page_size(0).unwrap(), m.page_size(0).unwrap());
    assert_eq!(w.page_text(0).unwrap(), m.page_text(0).unwrap());
    assert_eq!(w.render_page(0, 0.5).unwrap(), m.render_page(0, 0.5).unwrap());
    assert_eq!(w.render_tile(tile()).unwrap(), m.render_tile(tile()).unwrap());
    assert_eq!(w.outline().unwrap(), m.outline().unwrap());
    assert_eq!(
        w.search_page(1, "Pliego", 5).unwrap(),
        m.search_page(1, "Pliego", 5).unwrap()
    );
}

#[test]
fn open_bytes_and_errors() {
    let e = engine();
    let bytes = std::fs::read(hola()).unwrap();
    assert_eq!(e.open_bytes(&bytes, None).unwrap().page_count().unwrap(), 2);
    assert!(matches!(e.open_bytes(b"basura", None), Err(Error::Pdf(_))));
    assert!(matches!(
        e.open(&hola().with_file_name("nada.pdf"), None),
        Err(Error::Io { .. })
    ));
    let w = e.open(&hola(), None).unwrap();
    assert!(matches!(w.page_size(99), Err(Error::InvalidInput(_))));
    assert_eq!(w.restarts(), 0, "un error de la operación no reinicia el worker");
}

#[test]
fn recovers_when_worker_is_killed() {
    let w = engine().open(&hola(), None).unwrap();
    assert_eq!(w.page_count().unwrap(), 2);
    w.kill_worker();
    // La siguiente petición relanza el worker, reabre el documento y responde.
    assert!(w.page_text(0).unwrap().contains("Hola Pliego"));
    assert_eq!(w.restarts(), 1);
    assert!(w.take_recovered());
    assert!(!w.take_recovered());
}

#[test]
fn hung_worker_times_out_and_recovers() {
    let mut cfg = WorkerConfig::new(env!("CARGO_BIN_EXE_pliego-worker"));
    cfg.timeout = Duration::from_millis(1500);
    cfg.debug = true;
    let w = WorkerEngine::new(cfg).open(&hola(), None).unwrap();
    w.debug_request_hang();
    assert_eq!(w.page_count().unwrap(), 2);
    assert!(w.restarts() >= 1);
}

#[test]
fn corrupt_pdf_does_not_crash_host() {
    let e = engine();
    let mut bytes = std::fs::read(hola()).unwrap();
    for (i, b) in bytes.iter_mut().enumerate().skip(200) {
        *b = b.wrapping_add((i % 251) as u8);
    }
    // Puede abrirse o no, pero el proceso de pruebas sigue vivo.
    if let Ok(d) = e.open_bytes(&bytes, None) {
        let _ = d.page_count();
        let _ = d.render_page(0, 1.0);
    }
}

#[test]
fn tile_cache_hits_and_revision_invalidation() {
    let e = engine();
    let w = e.open(&hola(), None).unwrap();
    let a = w.render_tile_cached(tile()).unwrap();
    assert_eq!(e.cache().len(), 1);
    let b = w.render_tile_cached(tile()).unwrap();
    assert!(
        std::sync::Arc::ptr_eq(&a, &b),
        "segunda petición servida desde la caché"
    );
    w.set_revision(1);
    assert!(
        e.cache().is_empty(),
        "al cambiar la revisión se descartan las teselas viejas"
    );
    drop(w);
    assert_eq!(e.cache().bytes(), 0);
}

#[test]
fn reload_bytes_changes_content() {
    let e = engine();
    let w = e.open(&hola(), None).unwrap();
    let bytes = std::fs::read(hola()).unwrap();
    w.reload_bytes(&bytes, 7).unwrap();
    assert_eq!(w.revision(), 7);
    assert_eq!(w.page_count().unwrap(), 2);
    assert!(w.reload_bytes(b"no es pdf", 8).is_err());
    assert_eq!(w.revision(), 7);
    assert_eq!(
        w.page_count().unwrap(),
        2,
        "si falla la recarga se conserva el documento anterior"
    );
}

/// PDF hallado por el fuzzing: un marcador sin /Title tumbaba `Document::outlines` de mupdf-rs 0.8.0.
/// Ya no ocurre (recorrido propio de marcadores), y el documento sigue usable sin reinicios.
#[test]
fn outline_without_title_is_handled() {
    let hostile = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/worker/outline-null-title.pdf");
    let w = engine().open(&hostile, None).unwrap();
    let outline = w.outline().expect("los marcadores sin /Title ya no abortan el motor");
    assert!(!outline.is_empty());
    assert_eq!(w.restarts(), 0);
    assert_eq!(w.page_count().unwrap(), 2);
    assert!(w.page_text(0).unwrap().contains("Hola"));
}

/// Solo Windows: el job object limita la memoria del worker, así que un render enorme falla sin afectar a la app.
#[cfg(windows)]
#[test]
fn job_object_memory_limit_stops_huge_render() {
    let mut cfg = WorkerConfig::new(env!("CARGO_BIN_EXE_pliego-worker"));
    cfg.memory_limit = Some(256 * 1024 * 1024);
    let w = WorkerEngine::new(cfg).open(&hola(), None).unwrap();
    assert!(w.render_page(0, 1.0).is_ok(), "un render normal cabe en el límite");
    // 595x842 pt a zoom 30 son ~1,5 GB de píxeles: no cabe en 256 MB.
    assert!(w.render_page(0, 30.0).is_err());
    assert_eq!(w.page_count().unwrap(), 2, "la app y el documento siguen utilizables");
}

/// Medida de rendimiento (objetivo: < 30 ms de media por tesela de 512 px en páginas de texto).
/// Uso: `cargo test -p pliego-worker --release --test worker tile_perf -- --ignored --nocapture`
#[test]
#[ignore = "medida manual de rendimiento"]
fn tile_perf() {
    let w = engine().open(&hola(), None).unwrap();
    let req = |col| TileRequest {
        page: 0,
        zoom: 1.5,
        col,
        row: 0,
        size: 512,
    };
    for _ in 0..20 {
        let _ = w.render_tile(req(0)).unwrap(); // calentamiento (tesela cacheada tras la primera)
    }
    // Sin caché: tesela distinta en cada iteración variando el zoom.
    let n = 200;
    let t = std::time::Instant::now();
    for i in 0..n {
        let r = TileRequest {
            zoom: 1.0 + i as f32 * 0.001,
            ..req(0)
        };
        let _ = w.render_tile(r).unwrap();
    }
    let ms = t.elapsed().as_secs_f64() * 1000.0 / n as f64;
    println!("tesela 512 px sin caché a través del worker: {ms:.2} ms de media ({n} teselas)");
}
