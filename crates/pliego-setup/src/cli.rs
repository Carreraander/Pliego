//! Línea de órdenes (ver `instalador.md` §7) y reparto entre modo silencioso e interfaz.

use crate::engine::{self, DataDirs, Existing, Source};
use crate::i18n::{self, Lang};
use crate::logging;
use crate::model::{Options, Scope, SetupError, exit};
use crate::platform;
use crate::system::System;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InstallArgs {
    pub silent: bool,
    pub per_machine: bool,
    pub dir: Option<PathBuf>,
    pub desktop_shortcut: Option<bool>,
    pub assoc: Option<bool>,
    pub context_menu: Option<bool>,
    pub path: Option<bool>,
    pub updates: Option<bool>,
    pub lang: Option<String>,
    /// Interno: la interfaz empieza directamente a instalar (tras relanzar elevado).
    pub begin: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Help,
    Install(InstallArgs),
    Uninstall {
        silent: bool,
        purge: bool,
        root: Option<PathBuf>,
        lang: Option<String>,
    },
    PostUpdate {
        root: Option<PathBuf>,
    },
    Update {
        root: Option<PathBuf>,
        paq: PathBuf,
        /// Sobre firmado (Ed25519) del manifiesto que describe el `.paq`.
        manifest: PathBuf,
    },
    Pack {
        base: PathBuf,
        paq: PathBuf,
        out: PathBuf,
    },
    /// Solo compilación: empaqueta una carpeta en un `.paq`.
    MakePaq {
        dir: PathBuf,
        version: String,
        out: PathBuf,
    },
    /// Interno: espera a que termine `pid` y borra lo que quede de la instalación.
    Cleanup {
        root: PathBuf,
        pid: u32,
    },
}

/// Argumentos globales que no cambian el comando.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Parsed {
    pub command: Option<Command>,
    pub log: Option<PathBuf>,
}

pub fn parse(args: &[OsString]) -> Result<Parsed, String> {
    let args: Vec<String> = args.iter().map(|a| a.to_string_lossy().into_owned()).collect();
    let mut i = 0;
    let next = |name: &str, i: &mut usize| -> Result<String, String> {
        *i += 1;
        args.get(*i).cloned().ok_or_else(|| format!("{name} necesita un valor"))
    };

    let mut mode = "install";
    let (mut inst, mut purge, mut silent) = (InstallArgs::default(), false, false);
    let (mut root, mut manifest, mut paq, mut base, mut out, mut log, mut pid) =
        (None, None, None, None, None, None, None);
    let mut version_arg = None;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" | "/?" | "/h" => {
                return Ok(Parsed {
                    command: Some(Command::Help),
                    log: None,
                });
            }
            "/S" | "/s" | "--silent" => silent = true,
            "--per-machine" => inst.per_machine = true,
            "--dir" => inst.dir = Some(PathBuf::from(next("--dir", &mut i)?)),
            "--desktop-shortcut" => inst.desktop_shortcut = Some(true),
            "--no-desktop-shortcut" => inst.desktop_shortcut = Some(false),
            "--no-assoc" => inst.assoc = Some(false),
            "--no-context-menu" => inst.context_menu = Some(false),
            "--path" => inst.path = Some(true),
            "--no-path" => inst.path = Some(false),
            "--no-updates" => inst.updates = Some(false),
            "--lang" => inst.lang = Some(next("--lang", &mut i)?),
            "--begin" => inst.begin = true,
            "--log" => log = Some(PathBuf::from(next("--log", &mut i)?)),
            "--uninstall" => mode = "uninstall",
            "--purge" => purge = true,
            "--post-update" => mode = "post-update",
            "--update" => mode = "update",
            "--pack" => mode = "pack",
            "--make-paq" => mode = "make-paq",
            "--version" => version_arg = Some(next("--version", &mut i)?),
            "--cleanup-from" => {
                mode = "cleanup";
                root = Some(PathBuf::from(next("--cleanup-from", &mut i)?));
            }
            "--wait-pid" => {
                pid = Some(
                    next("--wait-pid", &mut i)?
                        .parse::<u32>()
                        .map_err(|_| "--wait-pid no válido")?,
                )
            }
            "--root" => root = Some(PathBuf::from(next("--root", &mut i)?)),
            "--manifest" => manifest = Some(PathBuf::from(next("--manifest", &mut i)?)),
            "--paq" => paq = Some(PathBuf::from(next("--paq", &mut i)?)),
            "--base" => base = Some(PathBuf::from(next("--base", &mut i)?)),
            "--out" => out = Some(PathBuf::from(next("--out", &mut i)?)),
            other => return Err(format!("argumento desconocido: {other}")),
        }
        i += 1;
    }
    inst.silent = silent;
    let need = |v: Option<PathBuf>, n: &str| v.ok_or_else(|| format!("falta {n}"));
    let command = match mode {
        "uninstall" => Command::Uninstall {
            silent,
            purge,
            root,
            lang: inst.lang.clone(),
        },
        "post-update" => Command::PostUpdate { root },
        "update" => Command::Update {
            root,
            paq: need(paq, "--paq")?,
            manifest: need(manifest, "--manifest")?,
        },
        "pack" => Command::Pack {
            base: need(base, "--base")?,
            paq: need(paq, "--paq")?,
            out: need(out, "--out")?,
        },
        "make-paq" => Command::MakePaq {
            dir: need(inst.dir.clone(), "--dir")?,
            version: version_arg.ok_or("falta --version")?,
            out: need(out, "--out")?,
        },
        "cleanup" => Command::Cleanup {
            root: need(root, "--cleanup-from")?,
            pid: pid.unwrap_or(0),
        },
        _ => Command::Install(inst),
    };
    Ok(Parsed {
        command: Some(command),
        log,
    })
}

