//! Gestor de modelos `tessdata_fast`: descarga bajo demanda a la carpeta de usuario con verificación obligatoria
//! de tamaño y SHA-256. Un modelo que no está en el catálogo (o sin huella) no se descarga.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use pliego_core::{Error, JobContext, Result};
use sha2::{Digest, Sha256};

use crate::tessdata;

/// Etiqueta de `tessdata_fast` de la que proceden las huellas del catálogo.
pub const TESSDATA_FAST_TAG: &str = "4.1.0";

/// Modelo conocido: código de idioma, tamaño y huella SHA-256 del `.traineddata` de `tessdata_fast`.
#[derive(Debug, Clone, Copy)]
pub struct ModelSpec {
    pub code: &'static str,
    pub size_bytes: u64,
    pub sha256: &'static str,
}

/// Catálogo verificado. Se amplía añadiendo filas con huella comprobada.
pub const CATALOG: &[ModelSpec] = &[];

pub fn find_spec(code: &str) -> Option<&'static ModelSpec> {
    CATALOG.iter().find(|m| m.code == code)
}

/// Códigos que se pueden descargar.
pub fn downloadable_languages() -> Vec<&'static str> {
    CATALOG.iter().map(|m| m.code).collect()
}

fn url_for(code: &str) -> String {
    format!("https://github.com/tesseract-ocr/tessdata_fast/raw/{TESSDATA_FAST_TAG}/{code}.traineddata")
}

/// Descarga el modelo `code` (p. ej. `"spa"`) a la carpeta de usuario (`%LOCALAPPDATA%\Pliego\tessdata`).
/// Si ya está instalado y coincide con la huella, no hace nada. Devuelve la ruta del archivo.
pub fn download_language(code: &str, ctx: &JobContext) -> Result<PathBuf> {
    let dir = tessdata::user_dir()
        .ok_or_else(|| Error::Other("no se pudo determinar la carpeta de modelos del usuario".into()))?;
    download_language_to(code, &dir, &url_for(code), ctx)
}

/// Igual que [`download_language`] con carpeta y URL explícitas (pruebas y espejos propios).
pub fn download_language_to(code: &str, dir: &Path, url: &str, ctx: &JobContext) -> Result<PathBuf> {
    let spec = find_spec(code).ok_or_else(|| {
        Error::Unsupported(format!(
            "el idioma «{code}» no está en el catálogo de modelos verificados ({})",
            downloadable_languages().join(", ")
        ))
    })?;
    let dest = dir.join(format!("{code}.traineddata"));
    if verify_file(&dest, spec).is_ok() {
        return Ok(dest);
    }
    fetch_verified(url, &dest, spec.size_bytes, spec.sha256, ctx)?;
    Ok(dest)
}

fn verify_file(path: &Path, spec: &ModelSpec) -> Result<()> {
    let len = fs::metadata(path).map_err(|e| Error::io_at(path, e))?.len();
    if len == spec.size_bytes && file_sha256(path)?.eq_ignore_ascii_case(spec.sha256) {
        Ok(())
    } else {
        Err(Error::Other("modelo distinto del esperado".into()))
    }
}

/// Descarga `url` a `dest` pasando por `dest.part` y solo lo renombra si tamaño y SHA-256 coinciden.
pub fn fetch_verified(url: &str, dest: &Path, size: u64, sha256: &str, ctx: &JobContext) -> Result<()> {
    let dir = dest.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(dir).map_err(|e| Error::io_at(dir, e))?;
    let part = dest.with_extension("part");
    let resp = ureq::get(url)
        .call()
        .map_err(|e| Error::Other(format!("no se pudo descargar el modelo: {e}")))?;
    let mut body = resp.into_body().into_reader();
    let mut out = File::create(&part).map_err(|e| Error::io_at(&part, e))?;
    let (mut have, mut since) = (0u64, 0u64);
    let mut buf = vec![0u8; 256 * 1024];
    let result = (|| -> Result<()> {
        loop {
            let n = body
                .read(&mut buf)
                .map_err(|e| Error::Other(format!("se interrumpió la descarga: {e}")))?;
            if n == 0 {
                break;
            }
            have += n as u64;
            if have > size {
                return Err(Error::Other("el modelo descargado es mayor de lo esperado".into()));
            }
            out.write_all(&buf[..n]).map_err(|e| Error::io_at(&part, e))?;
            since += n as u64;
            if since >= 1 << 20 {
                since = 0;
                ctx.step(have, size, "Descargando el modelo")?;
            }
        }
        out.flush().map_err(|e| Error::io_at(&part, e))
    })();
    drop(out);
    if let Err(e) = result {
        let _ = fs::remove_file(&part);
        return Err(e);
    }
    ctx.step(size, size, "Comprobando la descarga")?;
    let ok = have == size && file_sha256(&part)?.eq_ignore_ascii_case(sha256);
    if !ok {
        let _ = fs::remove_file(&part);
        return Err(Error::Other(
            "la descarga del modelo no coincide con la huella esperada; se ha descartado, vuelve a intentarlo".into(),
        ));
    }
    fs::rename(&part, dest).map_err(|e| Error::io_at(dest, e))
}

fn file_sha256(path: &Path) -> Result<String> {
    let mut f = File::open(path).map_err(|e| Error::io_at(path, e))?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf).map_err(|e| Error::io_at(path, e))?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader};
    use std::net::TcpListener;

    use super::*;

    /// Servidor HTTP mínimo que atiende una petición.
    fn serve(data: Vec<u8>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/m.traineddata", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            let mut r = BufReader::new(s.try_clone().unwrap());
            loop {
                let mut line = String::new();
                r.read_line(&mut line).unwrap();
                if line.trim().is_empty() {
                    break;
                }
            }
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                data.len()
            );
            s.write_all(head.as_bytes()).unwrap();
            s.write_all(&data).unwrap();
        });
        url
    }

    fn sha(data: &[u8]) -> String {
        Sha256::digest(data).iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn good_download_is_installed() {
        let data = vec![7u8; 300_000];
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("x.traineddata");
        fetch_verified(
            &serve(data.clone()),
            &dest,
            data.len() as u64,
            &sha(&data),
            &JobContext::silent(),
        )
        .unwrap();
        assert_eq!(fs::read(&dest).unwrap(), data);
        assert!(!dest.with_extension("part").exists());
    }

    #[test]
    fn wrong_hash_is_discarded() {
        let data = vec![7u8; 1000];
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("x.traineddata");
        let err = fetch_verified(
            &serve(data.clone()),
            &dest,
            1000,
            &sha(b"otra cosa"),
            &JobContext::silent(),
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("no coincide"), "{err}");
        assert!(!dest.exists() && !dest.with_extension("part").exists());
    }

    #[test]
    fn oversized_download_is_rejected() {
        let data = vec![1u8; 5000];
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("x.traineddata");
        let err = fetch_verified(&serve(data), &dest, 1000, &sha(b""), &JobContext::silent())
            .unwrap_err()
            .to_string();
        assert!(err.contains("mayor"), "{err}");
        assert!(!dest.exists());
    }

    #[test]
    fn unknown_language_is_refused_without_network() {
        let dir = tempfile::tempdir().unwrap();
        let err = download_language_to("zzz", dir.path(), "http://127.0.0.1:1/x", &JobContext::silent())
            .unwrap_err()
            .to_string();
        assert!(err.contains("no está en el catálogo"), "{err}");
    }
}
