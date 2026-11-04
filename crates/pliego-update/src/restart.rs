//! "Reiniciar ahora": la orden que, tras cerrar la app, lanza la versión nueva.
//!
//! La app no puede arrancarse a sí misma mientras sigue abierta (la instancia única reenviaría el arranque a la
//! vieja), así que se lanza un PowerShell oculto que espera dos segundos, aplica la actualización por equipo con
//! elevación si hace falta y abre `Pliego.exe` (el lanzador, que activa `pending`). Va en `-EncodedCommand` para no
//! depender de comillas ni de rutas con espacios.

use std::path::{Path, PathBuf};

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;

use crate::install::InstallKind;
use crate::stager::StagedPackage;

#[derive(Debug, Clone, PartialEq)]
pub struct RestartCommand {
    pub program: PathBuf,
    pub args: Vec<String>,
}

fn ps_quote(p: &str) -> String {
    format!("'{}'", p.replace('\'', "''"))
}

/// Une rutas de Windows con `\` aunque la herramienta se pruebe en otro sistema.
fn win_join(dir: &Path, name: &str) -> String {
    format!("{}\\{name}", dir.to_string_lossy().trim_end_matches(['\\', '/']))
}

fn encode(script: &str) -> String {
    let utf16: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    B64.encode(utf16)
}

/// Texto del script (separado para probarlo).
pub(crate) fn script(
    kind: InstallKind,
    root: &Path,
    current: &str,
    staged: Option<&StagedPackage>,
) -> Result<String, String> {
    let launcher = ps_quote(&win_join(root, "Pliego.exe"));
    match kind {
        InstallKind::PerUser => Ok(format!("Start-Sleep -Seconds 2; Start-Process -FilePath {launcher}")),
        InstallKind::PerMachine => {
            let staged = staged.ok_or("no hay ninguna actualización preparada")?;
            let setup = ps_quote(&win_join(
                &PathBuf::from(win_join(root, &format!("app-{current}"))),
                "pliego-setup.exe",
            ));
            // Cada argumento va entre comillas dobles dentro de las simples: Start-Process no las añade solo.
            let paq = format!("'\"{}\"'", staged.paq.to_string_lossy().replace('\'', "''"));
            let root_arg = format!("'\"{}\"'", root.to_string_lossy().replace('\'', "''"));
            let paq_path = ps_quote(&staged.paq.to_string_lossy());
            let sha = staged.sha256.replace('\'', "");
            // El paquete vive en una carpeta que escribe el usuario: se vuelve a comprobar su hash justo antes de pedir
            // elevación (no sustituye a la verificación de `pliego-setup`, que lo repite con la firma). Si se cancela el
            // aviso de UAC o el hash no coincide, Pliego se abre igualmente (con la versión de siempre).
            Ok(format!(
                "Start-Sleep -Seconds 2; try {{ if ((Get-FileHash -Algorithm SHA256 -LiteralPath {paq_path}).Hash -eq '{sha}') {{ Start-Process -FilePath {setup} -ArgumentList '--update','--paq',{paq},'--root',{root_arg} -Verb RunAs -Wait }} }} catch {{}}; Start-Process -FilePath {launcher}"
            ))
        }
        InstallKind::Msi | InstallKind::Portable => Err("esta instalación no se actualiza sola".into()),
    }
}

/// Orden de reinicio. `root`: carpeta de instalación; `current`: versión en marcha; `staged`: paquete preparado
/// (solo por equipo).
pub fn restart_command(
    kind: InstallKind,
    root: &Path,
    current: &str,
    staged: Option<&StagedPackage>,
) -> Result<RestartCommand, String> {
    Ok(RestartCommand {
        program: PathBuf::from("powershell.exe"),
        args: [
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-WindowStyle",
            "Hidden",
            "-EncodedCommand",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .chain(std::iter::once(encode(&script(kind, root, current, staged)?)))
        .collect(),
    })
}

/// Lanza la orden sin esperarla y sin ventana.
pub fn spawn(cmd: &RestartCommand) -> std::io::Result<()> {
    use std::process::{Command, Stdio};
    let mut c = Command::new(&cmd.program);
    c.args(&cmd.args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP
        c.creation_flags(0x0800_0000 | 0x0000_0200);
    }
    c.spawn().map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use semver::Version;

    #[test]
    fn per_user_launches_the_launcher() {
        let s = script(
            InstallKind::PerUser,
            Path::new(r"C:\Users\Ana O'Neil\Pliego"),
            "0.1.0",
            None,
        )
        .unwrap();
        assert!(s.contains(r"'C:\Users\Ana O''Neil\Pliego\Pliego.exe'"), "{s}");
        assert!(s.starts_with("Start-Sleep"));
    }

    #[test]
    fn per_machine_elevates_setup_then_launches() {
        let staged = StagedPackage {
            version: Version::new(0, 1, 1),
            paq: PathBuf::from(r"C:\Users\ana\AppData\Local\Pliego\updates\pliego-0.1.1.paq"),
            sha256: "ab".repeat(32),
        };
        let root = Path::new(r"C:\Program Files\Pliego");
        let s = script(InstallKind::PerMachine, root, "0.1.0", Some(&staged)).unwrap();
        assert!(s.contains(r"app-0.1.0\pliego-setup.exe"), "{s}");
        assert!(s.contains("-Verb RunAs -Wait"));
        assert!(s.contains(&format!("-eq '{}'", "ab".repeat(32))), "{s}");
        assert!(s.contains(r#"'"C:\Program Files\Pliego"'"#), "{s}");
        assert!(s.ends_with(r"Pliego.exe'"));
        assert!(script(InstallKind::PerMachine, root, "0.1.0", None).is_err());
    }

    #[test]
    fn unmanaged_installs_cannot_restart_into_an_update() {
        for k in [InstallKind::Msi, InstallKind::Portable] {
            assert!(restart_command(k, Path::new("x"), "0.1.0", None).is_err());
        }
    }

    #[test]
    fn command_is_utf16_base64() {
        let c = restart_command(InstallKind::PerUser, Path::new(r"D:\p"), "0.1.0", None).unwrap();
        let raw = B64.decode(c.args.last().unwrap()).unwrap();
        let units: Vec<u16> = raw.chunks(2).map(|b| u16::from_le_bytes([b[0], b[1]])).collect();
        assert!(String::from_utf16(&units).unwrap().contains(r"D:\p\Pliego.exe"));
    }
}