/// Entorno que se puede sustituir en pruebas.
pub struct Env {
    pub exe: PathBuf,
    pub data: DataDirs,
    pub default_user_dir: PathBuf,
    pub default_machine_dir: PathBuf,
}

impl Env {
    pub fn real() -> Env {
        Env {
            exe: std::env::current_exe().unwrap_or_default(),
            data: platform::data_dirs(),
            default_user_dir: platform::default_dir(Scope::User),
            default_machine_dir: platform::default_dir(Scope::Machine),
        }
    }

    pub fn exe_root(&self) -> PathBuf {
        self.exe.parent().map(Path::to_path_buf).unwrap_or_default()
    }
}

pub fn run(args: Vec<OsString>) -> u32 {
    let parsed = match parse(&args) {
        Ok(p) => p,
        Err(e) => {
            logging::init(None);
            logging::error(&format!("argumentos: {e}"));
            eprintln!("{e}\n{}", i18n::usage());
            return exit::USAGE;
        }
    };
    logging::init(parsed.log);
    logging::info(&format!("pliego-setup {} · {:?}", crate::model::SETUP_VERSION, args));
    let env = Env::real();
    let sys = platform::system();
    let cmd = parsed.command.unwrap_or(Command::Install(InstallArgs::default()));
    let code = execute(cmd, &env, sys.as_ref());
    logging::info(&format!("salida {code}"));
    code
}

fn lang_of(arg: &Option<String>) -> Lang {
    match arg {
        Some(l) => Lang::from_locale(l),
        None => platform::ui_language(),
    }
}

pub fn execute(cmd: Command, env: &Env, sys: &dyn System) -> u32 {
    match cmd {
        Command::Help => {
            println!("{}", i18n::usage());
            exit::OK
        }
        Command::Pack { base, paq, out } => match pliego_install::footer::append_paq(&base, &paq, &out) {
            Ok(f) => {
                println!("{} ({} bytes de paquete)", out.display(), f.len);
                exit::OK
            }
            Err(e) => {
                eprintln!("{e}");
                exit::FAILED
            }
        },
        Command::MakePaq { dir, version, out } => match pliego_install::paq::pack(&dir, &version, &out) {
            Ok(m) => {
                println!("{} ({} archivos)", out.display(), m.files.len());
                exit::OK
            }
            Err(e) => {
                eprintln!("{e}");
                exit::FAILED
            }
        },
        Command::Install(a) => install_command(a, env, sys),
        Command::Uninstall {
            silent,
            purge,
            root,
            lang,
        } => {
            let root = root.unwrap_or_else(|| env.exe_root());
            let scope = engine::read_info(&root).map(|i| i.options.scope).unwrap_or(Scope::User);
            if silent {
                finish(uninstall_silent(&root, scope, purge, env, sys))
            } else {
                platform::run_ui(crate::cli::UiRequest::Uninstall {
                    root,
                    scope,
                    lang: lang_of(&lang),
                })
            }
        }
        Command::PostUpdate { root } => {
            let root = root.unwrap_or_else(|| env.exe_root());
            finish(engine::post_update(&root, sys))
        }
        Command::Update { root, paq, manifest } => {
            let root = root.unwrap_or_else(|| env.exe_root());
            let trusted = pliego_update::release_keys();
            finish(engine::update_from_paq(&root, &paq, &manifest, &trusted).map(|_| ()))
        }
        Command::Cleanup { root, pid } => {
            platform::wait_for_exit(pid);
            let _ = engine::remove_installed_files(&root);
            platform::delete_self_later();
            exit::OK
        }
    }
}

