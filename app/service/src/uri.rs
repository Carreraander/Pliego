//! Análisis de URIs `pliego://tile/<doc>/<page>/<zoom>/<col>/<row>`.

use pliego_core::{Error, Result, TileRequest};

use crate::DocId;

/// Tamaño de tesela por defecto en píxeles.
pub const TILE_SIZE: u32 = 512;

/// Interpreta la ruta de un URI de tesela. Acepta con o sin el prefijo `pliego://tile/` y con o sin `/` inicial.
/// La página se numera desde 0 y el zoom es un factor (1.0 = 72 ppp).
pub fn parse_tile_uri(uri: &str) -> Result<(DocId, TileRequest)> {
    let path = uri.strip_prefix("pliego://").unwrap_or(uri);
    let path = path.trim_start_matches('/');
    let path = path.strip_prefix("tile/").ok_or_else(|| bad(uri))?;
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() != 5 {
        return Err(bad(uri));
    }
    let num = |s: &str| s.parse::<u32>().map_err(|_| bad(uri));
    let zoom: f32 = parts[2].parse().map_err(|_| bad(uri))?;
    if !zoom.is_finite() || !(0.01..=32.0).contains(&zoom) {
        return Err(Error::InvalidInput(format!("zoom fuera de rango: {zoom}")));
    }
    let doc = parts[0].parse::<u64>().map_err(|_| bad(uri))?;
    Ok((
        DocId(doc),
        TileRequest {
            page: num(parts[1])?,
            zoom,
            col: num(parts[3])?,
            row: num(parts[4])?,
            size: TILE_SIZE,
        },
    ))
}

fn bad(uri: &str) -> Error {
    Error::InvalidInput(format!("URI de tesela no válida: {uri}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_and_short_forms() {
        let (doc, req) = parse_tile_uri("pliego://tile/3/7/1.5/2/4").unwrap();
        assert_eq!(doc, DocId(3));
        assert_eq!((req.page, req.col, req.row, req.size), (7, 2, 4, TILE_SIZE));
        assert_eq!(req.zoom, 1.5);
        assert!(parse_tile_uri("/tile/3/7/1/0/0").is_ok());
    }

    #[test]
    fn rejects_bad_uris() {
        for u in [
            "pliego://tile/3/7/1/0",
            "pliego://x/3/7/1/0/0",
            "pliego://tile/a/7/1/0/0",
            "pliego://tile/3/7/nan/0/0",
            "pliego://tile/3/7/100/0/0",
            "pliego://tile/3/-1/1/0/0",
        ] {
            assert!(parse_tile_uri(u).is_err(), "{u}");
        }
    }
}
