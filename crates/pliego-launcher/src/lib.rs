//! Lanzador de Pliego (< 500 KB, sin dependencias salvo `pliego-install` sin empaquetado).
//!
//! Disposición (ver `docs/design/instalador.md` §2):
//! ```text
//! <raíz>\Pliego.exe          lanzador de la app (este programa, `Target::App`)
//! <raíz>\bin\pliego.exe      lanzador de la CLI (`Target::Cli`); NTFS no distingue mayúsculas, así que no puede
//!                            vivir junto a `Pliego.exe`. Es lo que se añade al PATH.
//! <raíz>\current, pending, app-<v>\…
//! ```
//! Pasos: activar `pending` → ejecutar `app-<v>\pliego-app.exe` (o `pliego.exe`) con los mismos argumentos →
//! anotar si falló al arrancar (regla de salud) → devolver su código de salida.

mod platform;

use pliego_install::{LaunchPlan, Layout};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Instant;

/// Qué ejecutable de la versión activa lanza este binario.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    App,
    Cli,
}

impl Target {
    pub fn exe_name(self) -> &'static str {
        match self {
            Target::App => "pliego-app.exe",
            Target::Cli => "pliego.exe",
        }
    }
}

/// Código de salida cuando no se puede lanzar nada.
pub const EXIT_LAUNCH_FAILED: u8 = 1;

/// Punto de entrada de los dos binarios.
pub fn main(target: Target) -> ExitCode {
    platform::init();
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => return fail(target, &format!("No se pudo localizar el lanzador: {e}")),
    };
    let root = install_root(&exe, target);
    match run(&root, target, std::env::args_os().skip(1).collect()) {
        Ok(code) => ExitCode::from(code),
        Err(msg) => fail(target, &msg),
    }
}

/// Raíz de la instalación: la carpeta del lanzador (o su padre para `bin\pliego.exe`).
/// `PLIEGO_INSTALL_ROOT` la fuerza (pruebas y ZIP portátil).
pub fn install_root(exe: &Path, target: Target) -> PathBuf {
    if let Some(r) = std::env::var_os("PLIEGO_INSTALL_ROOT") {
        return PathBuf::from(r);
    }
    let dir = exe.parent().unwrap_or(Path::new("."));
    match target {
        Target::Cli if dir.file_name().is_some_and(|n| n.eq_ignore_ascii_case("bin")) => {
            dir.parent().unwrap_or(dir).to_path_buf()
        }
        _ => dir.to_path_buf(),
    }
}

fn fail(target: Target, msg: &str) -> ExitCode {
    match target {
        Target::App => platform::error_box(msg),
        Target::Cli => eprintln!("pliego: {msg}"),
    }
    ExitCode::from(EXIT_LAUNCH_FAILED)
}

/// Ejecuta la versión activa y devuelve su código de salida.
pub fn run(root: &Path, target: Target, args: Vec<std::ffi::OsString>) -> Result<u8, String> {
    let layout = Layout::new(root);
    let plan = layout
        .prepare_launch()
        .map_err(|e| format!("No se pudo preparar Pliego: {e}"))?
        .ok_or_else(|| "Pliego no está instalado correctamente. Vuelve a ejecutar el instalador.".to_string())?;
    let exe = plan.dir.join(target.exe_name());
    if !exe.is_file() {
        return Err(format!(
            "Falta {} en la versión {}. Repara Pliego con el instalador.",
            target.exe_name(),
            plan.version
        ));
    }

    let mut cmd = Command::new(&exe);
    cmd.args(args)
        .env("PLIEGO_INSTALL_ROOT", root)
        .env("PLIEGO_VERSION", &plan.version);
    if let Some(v) = &plan.rolled_back_from {
        cmd.env("PLIEGO_ROLLED_BACK_FROM", v);
    }
    let started = Instant::now();
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("No se pudo iniciar Pliego {}: {e}", plan.version))?;
    after_spawn(&layout, &plan);
    let status = child
        .wait()
        .map_err(|e| format!("Se perdió el control de Pliego: {e}"))?;

    // La CLI termina rápido a propósito: solo la app cuenta para la regla de salud.
    if target == Target::App {
        let _ = layout.report_exit(&plan.version, started.elapsed());
    }
    let _ = layout.cleanup(2);
    Ok(exit_byte(status.code()))
}

/// ¿Hay que renovar el registro de Windows? Tras activar una versión nueva **o** tras volver atrás por la regla de
/// salud: `DisplayVersion` y los lanzadores deben reflejar la versión en uso, no la que falló.
fn needs_post_update(plan: &LaunchPlan) -> bool {
    plan.activated.is_some() || plan.rolled_back_from.is_some()
}

/// Trabajo posterior al arranque: si cambió la versión en uso, lanza las tareas de `--post-update`.
fn after_spawn(layout: &Layout, plan: &LaunchPlan) {
    if !needs_post_update(plan) {
        return;
    }
    let setup = plan.dir.join("pliego-setup.exe");
    if setup.is_file() {
        let mut cmd = Command::new(setup);
        cmd.arg("--post-update").arg("--root").arg(layout.root());
        platform::detach(&mut cmd);
        let _ = cmd.spawn();
    }
}

/// Convierte el código de salida (i32 en Windows) a 0..=255 conservando los habituales; las señales cuentan como 1.
fn exit_byte(code: Option<i32>) -> u8 {
    match code {
        Some(c) if (0..=255).contains(&c) => c as u8,
        Some(_) => 1, // códigos de Windows > 255 (p. ej. 0xC0000005): `ExitCode` solo admite un byte
        None => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes_map_to_one_byte() {
        assert_eq!(exit_byte(Some(0)), 0);
        assert_eq!(exit_byte(Some(42)), 42);
        assert_eq!(exit_byte(Some(-1)), 1);
        assert_eq!(exit_byte(None), 1);
    }

    #[test]
    fn post_update_runs_after_activation_and_after_rollback() {
        let plan = |activated: Option<&str>, back: Option<&str>| LaunchPlan {
            version: "0.4.0".into(),
            dir: PathBuf::from("x"),
            activated: activated.map(String::from),
            rolled_back_from: back.map(String::from),
        };
        assert!(needs_post_update(&plan(Some("0.5.0"), None)));
        assert!(needs_post_update(&plan(None, Some("0.5.0"))));
        assert!(!needs_post_update(&plan(None, None)));
    }

    #[test]
    fn cli_root_is_parent_of_bin() {
        // Sin PLIEGO_INSTALL_ROOT (las pruebas no lo fijan en este proceso).
        let r = install_root(Path::new("/x/Pliego/bin/pliego.exe"), Target::Cli);
        assert_eq!(r, Path::new("/x/Pliego"));
        let r = install_root(Path::new("/x/Pliego/Pliego.exe"), Target::App);
        assert_eq!(r, Path::new("/x/Pliego"));
    }
}
