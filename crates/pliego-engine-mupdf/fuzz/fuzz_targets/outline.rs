#![no_main]
//! Leer los marcadores de un PDF arbitrario no debe tumbar el proceso.
//! Un PDF pasa por el recorrido propio de `outline.rs`, que tolera marcadores sin /Title (fallo de
//! `walk_outlines` en mupdf-rs 0.8.0); solo los no PDF usan `outlines()`. Entra en `tools/fuzz.sh`.
use libfuzzer_sys::fuzz_target;
use pliego_core::{EngineDocument, PdfEngine};
use pliego_engine_mupdf::MupdfEngine;

fuzz_target!(|data: &[u8]| {
    if let Ok(doc) = MupdfEngine.open_bytes(data, None) {
        let _ = doc.outline();
    }
});
