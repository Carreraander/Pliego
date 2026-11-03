#![no_main]
//! Leer los marcadores de un PDF arbitrario no debe tumbar el proceso.
//! CONOCIDO: con mupdf-rs 0.8.0 un marcador sin /Title provoca un NULL en `walk_outlines`
//! (ver docs de W02); mientras no se corrija, este objetivo cae y no entra en `tools/fuzz.sh`.
use libfuzzer_sys::fuzz_target;
use pliego_core::{EngineDocument, PdfEngine};
use pliego_engine_mupdf::MupdfEngine;

fuzz_target!(|data: &[u8]| {
    if let Ok(doc) = MupdfEngine.open_bytes(data, None) {
        let _ = doc.outline();
    }
});
