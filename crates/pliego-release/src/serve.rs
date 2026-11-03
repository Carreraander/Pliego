//! Servidor HTTP estático mínimo para probar actualizaciones en local.
//! No es para producción: en el VPS se sirve el mismo árbol con Caddy (HTTPS automático).

use std::path::{Component, Path, PathBuf};

use anyhow::{Result, anyhow};
use tiny_http::{Header, Response, Server};

pub fn serve(root: &Path, addr: &str) -> Result<()> {
    let root = std::fs::canonicalize(root)?;
    let server = Server::http(addr).map_err(|e| anyhow!("no se pudo escuchar en {addr}: {e}"))?;
    println!("Sirviendo {} en http://{addr} (Ctrl+C para parar)", root.display());
    for req in server.incoming_requests() {
        let url = req.url().to_owned();
        let status;
        let result = match resolve(&root, &url) {
            Some(path) if path.is_file() => {
                status = 200;
                let file = std::fs::File::open(&path)?;
                let ctype = content_type(&path);
                let header = Header::from_bytes("Content-Type", ctype).expect("cabecera válida");
                let nocache = Header::from_bytes("Cache-Control", "no-cache").expect("cabecera válida");
                req.respond(Response::from_file(file).with_header(header).with_header(nocache))
            }
            _ => {
                status = 404;
                req.respond(Response::from_string("no encontrado").with_status_code(404))
            }
        };
        println!("{status} GET {url}");
        if let Err(e) = result {
            eprintln!("error al responder: {e}");
        }
    }
    Ok(())
}

/// Ruta dentro de `root` para una URL, o `None` si intenta salir de ella.
pub fn resolve(root: &Path, url: &str) -> Option<PathBuf> {
    let path = url.split(['?', '#']).next()?;
    let mut out = root.to_path_buf();
    for part in Path::new(path.trim_start_matches('/')).components() {
        match part {
            Component::Normal(p) => out.push(p),
            Component::CurDir => {}
            _ => return None,
        }
    }
    if path.contains('\\') || path.contains("%") {
        return None;
    }
    Some(out)
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("json") => "application/json",
        Some("exe") | Some("msi") | Some("paq") => "application/octet-stream",
        Some("md") | Some("txt") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_blocks_traversal() {
        let root = Path::new("/srv/act");
        assert_eq!(
            resolve(root, "/v1/stable/manifest.json?x=1").unwrap(),
            root.join("v1/stable/manifest.json")
        );
        assert!(resolve(root, "/../etc/passwd").is_none());
        assert!(resolve(root, "/v1/%2e%2e/x").is_none());
        assert!(resolve(root, "/v1\\..\\x").is_none());
    }
}
