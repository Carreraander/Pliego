#![no_main]
//! Extraer texto y buscar en la página 0 de un PDF arbitrario no debe tumbar el proceso.
use libfuzzer_sys::fuzz_target;
use pliego_core::{EngineDocument, PdfEngine};
use pliego_engine::DefaultEngine;

fuzz_target!(|data: &[u8]| {
    if let Ok(doc) = DefaultEngine.open_bytes(data, None) {
        let _ = doc.page_text(0);
        let _ = doc.search_page(0, "a", 5);
    }
});
