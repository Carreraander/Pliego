use std::path::PathBuf;
use std::time::Duration;

use pliego_core::{EngineDocument, Error, PdfEngine, TileRequest};
use pliego_engine::DefaultEngine;
use pliego_worker::{WorkerConfig, WorkerEngine};

fn hola() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/basic/hola.pdf")
}

fn config() -> WorkerConfig {
    WorkerConfig::new(env!("CARGO_BIN_EXE_pliego-worker"))
}

/// Worker de prueba. PDFium se carga de `PLIEGO_PDFIUM_DLL`, que el worker hereda del entorno.
fn engine() -> WorkerEngine {
    WorkerEngine::new(config())
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
    let m = DefaultEngine.open(&hola(), None).unwrap();
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
    let mut cfg = config();
    cfg.timeout = Duration::from_millis(1500);
    cfg.debug = true;
    let w = WorkerEngine::new(cfg).open(&hola(), None).unwrap();
    w.debug_request_hang();
    // Un cuelgue no se reintenta (suele ser determinista): la petición falla y el worker se relanza.
    assert!(w.page_count().is_err());
    assert_eq!(w.restarts(), 1);
    assert_eq!(w.page_count().unwrap(), 2, "la siguiente petición reabre el documento");
}

fn hostile(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/worker")
        .join(name)
}

/// PDF con un patrón de mosaico de paso 0,001: un motor ingenuo no termina nunca. El worker lo corta por tiempo.
#[test]
fn tiling_pattern_hang_is_contained() {
    let mut cfg = WorkerConfig::new(env!("CARGO_BIN_EXE_pliego-worker"));
    cfg.timeout = Duration::from_secs(2);
    let w = WorkerEngine::new(cfg).open(&hostile("tiling-hang.pdf"), None).unwrap();
    let t = std::time::Instant::now();
    assert!(w.render_page(0, 1.0).is_err());
    assert!(t.elapsed() < Duration::from_secs(8), "se corta cerca del tiempo límite");
    assert_eq!(w.restarts(), 1);
    assert!(w.take_recovered());
    assert_eq!(w.page_count().unwrap(), 1, "el documento sigue abierto tras el corte");
}

/// Con `PLIEGO_HOSTILE_DIR=<carpeta con PDFs>` renderiza cada uno a través del worker (p. ej. los de
/// pdf.js que fallan en proceso: bomb_giant, bug_jpx, tiling-pattern-tiny-steps*). Ninguno puede
/// tumbar el proceso de pruebas ni dejar el worker inutilizable.
#[test]
fn external_hostile_files_are_contained() {
    let Some(dir) = std::env::var_os("PLIEGO_HOSTILE_DIR") else {
        return;
    };
    let mut cfg = WorkerConfig::new(env!("CARGO_BIN_EXE_pliego-worker"));
    cfg.timeout = Duration::from_secs(5);
    let engine = WorkerEngine::new(cfg);
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "pdf") {
            continue;
        }
        let Ok(doc) = engine.open(&path, None) else {
            println!("{}: rechazado al abrir", path.display());
            continue;
        };
        let res = doc.render_page(0, 0.5);
        println!(
            "{}: {} (reinicios: {})",
            path.display(),
            if res.is_ok() { "ok" } else { "error contenido" },
            doc.restarts()
        );
        assert!(doc.info().is_ok(), "{} deja el worker inutilizable", path.display());
    }
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

/// PDF hallado por el fuzzing: un marcador sin /Title tumbaba `Document::outlines` del motor anterior.
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

/// PDF de una página con un cuadrado negro dentro de la capa «Fondo» (grupo de contenido opcional).
fn layered_pdf() -> Vec<u8> {
    let content = "/OC /L0 BDC 0 g 0 0 100 100 re f EMC";
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R /OCProperties << /OCGs [5 0 R] /D << /ON [5 0 R] /Order [5 0 R] >> >> >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Contents 4 0 R /Resources << /Properties << /L0 5 0 R >> >> >>".to_string(),
        format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()),
        "<< /Type /OCG /Name (Fondo) >>".to_string(),
    ];
    let mut out = b"%PDF-1.5\n".to_vec();
    let mut offsets = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
    for off in offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objs.len() + 1
        )
        .as_bytes(),
    );
    out
}

fn dark_pixels(img: &pliego_core::RgbaImage) -> usize {
    img.data.iter().step_by(4).filter(|&&r| r < 128).count()
}

#[test]
fn layers_can_be_hidden_and_survive_a_worker_restart() {
    let doc = engine().open_bytes(&layered_pdf(), None).unwrap();
    let layers = doc.layers().unwrap();
    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].name, "Fondo");
    assert!(layers[0].visible);
    let id = layers[0].id;
    let req = TileRequest {
        page: 0,
        zoom: 1.0,
        col: 0,
        row: 0,
        size: 128,
    };
    assert!(dark_pixels(&doc.render_tile(req).unwrap()) > 5000);

    doc.set_layer_visible(id, false).unwrap();
    assert!(!doc.layers().unwrap()[0].visible);
    assert_eq!(
        dark_pixels(&doc.render_tile(req).unwrap()),
        0,
        "la caché no debe devolver la tesela anterior"
    );

    // Si el worker se cae, al relanzarlo la capa sigue oculta.
    doc.kill_worker();
    assert_eq!(dark_pixels(&doc.render_tile(req).unwrap()), 0);
    assert!(!doc.layers().unwrap()[0].visible);

    doc.set_layer_visible(id, true).unwrap();
    assert!(dark_pixels(&doc.render_tile(req).unwrap()) > 5000);
}

#[test]
fn unknown_layer_is_rejected_and_plain_pdfs_have_none() {
    let doc = engine().open(&hola(), None).unwrap();
    assert!(doc.layers().unwrap().is_empty());
    assert!(matches!(
        doc.set_layer_visible(12345, false),
        Err(Error::InvalidInput(_))
    ));
}
