//! Preparar un paquete descargado: extraerlo junto a la versión actual (por usuario) o dejarlo anotado para que
//! `pliego-setup --update` elevado lo aplique (por equipo).

use std::path::{Path, PathBuf};

use pliego_install::Layout;
use pliego_install::paq::read_manifest_file;
use semver::Version;
use serde::{Deserialize, Serialize};

use crate::install::InstallKind;
use crate::service::Stager;

/// Paquete descargado y verificado que espera a "Reiniciar y actualizar" (instalación por equipo).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StagedPackage {
    pub version: Version,
    pub paq: PathBuf,
    /// SHA-256 del `.paq` en el momento de prepararlo (ya verificado contra el manifiesto firmado). El reinicio lo
    /// vuelve a comprobar justo antes de pedir elevación.
    pub sha256: String,
    /// Sobre firmado del manifiesto, que `pliego-setup --update --manifest` exige y verifica con la clave compilada.
    pub manifest: PathBuf,
}

/// SHA-256 de un archivo, en hexadecimal.
pub fn file_sha256(path: &Path) -> std::io::Result<String> {
    use sha2::{Digest, Sha256};
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    std::io::copy(&mut f, &mut h)?;
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

const STAGED_FILE: &str = "staged.json";

/// Paquete anotado en `downloads`, si sigue existiendo.
pub fn staged_package(downloads: &Path) -> Option<StagedPackage> {
    let raw = std::fs::read(downloads.join(STAGED_FILE)).ok()?;
    let staged: StagedPackage = serde_json::from_slice(&raw).ok()?;
    staged.paq.is_file().then_some(staged)
}

/// [`Stager`] real: usa la disposición en disco de `pliego-install`.
pub struct LayoutStager {
    kind: InstallKind,
    layout: Layout,
    downloads: PathBuf,
}

impl LayoutStager {
    pub fn new(kind: InstallKind, root: impl Into<PathBuf>, downloads: impl Into<PathBuf>) -> Self {
        Self {
            kind,
            layout: Layout::new(root),
            downloads: downloads.into(),
        }
    }
}

impl Stager for LayoutStager {
    fn stage(&self, package: &Path, version: &Version) -> Result<(), String> {
        let in_package = read_manifest_file(package).map_err(|e| e.to_string())?.version;
        if in_package != version.to_string() {
            return Err(format!("el paquete es de la {in_package}, no de la {version}"));
        }
        match self.kind {
            InstallKind::PerUser => self.layout.stage(package).map(|_| ()).map_err(|e| e.to_string()),
            InstallKind::PerMachine => {
                let staged = StagedPackage {
                    version: version.clone(),
                    paq: package.to_path_buf(),
                    sha256: file_sha256(package).map_err(|e| e.to_string())?,
                    manifest: crate::service::manifest_path(package),
                };
                if !staged.manifest.is_file() {
                    return Err("falta el manifiesto firmado de la actualización".into());
                }
                let json = serde_json::to_vec_pretty(&staged).map_err(|e| e.to_string())?;
                std::fs::create_dir_all(&self.downloads).map_err(|e| e.to_string())?;
                std::fs::write(self.downloads.join(STAGED_FILE), json).map_err(|e| e.to_string())
            }
            InstallKind::Msi | InstallKind::Portable => Err("esta instalación no se actualiza sola".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_paq(dir: &Path, version: &str) -> PathBuf {
        let src = dir.join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("pliego-app.exe"), version).unwrap();
        let out = dir.join(format!("pliego-{version}.paq"));
        pliego_install::paq::pack(&src, version, &out).unwrap();
        out
    }

    #[test]
    fn per_user_extracts_and_marks_pending() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().join("Pliego");
        let layout = Layout::new(&root);
        std::fs::create_dir_all(&root).unwrap();
        layout.write_atomic("current", "0.1.0").unwrap();
        let paq = make_paq(d.path(), "0.1.1");
        let stager = LayoutStager::new(InstallKind::PerUser, &root, d.path().join("dl"));
        stager.stage(&paq, &Version::new(0, 1, 1)).unwrap();
        assert_eq!(layout.pending().unwrap().as_deref(), Some("0.1.1"));
        assert!(root.join("app-0.1.1/pliego-app.exe").is_file());
    }

    #[test]
    fn wrong_version_in_package_is_rejected() {
        let d = tempfile::tempdir().unwrap();
        let paq = make_paq(d.path(), "0.1.1");
        let stager = LayoutStager::new(InstallKind::PerUser, d.path().join("r"), d.path().join("dl"));
        let err = stager.stage(&paq, &Version::new(0, 2, 0)).unwrap_err();
        assert!(err.contains("0.1.1"), "{err}");
    }

    #[test]
    fn per_machine_only_records_the_package() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().join("PF");
        let paq = make_paq(d.path(), "0.1.1");
        let dl = d.path().join("dl");
        let stager = LayoutStager::new(InstallKind::PerMachine, &root, &dl);
        assert!(
            stager.stage(&paq, &Version::new(0, 1, 1)).is_err(),
            "sin manifiesto firmado no se prepara"
        );
        std::fs::write(crate::service::manifest_path(&paq), b"{}").unwrap();
        stager.stage(&paq, &Version::new(0, 1, 1)).unwrap();
        assert!(!root.exists(), "no se toca Program Files sin elevación");
        let staged = staged_package(&dl).unwrap();
        assert_eq!(
            (staged.version, staged.paq.clone()),
            (Version::new(0, 1, 1), paq.clone())
        );
        assert_eq!(staged.sha256, file_sha256(&paq).unwrap());
        assert_eq!(staged.sha256.len(), 64);
        std::fs::remove_file(&paq).unwrap();
        assert!(
            staged_package(&dl).is_none(),
            "si el paquete desapareció, no hay nada preparado"
        );
    }

    #[test]
    fn portable_and_msi_never_stage() {
        let d = tempfile::tempdir().unwrap();
        let paq = make_paq(d.path(), "0.1.1");
        for kind in [InstallKind::Portable, InstallKind::Msi] {
            assert!(
                LayoutStager::new(kind, d.path(), d.path())
                    .stage(&paq, &Version::new(0, 1, 1))
                    .is_err()
            );
        }
    }
}
