#![no_main]
//! Abrir un PDF arbitrario y leer su información no debe tumbar el proceso.
use libfuzzer_sys::fuzz_target;
use pliego_core::{EngineDocument, PdfEngine};
use pliego_engine::DefaultEngine;

fuzz_target!(|data: &[u8]| {
    if let Ok(doc) = DefaultEngine.open_bytes(data, None) {
        let _ = doc.info();
        let _ = doc.page_count();
    }
});
