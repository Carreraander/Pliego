use std::path::{Path, PathBuf};

use pliego_core::JobContext;
use pliego_index::Index;

fn corpus(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus")
        .join(rel)
}

fn folder(dir: &Path) -> PathBuf {
    let docs = dir.join("docs");
    std::fs::create_dir_all(docs.join("sub")).unwrap();
    std::fs::copy(corpus("basic/hola.pdf"), docs.join("hola.pdf")).unwrap();
    std::fs::copy(corpus("compare/base.pdf"), docs.join("sub/contrato.pdf")).unwrap();
    std::fs::copy(corpus("organize/a.pdf"), docs.join("a.pdf")).unwrap();
    std::fs::write(docs.join("roto.pdf"), b"no soy un pdf").unwrap();
    docs
}

#[test]
fn indexes_searches_with_snippets_and_updates_incrementally() {
    let dir = tempfile::tempdir().unwrap();
    let docs = folder(dir.path());
    let idx = Index::open(&dir.path().join("indice")).unwrap();
    let ctx = JobContext::silent();

    let r = idx.add_folder(&docs, &ctx).unwrap();
    assert!(r.summary.starts_with("3 indexados, 0 sin cambios"), "{}", r.summary);
    assert_eq!(r.warnings.len(), 1, "el PDF roto se avisa: {:?}", r.warnings);

    let hits = idx.search("testigos", 10).unwrap();
    assert_eq!(hits.len(), 1);
    assert!(hits[0].path.ends_with("contrato.pdf"));
    assert_eq!(hits[0].page, 0);
    let (a, b) = hits[0].highlights[0];
    assert_eq!(hits[0].snippet[a..b].to_lowercase(), "testigos");

    // Salta a la segunda página.
    let hits = idx.search("Segunda", 10).unwrap();
    let hola: Vec<_> = hits.iter().filter(|h| h.path.ends_with("hola.pdf")).collect();
    assert_eq!((hola.len(), hola[0].page), (1, 1));

    // Segunda pasada: nada cambió.
    let r = idx.add_folder(&docs, &ctx).unwrap();
    assert!(r.summary.starts_with("0 indexados, 3 sin cambios"), "{}", r.summary);

    // Un archivo desaparece y otro cambia.
    std::fs::remove_file(docs.join("sub/contrato.pdf")).unwrap();
    std::fs::copy(corpus("organize/b.pdf"), docs.join("hola.pdf")).unwrap();
    let r = idx.add_folder(&docs, &ctx).unwrap();
    assert!(r.summary.contains("1 olvidados"), "{}", r.summary);
    assert!(idx.search("testigos", 10).unwrap().is_empty());
    assert!(
        idx.search("Segunda", 10)
            .unwrap()
            .iter()
            .all(|h| !h.path.ends_with("hola.pdf"))
    );
    assert_eq!(idx.search("B", 10).unwrap().len(), 2);
}

#[test]
fn invalid_query_is_a_friendly_error() {
    let dir = tempfile::tempdir().unwrap();
    let idx = Index::open(dir.path()).unwrap();
    assert!(idx.search("(", 5).is_err());
}

#[test]
fn reopened_index_keeps_data() {
    let dir = tempfile::tempdir().unwrap();
    let docs = folder(dir.path());
    let ip = dir.path().join("indice");
    Index::open(&ip)
        .unwrap()
        .add_folder(&docs, &JobContext::silent())
        .unwrap();
    let again = Index::open(&ip).unwrap();
    assert_eq!(again.search("Pliego", 10).unwrap().len(), 2);
}

/// Objetivo del flujo: 1.000 PDFs indexados y búsqueda en < 200 ms.
/// `cargo test -p pliego-index --release -- --ignored --nocapture`
#[test]
#[ignore = "benchmark: tarda; se ejecuta a mano (acr12)"]
fn thousand_pdfs_search_under_200ms() {
    let dir = tempfile::tempdir().unwrap();
    let docs = dir.path().join("docs");
    std::fs::create_dir_all(&docs).unwrap();
    for i in 0..1000 {
        let src = if i % 2 == 0 {
            "compare/base.pdf"
        } else {
            "basic/hola.pdf"
        };
        std::fs::copy(corpus(src), docs.join(format!("doc{i:04}.pdf"))).unwrap();
    }
    let idx = Index::open(&dir.path().join("indice")).unwrap();
    let t = std::time::Instant::now();
    let r = idx.add_folder(&docs, &JobContext::silent()).unwrap();
    println!("indexado de 1000 PDFs: {:?} — {}", t.elapsed(), r.summary);
    let t = std::time::Instant::now();
    let hits = idx.search("testigos", 20).unwrap();
    let dt = t.elapsed();
    println!("búsqueda: {dt:?}, {} resultados", hits.len());
    assert!(dt.as_millis() < 200);
}

/// Perfil de un solo PDF: separa abrir, cargar página y extraer texto, repitiendo 5 veces con el mismo archivo.
/// `cargo test -p pliego-index --release -- --ignored --nocapture profile_single_pdf`
#[test]
#[ignore = "diagnóstico: se ejecuta a mano (acr12)"]
fn profile_single_pdf() {
    use pliego_pdf::{Document, TextFlags};
    for name in ["compare/base.pdf", "basic/hola.pdf"] {
        let bytes = std::fs::read(corpus(name)).unwrap();
        for round in 0..5 {
            let t = std::time::Instant::now();
            let doc = Document::from_bytes(&bytes, "application/pdf").unwrap();
            let t_open = t.elapsed();
            let t = std::time::Instant::now();
            let page = doc.load_page(0).unwrap();
            let t_load = t.elapsed();
            let t = std::time::Instant::now();
            let tp = page.to_text_page(TextFlags::empty()).unwrap();
            let t_tp = t.elapsed();
            let t = std::time::Instant::now();
            let _ = tp.to_text().unwrap();
            let t_text = t.elapsed();
            println!("{name} #{round}: abrir {t_open:?}, cargar {t_load:?}, textpage {t_tp:?}, texto {t_text:?}");
        }
    }
}
