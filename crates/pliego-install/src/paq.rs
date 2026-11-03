//! Formato `.paq`: `tar` comprimido con zstd. La primera entrada es `files.json` (versión y, por archivo, ruta,
//! tamaño y SHA-256); el resto son los archivos de la carpeta `app-<v>\`.
//!
//! La extracción es estricta: solo archivos y carpetas normales, rutas relativas sin `..`, nada que no esté en
//! `files.json`, y cada archivo se comprueba (tamaño y SHA-256) mientras se escribe.

use crate::error::{Error, Result};
use crate::layout::{Layout, check_version};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};

pub const MANIFEST_NAME: &str = "files.json";
/// Tope de `files.json` para que un paquete hostil no agote la memoria.
const MAX_MANIFEST_BYTES: u64 = 16 * 1024 * 1024;
const ZSTD_LEVEL: i32 = 19;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    /// Ruta relativa con `/`.
    pub path: String,
    pub size: u64,
    /// SHA-256 en hexadecimal.
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub version: String,
    pub files: Vec<FileEntry>,
}

/// Comprueba que una ruta del paquete es relativa, usa `/` y no sale de la carpeta.
pub fn safe_rel_path(p: &str) -> Result<PathBuf> {
    let bad = || Error::UnsafeEntry(p.to_string());
    if p.is_empty() || p.contains('\\') || p.contains(':') || p.contains('\0') {
        return Err(bad());
    }
    let mut out = PathBuf::new();
    for c in Path::new(p).components() {
        match c {
            Component::Normal(s) => out.push(s),
            _ => return Err(bad()),
        }
    }
    if out.as_os_str().is_empty() {
        Err(bad())
    } else {
        Ok(out)
    }
}

fn io_err(context: &str, e: io::Error) -> Error {
    Error::io(context.to_string(), e)
}

fn hash_reader(r: &mut impl Read) -> io::Result<(u64, String)> {
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut n = 0u64;
    loop {
        let k = r.read(&mut buf)?;
        if k == 0 {
            break;
        }
        h.update(&buf[..k]);
        n += k as u64;
    }
    Ok((n, hex(&h.finalize())))
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn walk(dir: &Path, prefix: &str, out: &mut Vec<(String, PathBuf)>) -> Result<()> {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .map_err(|e| io_err("no se pudo leer la carpeta a empaquetar", e))?
        .collect::<io::Result<_>>()
        .map_err(|e| io_err("no se pudo leer la carpeta a empaquetar", e))?;
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let name = e.file_name().to_string_lossy().into_owned();
        let rel = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        let ty = e
            .file_type()
            .map_err(|e| io_err("no se pudo leer el tipo de archivo", e))?;
        if ty.is_symlink() {
            return Err(Error::UnsafeEntry(format!("{rel} (enlace simbólico)")));
        } else if ty.is_dir() {
            walk(&e.path(), &rel, out)?;
        } else if ty.is_file() && rel != MANIFEST_NAME {
            out.push((rel, e.path()));
        }
    }
    Ok(())
}

/// Calcula el `files.json` de una carpeta (sin incluir `files.json` ni `healthy`/`launch-failures`).
pub fn build_manifest(dir: &Path, version: &str) -> Result<Manifest> {
    check_version(version)?;
    let mut list = Vec::new();
    walk(dir, "", &mut list)?;
    let mut files = Vec::with_capacity(list.len());
    for (rel, path) in list {
        if rel == "healthy" || rel == "launch-failures" {
            continue;
        }
        let mut f = File::open(&path).map_err(|e| io_err(&format!("no se pudo abrir {rel}"), e))?;
        let (size, sha256) = hash_reader(&mut f).map_err(|e| io_err(&format!("no se pudo leer {rel}"), e))?;
        files.push(FileEntry {
            path: rel,
            size,
            sha256,
        });
    }
    Ok(Manifest {
        version: version.to_string(),
        files,
    })
}

