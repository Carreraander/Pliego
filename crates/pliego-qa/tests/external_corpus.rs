//! Corpus externo (descargado con `tools/fetch_corpus.py`): 0 caídas y 0 bloqueos.
//! Es opt-in por lentitud (~6 min en debug): `PLIEGO_CORPUS=1 cargo test --release -p pliego-qa`.
//! Sin la variable, o si el directorio no existe, el test se omite.

use std::path::PathBuf;
use std::process::Command;

#[test]
fn corpus_externo_sin_caidas_ni_bloqueos() {
    if std::env::var_os("PLIEGO_CORPUS").is_none() {
        eprintln!("omitido: define PLIEGO_CORPUS=1 para ejecutarlo");
        return;
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/external");
    if !root.is_dir() {
        eprintln!("omitido: ejecuta `python tools/fetch_corpus.py` para descargar el corpus");
        return;
    }
    let out = Command::new(env!("CARGO_BIN_EXE_corpus-runner"))
        .arg(&root)
        .arg("--known")
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/known-issues.txt"))
        .output()
        .expect("no se pudo ejecutar corpus-runner");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "corpus con caídas o bloqueos:\n{text}");
}