fn finish(r: Result<(), SetupError>) -> u32 {
    match r {
        Ok(()) => exit::OK,
        Err(e) => {
            logging::error(&e.to_string());
            e.exit_code()
        }
    }
}

/// Qué debe mostrar la interfaz.
#[derive(Debug, Clone)]
pub enum UiRequest {
    Install { args: InstallArgs, lang: Lang },
    Uninstall { root: PathBuf, scope: Scope, lang: Lang },
}

fn build_options(a: &InstallArgs, env: &Env) -> Options {
    let scope = if a.per_machine { Scope::Machine } else { Scope::User };
    let dir = a.dir.clone().unwrap_or_else(|| match scope {
        Scope::User => env.default_user_dir.clone(),
        Scope::Machine => env.default_machine_dir.clone(),
    });
    let mut o = Options::defaults_for(scope, dir);
    o.desktop_shortcut = a.desktop_shortcut.unwrap_or(o.desktop_shortcut);
    o.assoc = a.assoc.unwrap_or(o.assoc);
    o.context_menu = a.context_menu.unwrap_or(o.context_menu);
    o.path = a.path.unwrap_or(o.path);
    o.updates = a.updates.unwrap_or(o.updates);
    o
}

fn install_command(a: InstallArgs, env: &Env, sys: &dyn System) -> u32 {
    let embedded = match Source::embedded(env.exe.clone()) {
        Ok(s) => s,
        Err(e) => {
            logging::error(&format!("paquete embebido: {e}"));
            return e.exit_code();
        }
    };
    let Some(source) = embedded else {
        // Sin paquete: es el desinstalador (Uninstall.exe) llamado sin argumentos.
        let root = env.exe_root();
        let scope = engine::read_info(&root).map(|i| i.options.scope).unwrap_or(Scope::User);
        return if a.silent {
            finish(uninstall_silent(&root, scope, false, env, sys))
        } else {
            platform::run_ui(UiRequest::Uninstall {
                root,
                scope,
                lang: lang_of(&a.lang),
            })
        };
    };
    if !a.silent {
        return platform::run_ui(UiRequest::Install {
            lang: lang_of(&a.lang),
            args: a,
        });
    }
    let opts = build_options(&a, env);
    finish(silent_install(&source, &opts, sys))
}

pub fn silent_install(source: &Source, opts: &Options, sys: &dyn System) -> Result<(), SetupError> {
    if opts.scope == Scope::Machine && !sys.is_admin() {
        return Err(SetupError::Permission(
            "--per-machine requiere ejecutarse como administrador".into(),
        ));
    }
    let manifest = source.manifest()?;
    let installed = engine::active_version(&opts.dir);
    if engine::classify(installed.as_deref(), &manifest.version) == Existing::Newer {
        logging::info(&format!(
            "ya hay una versión más reciente ({}); no se hace nada",
            installed.unwrap_or_default()
        ));
        return Ok(());
    }
    engine::install(
        source,
        opts,
        sys,
        &std::sync::atomic::AtomicBool::new(false),
        &mut |p| logging::info(&format!("{} {}%", p.step, p.percent)),
    )?;
    Ok(())
}