/// Empaqueta `dir` en `out` (`.paq`). Devuelve el manifiesto.
pub fn pack(dir: &Path, version: &str, out: &Path) -> Result<Manifest> {
    let manifest = build_manifest(dir, version)?;
    let tmp = out.with_extension("paq.tmp");
    let file = File::create(&tmp).map_err(|e| io_err("no se pudo crear el paquete", e))?;
    let mut enc = zstd::Encoder::new(file, ZSTD_LEVEL).map_err(|e| io_err("zstd", e))?;
    enc.long_distance_matching(true).map_err(|e| io_err("zstd", e))?;
    let mut tar = tar::Builder::new(enc);
    tar.mode(tar::HeaderMode::Deterministic);

    let json = serde_json::to_vec_pretty(&manifest).map_err(|e| Error::Corrupt(e.to_string()))?;
    let mut h = tar::Header::new_gnu();
    h.set_size(json.len() as u64);
    h.set_mode(0o644);
    tar.append_data(&mut h, MANIFEST_NAME, json.as_slice())
        .map_err(|e| io_err("no se pudo escribir files.json", e))?;
    for f in &manifest.files {
        let mut src =
            File::open(dir.join(safe_rel_path(&f.path)?)).map_err(|e| io_err("no se pudo abrir un archivo", e))?;
        let mut h = tar::Header::new_gnu();
        h.set_size(f.size);
        h.set_mode(0o644);
        tar.append_data(&mut h, &f.path, &mut src)
            .map_err(|e| io_err(&format!("no se pudo empaquetar {}", f.path), e))?;
    }
    let enc = tar
        .into_inner()
        .map_err(|e| io_err("no se pudo cerrar el paquete", e))?;
    let mut file = enc.finish().map_err(|e| io_err("zstd", e))?;
    file.flush().map_err(|e| io_err("no se pudo cerrar el paquete", e))?;
    drop(file);
    fs::rename(&tmp, out).map_err(|e| io_err("no se pudo guardar el paquete", e))?;
    Ok(manifest)
}

fn open_tar<R: Read>(r: R) -> Result<tar::Archive<zstd::Decoder<'static, io::BufReader<R>>>> {
    let dec = zstd::Decoder::new(r).map_err(|e| Error::Corrupt(format!("zstd: {e}")))?;
    Ok(tar::Archive::new(dec))
}

fn read_manifest_entry<R: Read>(entry: &mut tar::Entry<'_, R>) -> Result<Manifest> {
    let path = entry
        .path()
        .map_err(|e| Error::Corrupt(e.to_string()))?
        .to_string_lossy()
        .into_owned();
    if path != MANIFEST_NAME {
        return Err(Error::Corrupt("la primera entrada no es files.json".into()));
    }
    if entry.size() > MAX_MANIFEST_BYTES {
        return Err(Error::Corrupt("files.json demasiado grande".into()));
    }
    let mut buf = Vec::new();
    entry.read_to_end(&mut buf).map_err(|e| Error::Corrupt(e.to_string()))?;
    let m: Manifest = serde_json::from_slice(&buf).map_err(|e| Error::Corrupt(format!("files.json: {e}")))?;
    check_version(&m.version)?;
    let mut seen = HashSet::new();
    for f in &m.files {
        safe_rel_path(&f.path)?;
        if !seen.insert(&f.path) {
            return Err(Error::Corrupt(format!("{} aparece dos veces", f.path)));
        }
    }
    Ok(m)
}

/// Lee solo el `files.json` de un paquete.
pub fn read_manifest(paq: impl Read) -> Result<Manifest> {
    let mut ar = open_tar(paq)?;
    let mut entries = ar.entries().map_err(|e| Error::Corrupt(e.to_string()))?;
    let mut first = entries
        .next()
        .ok_or_else(|| Error::Corrupt("paquete vacío".into()))?
        .map_err(|e| Error::Corrupt(e.to_string()))?;
    read_manifest_entry(&mut first)
}

