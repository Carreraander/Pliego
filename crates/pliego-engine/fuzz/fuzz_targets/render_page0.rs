#![no_main]
//! Renderizar la página 0 (y una tesela) de un PDF arbitrario no debe tumbar el proceso.
use libfuzzer_sys::fuzz_target;
use pliego_core::{EngineDocument, PdfEngine, TileRequest};
use pliego_engine::DefaultEngine;

fuzz_target!(|data: &[u8]| {
    if let Ok(doc) = DefaultEngine.open_bytes(data, None) {
        let _ = doc.render_page(0, 0.25);
        let _ = doc.render_tile(TileRequest { page: 0, zoom: 0.5, col: 0, row: 0, size: 128 });
    }
});
