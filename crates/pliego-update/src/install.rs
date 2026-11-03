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
        assert!(!InstallKind::Msi.checks() && !InstallKind::Portable.can_stage());
    }
}