pub fn read_manifest_file(path: &Path) -> Result<Manifest> {
    read_manifest(File::open(path).map_err(|e| io_err("no se pudo abrir el paquete", e))?)
}

fn partial_path(dest: &Path) -> PathBuf {
    let mut name = dest.file_name().unwrap_or_default().to_os_string();
    name.push(".partial");
    dest.with_file_name(name)
}

/// Extrae `paq` a `dest.partial`, verifica cada archivo y renombra a `dest`. Si `dest` existe se sustituye.
pub fn unpack_verified(paq: impl Read, dest: &Path) -> Result<Manifest> {
    let partial = partial_path(dest);
    if partial.exists() {
        fs::remove_dir_all(&partial).map_err(|e| io_err("no se pudo limpiar una extracción anterior", e))?;
    }
    fs::create_dir_all(&partial).map_err(|e| io_err("no se pudo crear la carpeta de extracción", e))?;
    let result = extract(paq, &partial);
    let manifest = match result {
        Ok(m) => m,
        Err(e) => {
            let _ = fs::remove_dir_all(&partial);
            return Err(e);
        }
    };
    if dest.exists() {
        fs::remove_dir_all(dest).map_err(|e| io_err("no se pudo sustituir la versión existente", e))?;
    }
    fs::rename(&partial, dest).map_err(|e| io_err("no se pudo terminar la extracción", e))?;
    Ok(manifest)
}

pub fn unpack_verified_file(paq: &Path, dest: &Path) -> Result<Manifest> {
    unpack_verified(
        File::open(paq).map_err(|e| io_err("no se pudo abrir el paquete", e))?,
        dest,
    )
}

fn extract(paq: impl Read, into: &Path) -> Result<Manifest> {
    let mut ar = open_tar(paq)?;
    let mut entries = ar.entries().map_err(|e| Error::Corrupt(e.to_string()))?;
    let mut first = entries
        .next()
        .ok_or_else(|| Error::Corrupt("paquete vacío".into()))?
        .map_err(|e| Error::Corrupt(e.to_string()))?;
    let manifest = read_manifest_entry(&mut first)?;
    let expected: BTreeMap<&str, &FileEntry> = manifest.files.iter().map(|f| (f.path.as_str(), f)).collect();
    let mut done: HashSet<String> = HashSet::new();

    for entry in entries {
        let mut entry = entry.map_err(|e| Error::Corrupt(e.to_string()))?;
        let ty = entry.header().entry_type();
        let name = entry
            .path()
            .map_err(|e| Error::Corrupt(e.to_string()))?
            .to_string_lossy()
            .into_owned();
        if ty.is_dir() {
            safe_rel_path(name.trim_end_matches('/'))?;
            continue;
        }
        if !ty.is_file() {
            return Err(Error::UnsafeEntry(format!("{name} (enlace o tipo especial)")));
        }
        let rel = safe_rel_path(&name)?;
        let want = expected
            .get(name.as_str())
            .ok_or_else(|| Error::Corrupt(format!("{name} no figura en files.json")))?;
        if !done.insert(name.clone()) {
            return Err(Error::Corrupt(format!("{name} aparece dos veces")));
        }
        if entry.size() != want.size {
            return Err(Error::Corrupt(format!("{name}: tamaño distinto del declarado")));
        }
        let target = into.join(&rel);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| io_err("no se pudo crear una carpeta", e))?;
        }
        let mut out = File::create(&target).map_err(|e| io_err(&format!("no se pudo crear {name}"), e))?;
        let mut hasher = Sha256::new();
        let mut buf = vec![0u8; 64 * 1024];
        let mut total = 0u64;
        loop {
            let k = entry
                .read(&mut buf)
                .map_err(|e| Error::Corrupt(format!("{name}: {e}")))?;
            if k == 0 {
                break;
            }
            total += k as u64;
            if total > want.size {
                return Err(Error::Corrupt(format!("{name}: más datos de los declarados")));
            }
            hasher.update(&buf[..k]);
            out.write_all(&buf[..k])
                .map_err(|e| io_err(&format!("no se pudo escribir {name}"), e))?;
        }
        if total != want.size || hex(&hasher.finalize()) != want.sha256 {
            return Err(Error::Corrupt(format!("{name}: la suma de comprobación no coincide")));
        }
    }
    if let Some(missing) = manifest.files.iter().find(|f| !done.contains(&f.path)) {
        return Err(Error::Corrupt(format!("falta {}", missing.path)));
    }
    let json = serde_json::to_vec_pretty(&manifest).map_err(|e| Error::Corrupt(e.to_string()))?;
    fs::write(into.join(MANIFEST_NAME), json).map_err(|e| io_err("no se pudo escribir files.json", e))?;
    Ok(manifest)
}

