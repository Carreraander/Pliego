//! Descarga de modelos bajo demanda, con reanudación y verificación obligatoria de tamaño y SHA-256.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use pliego_core::{Error, JobContext, Result};
use sha2::{Digest, Sha256};

use crate::models::ModelSpec;

/// Ruta del modelo si ya está descargado y tiene el tamaño esperado.
pub fn installed_path(spec: &ModelSpec, dir: &Path) -> Option<PathBuf> {
    let path = dir.join(spec.file_name);
    (fs::metadata(&path).ok()?.len() == spec.size_bytes).then_some(path)
}

/// Descarga un modelo del catálogo a `dir`. Sin SHA-256 en el catálogo no se descarga nada.
pub fn download_model(spec: &ModelSpec, dir: &Path, ctx: &JobContext) -> Result<PathBuf> {
    if let Some(done) = installed_path(spec, dir) {
        return Ok(done);
    }
    let sha = spec
        .sha256
        .ok_or_else(|| Error::Unsupported(format!("el modelo {} no tiene huella verificada", spec.id)))?;
    fs::create_dir_all(dir).map_err(|e| Error::io_at(dir, e))?;
    download_file(spec.url, &dir.join(spec.file_name), sha, spec.size_bytes, ctx)?;
    Ok(dir.join(spec.file_name))
}

/// Descarga `url` a `dest` (vía `dest.part`, reanudable) y solo lo renombra si el tamaño y el SHA-256 coinciden.
pub fn download_file(url: &str, dest: &Path, sha256: &str, size: u64, ctx: &JobContext) -> Result<()> {
    let part = dest.with_extension("part");
    let mut have = fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
    if have > size {
        fs::remove_file(&part).map_err(|e| Error::io_at(&part, e))?;
        have = 0;
    }

    if have < size {
        let mut req = ureq::get(url);
        if have > 0 {
            req = req.header("Range", format!("bytes={have}-"));
        }
        let resp = req
            .call()
            .map_err(|e| Error::Other(format!("no se pudo descargar el modelo: {e}")))?;
        // 206 = el servidor reanuda; 200 = envía todo desde el principio.
        let resume = have > 0 && resp.status().as_u16() == 206;
        let mut out = if resume {
            OpenOptions::new().append(true).open(&part)
        } else {
            have = 0;
            File::create(&part)
        }
        .map_err(|e| Error::io_at(&part, e))?;

        let mut body = resp.into_body().into_reader();
        let mut buf = vec![0u8; 256 * 1024];
        let mut since_report = 0u64;
        loop {
            let n = body
                .read(&mut buf)
                .map_err(|e| Error::Other(format!("se interrumpió la descarga: {e}")))?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n]).map_err(|e| write_error(&part, e))?;
            have += n as u64;
            since_report += n as u64;
            if since_report >= 1 << 20 {
                since_report = 0;
                // Si se cancela, el .part queda para reanudar más tarde.
                ctx.step(have, size, "Descargando el modelo")?;
            }
        }
        out.flush().map_err(|e| write_error(&part, e))?;
    }

    ctx.step(size, size, "Comprobando la descarga")?;
    let got = fs::metadata(&part).map_err(|e| Error::io_at(&part, e))?.len();
    let digest = if got == size { Some(file_sha256(&part)?) } else { None };
    if digest.as_deref().is_none_or(|d| !d.eq_ignore_ascii_case(sha256)) {
        // Archivo corrupto o distinto del esperado: se descarta para no reanudar sobre datos malos.
        let _ = fs::remove_file(&part);
        return Err(Error::Other(
            "la descarga del modelo no coincide con la huella esperada; se ha descartado, vuelve a intentarlo".into(),
        ));
    }
    fs::rename(&part, dest).map_err(|e| Error::io_at(dest, e))
}

