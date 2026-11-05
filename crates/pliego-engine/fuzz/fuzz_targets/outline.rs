#![no_main]
//! Leer los marcadores de un PDF arbitrario no debe tumbar el proceso.
//! Un PDF pasa por el recorrido propio de `PdfDocument::outlines` (en `pliego-pdf`), que tolera marcadores sin /Title (fallo de
//! `walk_outlines` de la capa anterior (MuPDF)); solo los no PDF usan `outlines()`. Entra en `tools/fuzz.sh`.
use libfuzzer_sys::fuzz_target;
use pliego_core::{EngineDocument, PdfEngine};
use pliego_engine::DefaultEngine;

fuzz_target!(|data: &[u8]| {
    if let Ok(doc) = DefaultEngine.open_bytes(data, None) {
        let _ = doc.outline();
    }
});
