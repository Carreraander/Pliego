//! Verificación de un `.paq` antes de que el instalador elevado lo extraiga (`--update --paq --manifest`).
//!
//! El `.paq` suele estar en una carpeta que el usuario puede escribir (`%LOCALAPPDATA%`), así que el proceso
//! elevado: 1) lo copia a una carpeta protegida (dentro de la raíz de instalación, que solo escriben los
//! administradores), 2) calcula el SHA-256 sobre la **copia** (sin ventana para cambiar el archivo después) y
//! 3) lo compara con el manifiesto firmado con Ed25519 por una clave de confianza compilada.

use crate::model::{Result, SetupError};
use pliego_update::{ArtifactKind, Envelope, PublicKey};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// Carpeta protegida de trabajo, bajo la raíz de instalación.
pub const STAGING_DIR: &str = "updates-staging";

/// Arquitectura de este instalador.
const ARCH: &str = "x86_64";

/// Resultado de la verificación: la copia protegida y la versión que el manifiesto firmado declara.
#[derive(Debug)]
pub struct Verified {
    pub copy: PathBuf,
    pub version: String,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Copia `src` a `dest` y devuelve (tamaño, SHA-256) de lo copiado.
fn copy_hashing(src: &Path, dest: &Path) -> std::io::Result<(u64, String)> {
    let mut input = fs::File::open(src)?;
    let mut out = fs::File::create(dest)?;
    let (mut hasher, mut total, mut buf) = (Sha256::new(), 0u64, vec![0u8; 256 * 1024]);
    loop {
        let n = input.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        out.write_all(&buf[..n])?;
        total += n as u64;
    }
    out.sync_all()?;
    Ok((total, hex(&hasher.finalize())))
}

/// Copia `paq` a la carpeta protegida de `root` y la verifica contra `manifest` (sobre firmado).
/// Si algo falla, no queda ninguna copia. Con `trusted` vacío no se acepta nada.
pub fn copy_and_verify(root: &Path, paq: &Path, manifest: &Path, trusted: &[PublicKey]) -> Result<Verified> {
    let bad = |m: String| SetupError::Corrupt(m);
    let envelope = Envelope::from_json(&fs::read(manifest)?).map_err(|e| bad(format!("manifiesto: {e}")))?;
    let signed = envelope
        .open(trusted)
        .map_err(|e| bad(format!("firma del manifiesto no válida: {e}")))?;
    let artifact = signed
        .latest
        .artifacts
        .iter()
        .find(|a| a.kind == ArtifactKind::Package && a.arch == ARCH)
        .ok_or_else(|| bad("el manifiesto no trae paquete para esta arquitectura".into()))?;

    let dir = root.join(STAGING_DIR);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir)?;
    let copy = dir.join("update.paq");
    let result = (|| {
        let (size, sha) = copy_hashing(paq, &copy)?;
        if size != artifact.size || !sha.eq_ignore_ascii_case(&artifact.sha256) {
            return Err(bad("el paquete no coincide con el manifiesto firmado".into()));
        }
        Ok(Verified {
            copy: copy.clone(),
            version: signed.latest.version.to_string(),
        })
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&dir);
    }
    result
}

/// Borra la carpeta protegida de trabajo.
pub fn clean(root: &Path) {
    let _ = fs::remove_dir_all(root.join(STAGING_DIR));
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use pliego_update::sign::{ReleaseKey, seal};
    use pliego_update::{Artifact, Manifest};

    pub(crate) fn manifest_for(bytes: &[u8], version: &str) -> Manifest {
        let mut m = sample();
        m.latest.version = version.parse().unwrap();
        m.latest.artifacts = vec![Artifact {
            kind: ArtifactKind::Package,
            arch: ARCH.into(),
            url: "p.paq".into(),
            size: bytes.len() as u64,
            sha256: hex(&Sha256::digest(bytes)),
        }];
        m
    }

    fn sample() -> Manifest {
        let json = r#"{"schema":1,"product":"pliego","channel":"stable","serial":1,
          "issued_at":"2026-11-03T10:00:00Z","expires_at":"2099-01-01T00:00:00Z",
          "latest":{"version":"0.5.0","released_at":"2026-11-03T10:00:00Z","notes":{},"notes_url":null,
          "critical":false,"rollout_percent":100,"min_windows_build":19041,"artifacts":[]},
          "min_supported_version":"0.1.0","blocked_versions":[]}"#;
        serde_json::from_str::<Manifest>(json).expect("manifiesto de prueba")
    }

    pub(crate) fn write_env(dir: &Path, m: &Manifest, key: &ReleaseKey) -> PathBuf {
        let p = dir.join("manifest.json");
        fs::write(&p, serde_json::to_vec(&seal(m, &[key])).unwrap()).unwrap();
        p
    }

    #[test]
    fn valid_signature_and_hash_gives_protected_copy() {
        let d = tempfile::tempdir().unwrap();
        let (paq, root) = (d.path().join("u.paq"), d.path().join("root"));
        fs::write(&paq, b"contenido del paquete").unwrap();
        fs::create_dir_all(&root).unwrap();
        let key = ReleaseKey::generate().unwrap();
        let man = write_env(d.path(), &manifest_for(b"contenido del paquete", "0.5.0"), &key);
        let v = copy_and_verify(&root, &paq, &man, &[key.public()]).unwrap();
        assert_eq!(v.version, "0.5.0");
        assert!(v.copy.starts_with(root.join(STAGING_DIR)));
        assert_eq!(fs::read(&v.copy).unwrap(), b"contenido del paquete");
        clean(&root);
        assert!(!root.join(STAGING_DIR).exists());
    }

    #[test]
    fn tampered_package_untrusted_key_and_no_keys_are_rejected() {
        let d = tempfile::tempdir().unwrap();
        let (paq, root) = (d.path().join("u.paq"), d.path().join("root"));
        fs::create_dir_all(&root).unwrap();
        let key = ReleaseKey::generate().unwrap();
        let other = ReleaseKey::generate().unwrap();
        let man = write_env(d.path(), &manifest_for(b"bueno", "0.5.0"), &key);

        // El usuario cambia el paquete tras firmarse el manifiesto.
        fs::write(&paq, b"malo!").unwrap();
        let e = copy_and_verify(&root, &paq, &man, &[key.public()]).unwrap_err();
        assert!(matches!(e, SetupError::Corrupt(_)), "{e}");
        assert!(!root.join(STAGING_DIR).exists(), "no deja copia");

        // Manifiesto firmado por una clave que no es de confianza, y sin claves compiladas.
        fs::write(&paq, b"bueno").unwrap();
        assert!(copy_and_verify(&root, &paq, &man, &[other.public()]).is_err());
        assert!(copy_and_verify(&root, &paq, &man, &[]).is_err());
        assert!(copy_and_verify(&root, &paq, &man, &[key.public()]).is_ok());
    }
}
