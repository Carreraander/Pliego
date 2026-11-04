//! Tipo de instalación: decide si Pliego puede actualizarse solo y cómo.

use serde::{Deserialize, Serialize};

/// Cómo está instalado Pliego en este equipo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstallKind {
    /// Instalación por usuario (`%LOCALAPPDATA%\Programs\Pliego`): se actualiza sin elevar.
    PerUser,
    /// Instalación por equipo (`Program Files`): aplicar necesita `pliego-setup --update --paq` elevado.
    PerMachine,
    /// MSI corporativo: lo gestiona TI, sin autoactualización.
    Msi,
    /// ZIP portátil o ejecución desde el código: solo avisa y enlaza la descarga.
    Portable,
}

impl InstallKind {
    /// Deduce el tipo a partir de la carpeta de instalación y de la marca `install-kind` que escribe el instalador
    /// (`user`, `machine`, `msi`). Sin marca válida: portátil, que es lo más prudente (solo avisa).
    pub fn detect(mark: Option<&str>) -> Self {
        match mark.map(str::trim) {
            Some("user") => Self::PerUser,
            Some("machine") => Self::PerMachine,
            Some("msi") => Self::Msi,
            _ => Self::Portable,
        }
    }

    /// Tipo de instalación de la app en marcha. `root`: carpeta de instalación (la pasa el lanzador en
    /// `PLIEGO_INSTALL_ROOT`; sin ella, Pliego se ejecuta suelto: portátil). `mark`: contenido de `<raíz>\install-kind`
    /// si existe (el MSI escribe `msi`). Sin marca, una raíz dentro de `Program Files` es por equipo y cualquier otra,
    /// por usuario.
    pub fn infer(root: Option<&std::path::Path>, mark: Option<&str>, program_dirs: &[std::path::PathBuf]) -> Self {
        let Some(root) = root else {
            return Self::Portable;
        };
        if let Some(m) = mark.filter(|m| !m.trim().is_empty()) {
            return Self::detect(Some(m));
        }
        let lower = |p: &std::path::Path| p.to_string_lossy().replace('/', "\\").to_lowercase();
        let root = lower(root);
        let inside = |d: &std::path::PathBuf| {
            let d = lower(d);
            !d.is_empty() && (root == d || root.starts_with(&format!("{}\\", d.trim_end_matches('\\'))))
        };
        if program_dirs.iter().any(inside) {
            Self::PerMachine
        } else {
            Self::PerUser
        }
    }

    /// ¿Puede la app descargar y preparar la actualización?
    pub fn can_stage(self) -> bool {
        matches!(self, Self::PerUser | Self::PerMachine)
    }

    /// ¿Hay que pedir elevación (UAC) para activarla?
    pub fn needs_elevation(self) -> bool {
        self == Self::PerMachine
    }

    /// ¿Comprueba versiones nuevas? El MSI no: lo despliega TI.
    pub fn checks(self) -> bool {
        self != Self::Msi
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_map_to_kinds() {
        assert_eq!(InstallKind::detect(Some("user\n")), InstallKind::PerUser);
        assert_eq!(InstallKind::detect(Some("machine")), InstallKind::PerMachine);
        assert_eq!(InstallKind::detect(Some("msi")), InstallKind::Msi);
        assert_eq!(InstallKind::detect(None), InstallKind::Portable);
        assert_eq!(InstallKind::detect(Some("???")), InstallKind::Portable);
        assert!(InstallKind::PerMachine.needs_elevation() && !InstallKind::PerUser.needs_elevation());
        let pf = vec![std::path::PathBuf::from(r"C:\Program Files")];
        let r = |p: &str| Some(std::path::PathBuf::from(p));
        assert_eq!(InstallKind::infer(None, None, &pf), InstallKind::Portable);
        assert_eq!(
            InstallKind::infer(r(r"C:\Program Files\Pliego").as_deref(), None, &pf),
            InstallKind::PerMachine
        );
        assert_eq!(
            InstallKind::infer(r(r"c:\program files\Pliego").as_deref(), None, &pf),
            InstallKind::PerMachine
        );
        assert_eq!(
            InstallKind::infer(r(r"C:\Program Files2\Pliego").as_deref(), None, &pf),
            InstallKind::PerUser
        );
        assert_eq!(
            InstallKind::infer(r(r"C:\Program Files\Pliego").as_deref(), Some("msi\n"), &pf),
            InstallKind::Msi
        );
        assert!(!InstallKind::Msi.checks() && !InstallKind::Portable.can_stage());
    }
}