pub fn uninstall_silent(root: &Path, scope: Scope, purge: bool, env: &Env, sys: &dyn System) -> Result<(), SetupError> {
    if scope == Scope::Machine && !sys.is_admin() {
        return Err(SetupError::Permission(
            "desinstalar una instalación por máquina requiere administrador".into(),
        ));
    }
    // El desinstalador vive dentro de la carpeta que borra: una copia temporal termina el trabajo.
    if env.exe.starts_with(root) {
        platform::spawn_cleanup(root);
    }
    engine::uninstall(root, scope, purge, &env.data, sys, &mut |p| {
        logging::info(&format!("{} {}%", p.step, p.percent))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system::fake::FakeSystem;

    fn p(args: &[&str]) -> Result<Parsed, String> {
        parse(&args.iter().map(OsString::from).collect::<Vec<_>>())
    }

    fn cmd(args: &[&str]) -> Command {
        p(args).unwrap().command.unwrap()
    }

    #[test]
    fn parses_install_options() {
        let Command::Install(a) = cmd(&[
            "/S",
            "--per-machine",
            "--dir",
            "D:\\P",
            "--no-assoc",
            "--path",
            "--no-updates",
        ]) else {
            panic!()
        };
        assert!(a.silent && a.per_machine && !a.begin);
        assert_eq!(a.dir, Some(PathBuf::from("D:\\P")));
        assert_eq!(
            (a.assoc, a.path, a.updates, a.context_menu),
            (Some(false), Some(true), Some(false), None)
        );
    }

    #[test]
    fn parses_other_modes() {
        assert_eq!(
            cmd(&["--uninstall", "--silent", "--purge"]),
            Command::Uninstall {
                silent: true,
                purge: true,
                root: None,
                lang: None
            }
        );
        assert_eq!(
            cmd(&["--post-update", "--root", "R"]),
            Command::PostUpdate { root: Some("R".into()) }
        );
        assert_eq!(
            cmd(&["--update", "--paq", "a.paq", "--manifest", "m.json"]),
            Command::Update {
                root: None,
                paq: "a.paq".into(),
                manifest: "m.json".into()
            }
        );
        assert_eq!(
            cmd(&["--pack", "--base", "b", "--paq", "p", "--out", "o"]),
            Command::Pack {
                base: "b".into(),
                paq: "p".into(),
                out: "o".into()
            }
        );
        assert_eq!(
            cmd(&["--cleanup-from", "R", "--wait-pid", "42"]),
            Command::Cleanup {
                root: "R".into(),
                pid: 42
            }
        );
        assert_eq!(p(&["--log", "x.log", "/S"]).unwrap().log, Some("x.log".into()));
        assert_eq!(cmd(&[]), Command::Install(InstallArgs::default()));
    }

    #[test]
    fn rejects_bad_arguments() {
        assert!(p(&["--nope"]).is_err());
        assert!(p(&["--dir"]).is_err());
        assert!(p(&["--update"]).is_err());
        assert!(
            p(&["--update", "--paq", "a.paq"]).is_err(),
            "sin --manifest no se actualiza"
        );
        assert!(p(&["--pack", "--base", "b"]).is_err());
    }

    fn env(d: &Path) -> Env {
        Env {
            exe: d.join("setup.exe"),
            data: DataDirs {
                settings: d.join("s"),
                cache: d.join("c"),
            },
            default_user_dir: d.join("user/Pliego"),
            default_machine_dir: d.join("machine/Pliego"),
        }
    }

    fn sample_paq(d: &Path, version: &str) -> PathBuf {
        let src = d.join(format!("src-{version}"));
        std::fs::create_dir_all(src.join("launcher")).unwrap();
        for (f, c) in [
            ("launcher/Pliego.exe", "L"),
            ("pliego-setup.exe", "S"),
            ("pliego-app.exe", "A"),
        ] {
            std::fs::write(src.join(f), c).unwrap();
        }
        let out = d.join(format!("{version}.paq"));
        pliego_install::paq::pack(&src, version, &out).unwrap();
        out
    }

    #[test]
    fn silent_install_and_exit_codes() {
        let d = tempfile::tempdir().unwrap();
        let e = env(d.path());
        let sys = FakeSystem::new();
        let a = InstallArgs {
            silent: true,
            ..Default::default()
        };
        let opts = build_options(&a, &e);
        assert_eq!(opts.dir, d.path().join("user/Pliego"));
        let src = Source::from_file(sample_paq(d.path(), "0.4.0")).unwrap();
        assert_eq!(finish(silent_install(&src, &opts, &sys)), exit::OK);
        assert!(opts.dir.join("Pliego.exe").is_file());

        // Una versión anterior no sustituye a una posterior: sale bien sin tocar nada.
        let old = Source::from_file(sample_paq(d.path(), "0.3.0")).unwrap();
        assert_eq!(finish(silent_install(&old, &opts, &sys)), exit::OK);
        assert_eq!(std::fs::read_to_string(opts.dir.join("current")).unwrap(), "0.4.0");

        // Por máquina sin ser administrador: 1603.
        let mut m = build_options(
            &InstallArgs {
                per_machine: true,
                ..a.clone()
            },
            &e,
        );
        m.dir = d.path().join("m");
        assert_eq!(finish(silent_install(&src, &m, &sys)), exit::FAILED);

        // Sin espacio: 1603.
        let mut poor = FakeSystem::new();
        poor.free = Some(10);
        let mut o2 = build_options(&a, &e);
        o2.dir = d.path().join("otro");
        assert_eq!(finish(silent_install(&src, &o2, &poor)), exit::FAILED);
    }

    #[test]
    fn pack_command_builds_installer_exe() {
        let d = tempfile::tempdir().unwrap();
        let paq = sample_paq(d.path(), "0.4.0");
        let base = d.path().join("base.exe");
        std::fs::write(&base, b"MZ-base").unwrap();
        let out = d.path().join("Setup.exe");
        let code = execute(
            Command::Pack {
                base,
                paq,
                out: out.clone(),
            },
            &env(d.path()),
            &FakeSystem::new(),
        );
        assert_eq!(code, exit::OK);
        let src = Source::embedded(out).unwrap().unwrap();
        assert_eq!(src.manifest().unwrap().version, "0.4.0");
    }
}
