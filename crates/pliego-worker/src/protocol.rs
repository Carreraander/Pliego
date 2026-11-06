//! Protocolo entre la app y el worker: mensajes por stdin/stdout.
//!
//! Cada mensaje es un *marco*: longitud `u32` little-endian + bytes. Las peticiones y respuestas son
//! JSON; los datos binarios (PDF en memoria, píxeles RGBA) viajan en un marco binario aparte que sigue
//! inmediatamente al marco JSON que lo anuncia.

use std::io::{self, Read, Write};

use pliego_core::{DocInfo, Error, Layer, OutlineItem, PageLink, PageSize, PageWord, SearchHit, TileRequest};
use serde::{Deserialize, Serialize};

/// Máximo de un marco JSON.
pub const MAX_JSON_FRAME: usize = 64 * 1024 * 1024;
/// Máximo de un marco binario (PDF o imagen).
pub const MAX_BINARY_FRAME: usize = 1 << 30;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Request {
    OpenPath {
        path: String,
        password: Option<String>,
    },
    /// Sigue un marco binario con el PDF.
    OpenBytes {
        password: Option<String>,
    },
    Info,
    PageCount,
    PageSize {
        page: u32,
    },
    RenderPage {
        page: u32,
        zoom: f32,
    },
    RenderTile(TileRequest),
    PageText {
        page: u32,
    },
    SearchPage {
        page: u32,
        needle: String,
        max_hits: u32,
    },
    Outline,
    /// Palabras de la página con su caja (capa de texto del visor).
    PageWords {
        page: u32,
    },
    /// Enlaces de la página a otra página o a una dirección.
    PageLinks {
        page: u32,
    },
    /// Capas del documento (añadido; no cambia las peticiones anteriores).
    Layers,
    SetLayer {
        id: i32,
        visible: bool,
    },
    Shutdown,
    /// Solo con `PLIEGO_WORKER_DEBUG=1`: el worker termina de golpe (pruebas de recuperación).
    DebugCrash,
    /// Solo con `PLIEGO_WORKER_DEBUG=1`: el worker deja de responder (pruebas de tiempo de espera).
    DebugHang,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Response {
    Ok,
    Info(DocInfo),
    Count(u32),
    Size(PageSize),
    /// Sigue un marco binario con `width * height * 4` bytes RGBA.
    Image {
        width: u32,
        height: u32,
    },
    Text(String),
    Hit(SearchHit),
    Outline(Vec<OutlineItem>),
    Words(Vec<PageWord>),
    Links(Vec<PageLink>),
    Layers(Vec<Layer>),
    Err(WireError),
}

/// Error serializable (los `std::io::Error` no lo son).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WireError {
    Io(String),
    Pdf(String),
    Password,
    InvalidInput(String),
    Unsupported(String),
    Cancelled,
    Other(String),
}

impl From<&Error> for WireError {
    fn from(e: &Error) -> Self {
        match e {
            Error::Io { source, .. } => WireError::Io(source.to_string()),
            Error::Pdf(s) => WireError::Pdf(s.clone()),
            Error::Password => WireError::Password,
            Error::InvalidInput(s) => WireError::InvalidInput(s.clone()),
            Error::Unsupported(s) => WireError::Unsupported(s.clone()),
            Error::Cancelled => WireError::Cancelled,
            Error::Other(s) => WireError::Other(s.clone()),
        }
    }
}

impl From<WireError> for Error {
    fn from(e: WireError) -> Self {
        match e {
            WireError::Io(s) => Error::Io {
                path: None,
                source: io::Error::other(s),
            },
            WireError::Pdf(s) => Error::Pdf(s),
            WireError::Password => Error::Password,
            WireError::InvalidInput(s) => Error::InvalidInput(s),
            WireError::Unsupported(s) => Error::Unsupported(s),
            WireError::Cancelled => Error::Cancelled,
            WireError::Other(s) => Error::Other(s),
        }
    }
}

pub fn write_frame(w: &mut impl Write, data: &[u8]) -> io::Result<()> {
    let len =
        u32::try_from(data.len()).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "marco demasiado grande"))?;
    w.write_all(&len.to_le_bytes())?;
    w.write_all(data)?;
    w.flush()
}

/// Lee un marco. `Ok(None)` indica fin limpio del flujo entre mensajes.
pub fn read_frame(r: &mut impl Read, max: usize) -> io::Result<Option<Vec<u8>>> {
    let mut len = [0u8; 4];
    match r.read_exact(&mut len) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let len = u32::from_le_bytes(len) as usize;
    if len > max {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "marco mayor que el máximo permitido",
        ));
    }
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)?;
    Ok(Some(buf))
}

pub fn write_json(w: &mut impl Write, msg: &impl Serialize) -> io::Result<()> {
    let data = serde_json::to_vec(msg).map_err(io::Error::other)?;
    write_frame(w, &data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_roundtrip() {
        let mut buf = Vec::new();
        write_frame(&mut buf, b"hola").unwrap();
        write_json(&mut buf, &Request::PageSize { page: 3 }).unwrap();
        let mut r = buf.as_slice();
        assert_eq!(read_frame(&mut r, 100).unwrap().unwrap(), b"hola");
        let req: Request = serde_json::from_slice(&read_frame(&mut r, 100).unwrap().unwrap()).unwrap();
        assert!(matches!(req, Request::PageSize { page: 3 }));
        assert!(read_frame(&mut r, 100).unwrap().is_none());
    }

    #[test]
    fn oversized_and_truncated_frames_fail() {
        let mut buf = Vec::new();
        write_frame(&mut buf, &[0u8; 50]).unwrap();
        assert!(read_frame(&mut buf.as_slice(), 10).is_err());
        buf.truncate(20);
        assert!(read_frame(&mut buf.as_slice(), 100).is_err());
    }
}