fn write_error(path: &Path, e: std::io::Error) -> Error {
    if e.kind() == std::io::ErrorKind::StorageFull {
        Error::Other("no queda espacio en el disco para el modelo; libera espacio y vuelve a intentarlo".into())
    } else {
        Error::io_at(path, e)
    }
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

    /// Servidor HTTP mínimo con soporte de `Range`, atiende `n` peticiones.
    fn serve(data: Vec<u8>, n: usize, honour_range: bool) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/m.gguf", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            for _ in 0..n {
                let (mut s, _) = listener.accept().unwrap();
                let mut r = BufReader::new(s.try_clone().unwrap());
                let mut start = 0usize;
                loop {
                    let mut line = String::new();
                    r.read_line(&mut line).unwrap();
                    if line.trim().is_empty() {
                        break;
                    }
                    if let Some(v) = line.to_ascii_lowercase().strip_prefix("range: bytes=") {
                        start = v.trim().trim_end_matches('-').parse().unwrap();
                    }
                }
                let (code, body) = if honour_range && start > 0 {
                    ("206 Partial Content", &data[start..])
                } else {
                    ("200 OK", &data[..])
                };
                let head = format!(
                    "HTTP/1.1 {code}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                s.write_all(head.as_bytes()).unwrap();
                s.write_all(body).unwrap();
            }
        });
        url
    }

    fn sha(data: &[u8]) -> String {
        Sha256::digest(data).iter().map(|b| format!("{b:02x}")).collect()
    }

    fn payload() -> Vec<u8> {
        (0..3_000_000u32).map(|i| (i % 251) as u8).collect()
    }

    #[test]
    fn downloads_and_verifies() {
        let data = payload();
        let url = serve(data.clone(), 1, true);
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("m.gguf");
        download_file(&url, &dest, &sha(&data), data.len() as u64, &JobContext::silent()).unwrap();
        assert_eq!(fs::read(&dest).unwrap(), data);
        assert!(!dest.with_extension("part").exists());
    }

    #[test]
    fn resumes_a_partial_download() {
        let data = payload();
        let url = serve(data.clone(), 1, true);
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("m.gguf");
        fs::write(dest.with_extension("part"), &data[..1_000_000]).unwrap();
        download_file(&url, &dest, &sha(&data), data.len() as u64, &JobContext::silent()).unwrap();
        assert_eq!(fs::read(&dest).unwrap(), data);
    }

    #[test]
    fn restarts_if_server_ignores_range() {
        let data = payload();
        let url = serve(data.clone(), 1, false);
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("m.gguf");
        fs::write(dest.with_extension("part"), &data[..1_000_000]).unwrap();
        download_file(&url, &dest, &sha(&data), data.len() as u64, &JobContext::silent()).unwrap();
        assert_eq!(fs::read(&dest).unwrap(), data);
    }

    #[test]
    fn wrong_hash_is_rejected_and_discarded() {
        let data = payload();
        let url = serve(data.clone(), 1, true);
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("m.gguf");
        let bad = sha(b"otra cosa");
        let err = download_file(&url, &dest, &bad, data.len() as u64, &JobContext::silent()).unwrap_err();
        assert!(err.to_string().contains("huella"));
        assert!(!dest.exists() && !dest.with_extension("part").exists());
    }

    #[test]
    fn cancel_keeps_the_partial_file() {
        let data = payload();
        let url = serve(data.clone(), 1, true);
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("m.gguf");
        let ctx = JobContext::silent();
        ctx.cancel.cancel();
        let err = download_file(&url, &dest, &sha(&data), data.len() as u64, &ctx).unwrap_err();
        assert!(matches!(err, Error::Cancelled));
        assert!(dest.with_extension("part").exists());
    }

    #[test]
    fn model_without_hash_is_refused() {
        let mut spec = crate::catalog()[0].clone();
        spec.sha256 = None;
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            download_model(&spec, dir.path(), &JobContext::silent()),
            Err(Error::Unsupported(_))
        ));
    }
}