/// Comprueba una carpeta instalada contra su `files.json` (para «Reparar»). Devuelve las rutas que fallan.
pub fn verify_dir(dir: &Path) -> Result<Vec<String>> {
    let raw = fs::read(dir.join(MANIFEST_NAME)).map_err(|e| io_err("falta files.json", e))?;
    let m: Manifest = serde_json::from_slice(&raw).map_err(|e| Error::Corrupt(format!("files.json: {e}")))?;
    let mut bad = Vec::new();
    for f in &m.files {
        let path = dir.join(safe_rel_path(&f.path)?);
        let ok = File::open(&path)
            .and_then(|mut h| hash_reader(&mut h))
            .map(|(size, sha)| size == f.size && sha == f.sha256)
            .unwrap_or(false);
        if !ok {
            bad.push(f.path.clone());
        }
    }
    Ok(bad)
}

impl Layout {
    /// Extrae el paquete a `app-<v>\` (verificado) y lo deja como `pending`. Devuelve la versión.
    pub fn stage(&self, paq: &Path) -> Result<String> {
        let version = read_manifest_file(paq)?.version;
        let dest = self.app_dir(&version);
        if self.current()?.as_deref() == Some(version.as_str()) {
            return Err(Error::InvalidVersion(format!("{version} ya es la versión activa")));
        }
        unpack_verified_file(paq, &dest)?;
        self.set_pending(&version)?;
        Ok(version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn sample(dir: &Path, marker: &str) {
        fs::create_dir_all(dir.join("resources/fonts")).unwrap();
        fs::write(dir.join("pliego-app.exe"), format!("app {marker}")).unwrap();
        fs::write(dir.join("pliego.exe"), "cli").unwrap();
        fs::write(dir.join("resources/fonts/a.ttf"), vec![7u8; 100_000]).unwrap();
    }

    fn make_paq(version: &str, marker: &str) -> (tempfile::TempDir, PathBuf) {
        let d = tempfile::tempdir().unwrap();
        let src = d.path().join("src");
        sample(&src, marker);
        let out = d.path().join(format!("pliego-{version}.paq"));
        pack(&src, version, &out).unwrap();
        (d, out)
    }

    #[test]
    fn pack_unpack_roundtrip_verifies_every_file() {
        let (d, paq) = make_paq("0.4.0", "a");
        let dest = d.path().join("app-0.4.0");
        let m = unpack_verified_file(&paq, &dest).unwrap();
        assert_eq!(m.version, "0.4.0");
        assert_eq!(m.files.len(), 3);
        assert_eq!(fs::read_to_string(dest.join("pliego-app.exe")).unwrap(), "app a");
        assert!(dest.join("files.json").is_file());
        assert!(!partial_path(&dest).exists());
        assert!(verify_dir(&dest).unwrap().is_empty());
    }

    #[test]
    fn verify_dir_detects_changes() {
        let (d, paq) = make_paq("0.4.0", "a");
        let dest = d.path().join("app-0.4.0");
        unpack_verified_file(&paq, &dest).unwrap();
        fs::write(dest.join("pliego.exe"), "cli!").unwrap();
        fs::remove_file(dest.join("pliego-app.exe")).unwrap();
        let mut bad = verify_dir(&dest).unwrap();
        bad.sort();
        assert_eq!(bad, vec!["pliego-app.exe", "pliego.exe"]);
    }

    #[test]
    fn pack_is_deterministic() {
        let d = tempfile::tempdir().unwrap();
        let src = d.path().join("src");
        sample(&src, "a");
        pack(&src, "0.4.0", &d.path().join("1.paq")).unwrap();
        pack(&src, "0.4.0", &d.path().join("2.paq")).unwrap();
        assert_eq!(
            fs::read(d.path().join("1.paq")).unwrap(),
            fs::read(d.path().join("2.paq")).unwrap()
        );
    }

    /// Construye un `.paq` a mano con entradas arbitrarias para probar rechazos.
    fn raw_paq(manifest_files: &[(&str, &[u8])], entries: &[(&str, &[u8], tar::EntryType)]) -> Vec<u8> {
        let files: Vec<FileEntry> = manifest_files
            .iter()
            .map(|(p, c)| FileEntry {
                path: p.to_string(),
                size: c.len() as u64,
                sha256: hex(&Sha256::digest(c)),
            })
            .collect();
        let m = Manifest {
            version: "0.4.0".into(),
            files,
        };
        let mut tar = tar::Builder::new(Vec::new());
        let json = serde_json::to_vec(&m).unwrap();
        let mut h = tar::Header::new_gnu();
        h.set_size(json.len() as u64);
        h.set_cksum();
        tar.append_data(&mut h, MANIFEST_NAME, json.as_slice()).unwrap();
        for (name, data, ty) in entries {
            let mut h = tar::Header::new_gnu();
            h.set_entry_type(*ty);
            h.set_size(data.len() as u64);
            // `set_path` rechaza «..»: se escribe el nombre directamente en la cabecera.
            let raw = h.as_old_mut();
            raw.name[..name.len()].copy_from_slice(name.as_bytes());
            if matches!(ty, tar::EntryType::Symlink) {
                h.set_link_name("/etc/passwd").unwrap();
            }
            h.set_cksum();
            tar.append(&h, *data).unwrap();
        }
        zstd::encode_all(tar.into_inner().unwrap().as_slice(), 3).unwrap()
    }

    fn try_unpack(bytes: &[u8]) -> Result<Manifest> {
        let d = tempfile::tempdir().unwrap();
        let dest = d.path().join("app-0.4.0");
        let r = unpack_verified(bytes, &dest);
        assert!(r.is_err() == !dest.exists(), "no debe quedar nada si falla");
        assert!(!partial_path(&dest).exists());
        r
    }

    #[test]
    fn rejects_traversal_absolute_and_links() {
        use tar::EntryType::{Regular, Symlink};
        for name in ["../evil.txt", "/abs.txt", "a/../../b", "C:evil", "a\\b"] {
            let b = raw_paq(&[(name, b"x")], &[(name, b"x", Regular)]);
            assert!(matches!(try_unpack(&b), Err(Error::UnsafeEntry(_))), "{name}");
        }
        let b = raw_paq(&[("ok.txt", b"x")], &[("ok.txt", b"", Symlink)]);
        assert!(matches!(try_unpack(&b), Err(Error::UnsafeEntry(_))));
    }

    #[test]
    fn rejects_tampering_and_extras() {
        use tar::EntryType::Regular;
        // Contenido distinto con el mismo tamaño.
        let b = raw_paq(&[("a.txt", b"hola")], &[("a.txt", b"hole", Regular)]);
        assert!(matches!(try_unpack(&b), Err(Error::Corrupt(_))));
        // Archivo que no figura en files.json.
        let b = raw_paq(
            &[("a.txt", b"x")],
            &[("a.txt", b"x", Regular), ("b.txt", b"y", Regular)],
        );
        assert!(matches!(try_unpack(&b), Err(Error::Corrupt(_))));
        // Falta un archivo declarado.
        let b = raw_paq(&[("a.txt", b"x"), ("b.txt", b"y")], &[("a.txt", b"x", Regular)]);
        assert!(matches!(try_unpack(&b), Err(Error::Corrupt(_))));
        // No es zstd.
        assert!(matches!(try_unpack(b"nada"), Err(Error::Corrupt(_))));
    }

    #[test]
    fn truncated_paq_fails_cleanly() {
        let (d, paq) = make_paq("0.4.0", "a");
        let bytes = fs::read(&paq).unwrap();
        let dest = d.path().join("app-0.4.0");
        let r = unpack_verified(&bytes[..bytes.len() / 2], &dest);
        assert!(r.is_err());
        assert!(!dest.exists() && !partial_path(&dest).exists());
    }

    #[test]
    fn manifest_with_invalid_version_is_rejected() {
        let m = Manifest {
            version: "../x".into(),
            files: vec![],
        };
        let mut tar = tar::Builder::new(Vec::new());
        let json = serde_json::to_vec(&m).unwrap();
        let mut h = tar::Header::new_gnu();
        h.set_size(json.len() as u64);
        h.set_cksum();
        tar.append_data(&mut h, MANIFEST_NAME, json.as_slice()).unwrap();
        let b = zstd::encode_all(tar.into_inner().unwrap().as_slice(), 3).unwrap();
        assert!(matches!(read_manifest(b.as_slice()), Err(Error::InvalidVersion(_))));
    }

    /// Flujo completo con un `pliego-app.exe` falso que falla al arrancar: stage → activar → 2 fallos → vuelta atrás.
    #[test]
    fn stage_activate_and_health_rule_with_failing_app() {
        let root = tempfile::tempdir().unwrap();
        let layout = Layout::new(root.path().join("Pliego"));

        let (_d1, good) = make_paq("0.4.0", "good");
        assert_eq!(layout.stage(&good).unwrap(), "0.4.0");
        assert_eq!(layout.prepare_launch().unwrap().unwrap().version, "0.4.0");
        layout.mark_healthy("0.4.0").unwrap();

        let (_d2, bad) = make_paq("0.5.0", "crashes");
        assert_eq!(layout.stage(&bad).unwrap(), "0.5.0");
        assert_eq!(layout.pending().unwrap().as_deref(), Some("0.5.0"));
        // Mientras no se activa, se sigue ejecutando la actual.
        assert_eq!(layout.current().unwrap().as_deref(), Some("0.4.0"));

        for _ in 0..2 {
            let plan = layout.prepare_launch().unwrap().unwrap();
            assert_eq!(plan.version, "0.5.0");
            layout.report_exit("0.5.0", Duration::from_millis(100)).unwrap();
        }
        let plan = layout.prepare_launch().unwrap().unwrap();
        assert_eq!(
            (plan.version.as_str(), plan.rolled_back_from.as_deref()),
            ("0.4.0", Some("0.5.0"))
        );
        assert_eq!(layout.skipped(), vec!["0.5.0"]);
        layout.cleanup(2).unwrap();
        assert!(layout.app_dir("0.4.0").is_dir());
    }

    #[test]
    fn stage_refuses_active_version() {
        let root = tempfile::tempdir().unwrap();
        let layout = Layout::new(root.path());
        let (_d, p) = make_paq("0.4.0", "a");
        layout.stage(&p).unwrap();
        layout.prepare_launch().unwrap();
        assert!(layout.stage(&p).is_err());
        assert!(layout.app_dir("0.4.0").join("pliego-app.exe").is_file());
    }
}
