//! El ejecutor del corpus funciona sobre el corpus pequeño incluido en el repositorio.

use std::path::PathBuf;
use std::process::Command;

#[test]
fn corpus_basico_pasa() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/basic");
    let out = Command::new(env!("CARGO_BIN_EXE_corpus-runner"))
        .arg(&dir)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{text}");
    assert!(text.contains("caídas/bloqueos: 0"), "{text}");
}
