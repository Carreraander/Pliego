//! Proceso `pliego-worker`: abre UN documento (lopdf + PDFium) y atiende peticiones por stdin/stdout.
//! Si un PDF hostil tumba este proceso, la app sigue viva y lo relanza (ver `WorkerEngine`).

// En Windows sin subsistema de consola: evita el conhost.exe colgado del proceso (stdio va por tuberías).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::io::{self, BufReader, BufWriter, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};

use pliego_core::{EngineDocument, Error, PdfEngine, Result};
use pliego_engine::{DefaultDocument, DefaultEngine};
use pliego_worker::protocol::{
    MAX_BINARY_FRAME, MAX_JSON_FRAME, Request, Response, WireError, read_frame, write_frame, write_json,
};

fn main() {
    // PDFium se carga de `pdfium.dll` junto al ejecutable o de PLIEGO_PDFIUM_DLL. Un error aquí no impide arrancar:
    // dibujar y extraer texto fallan con un mensaje claro, y el resto de peticiones siguen funcionando.
    if let Err(e) = pliego_engine::configure_from_environment() {
        eprintln!("pliego-worker: {e}");
    }
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut input = BufReader::new(stdin.lock());
    let mut output = BufWriter::new(stdout.lock());
    let debug = std::env::var_os("PLIEGO_WORKER_DEBUG").is_some();
    let mut doc: Option<DefaultDocument> = None;

    loop {
        let frame = match read_frame(&mut input, MAX_JSON_FRAME) {
            Ok(Some(f)) => f,
            // Fin de flujo o marco corrupto: la app ya no está o el canal es inválido.
            Ok(None) | Err(_) => return,
        };
        let req: Request = match serde_json::from_slice(&frame) {
            Ok(r) => r,
            Err(e) => {
                let _ = write_json(&mut output, &Response::Err(WireError::InvalidInput(e.to_string())));
                continue;
            }
        };
        let bytes = if matches!(req, Request::OpenBytes { .. }) {
            match read_frame(&mut input, MAX_BINARY_FRAME) {
                Ok(Some(b)) => Some(b),
                _ => return,
            }
        } else {
            None
        };
        match req {
            Request::Shutdown => return,
            Request::DebugCrash if debug => std::process::abort(),
            Request::DebugHang if debug => loop {
                std::thread::park();
            },
            _ => {}
        }
        let result = catch_unwind(AssertUnwindSafe(|| handle(&mut doc, req, bytes)))
            .unwrap_or_else(|_| Err(Error::Pdf("el motor falló al procesar el documento".into())));
        let sent = match result {
            Ok((resp, binary)) => write_json(&mut output, &resp).and_then(|_| match binary {
                Some(b) => write_frame(&mut output, &b),
                None => Ok(()),
            }),
            Err(e) => write_json(&mut output, &Response::Err(WireError::from(&e))),
        };
        if sent.and_then(|_| output.flush()).is_err() {
            return;
        }
    }
}

fn handle(
    doc: &mut Option<DefaultDocument>,
    req: Request,
    bytes: Option<Vec<u8>>,
) -> Result<(Response, Option<Vec<u8>>)> {
    let image = |img: pliego_core::RgbaImage| {
        (
            Response::Image {
                width: img.width,
                height: img.height,
            },
            Some(img.data),
        )
    };
    match req {
        Request::OpenPath { path, password } => {
            *doc = None;
            *doc = Some(DefaultEngine.open_auto(path.as_ref(), password.as_deref())?);
            Ok((Response::Ok, None))
        }
        Request::OpenBytes { password } => {
            *doc = None;
            let bytes = bytes.ok_or_else(|| Error::InvalidInput("falta el contenido del PDF".into()))?;
            *doc = Some(DefaultEngine.open_bytes(&bytes, password.as_deref())?);
            Ok((Response::Ok, None))
        }
        Request::SetLayer { id, visible } => {
            let d = doc
                .as_mut()
                .ok_or_else(|| Error::InvalidInput("no hay ningún documento abierto".into()))?;
            d.set_layer_visible_mut(id, visible)?;
            Ok((Response::Ok, None))
        }
        other => {
            let d = doc
                .as_ref()
                .ok_or_else(|| Error::InvalidInput("no hay ningún documento abierto".into()))?;
            Ok(match other {
                Request::Info => (Response::Info(d.info()?), None),
                Request::PageCount => (Response::Count(d.page_count()?), None),
                Request::PageSize { page } => (Response::Size(d.page_size(page)?), None),
                Request::RenderPage { page, zoom } => image(d.render_page(page, zoom)?),
                Request::RenderTile(t) => image(d.render_tile(t)?),
                Request::PageText { page } => (Response::Text(d.page_text(page)?), None),
                Request::SearchPage { page, needle, max_hits } => {
                    (Response::Hit(d.search_page(page, &needle, max_hits)?), None)
                }
                Request::Outline => (Response::Outline(d.outline()?), None),
                Request::Layers => (Response::Layers(d.layers()?), None),
                _ => return Err(Error::InvalidInput("petición no válida".into())),
            })
        }
    }
}
