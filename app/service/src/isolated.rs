//! Herramientas archivo → archivo en un **proceso aparte** (hueco 1.0 n.º 7).
//!
//! Un PDF roto o malicioso puede tumbar el motor (PDFium, lopdf, un desbordamiento de pila…). Si la herramienta corriera
//! en el proceso de la app, se llevaría la app por delante con todos los documentos abiertos. Por eso cada trabajo se
//! describe con un [`JobSpec`] (los mismos argumentos que antes recibía la función del servicio) y se ejecuta en un
//! hijo: `pliego __job`, la CLI que va junto a la app (la misma que ya usa el OCR). Si el hijo cae, el trabajo termina
//! con un error legible y la app sigue viva.
//!
//! Protocolo (todo en texto, una línea cada vez):
//! - El padre escribe en la **entrada estándar** del hijo el `JobSpec` en JSON y una línea. Así las contraseñas no van
//!   en la línea de órdenes ni en el entorno, donde otros procesos podrían leerlas. Después deja la entrada abierta.
//! - El hijo escribe el progreso en su **salida de error** (`[hecho/total] mensaje`, igual que el OCR) y, al terminar,
//!   una línea JSON en la **salida estándar**: `{"ok": Report}` o `{"err": WireError}`.
//! - Cancelar: el padre escribe `cancel` en la entrada del hijo; el hijo activa su `CancelToken` y la herramienta
//!   termina y limpia como si corriera en la app. Si en [`CANCEL_GRACE`] no termina, el padre lo mata.
//! - Si la entrada del hijo se cierra (la app murió) el hijo cancela y sale: no quedan procesos huérfanos trabajando.
//!
//! La lógica vive aquí, sin Tauri, para probarla en Linux; `app/src-tauri/src/commands/*` solo construye el `JobSpec`.

use std::io::{BufRead, BufReader, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

use pliego_appdata::copyjob::unique_sibling;
use pliego_core::{CancelToken, Error, JobContext, Lang, Progress, Report, Result};
use pliego_organize::{MergeOptions, SplitRule};
use pliego_sign::{SignLang, SignOptions};
use pliego_signflow::SignerSpec;
use pliego_worker::protocol::WireError;
use serde::{Deserialize, Serialize};

use crate::actions::{self, BatchRequest};
use crate::compare::{self, CompareRequest};
use crate::convert::{self, CreateRequest, ExportRequest, OfficeRequest};
use crate::ocr::{find_cli, parse_progress};
use crate::optimize::{self, CompressRequest};
use crate::organize::{self, InsertSource, NumberRequest, PageEdit};
use crate::protect::{self, ProtectRequest, RedactRequest, SanitizeRequest};
use crate::sign::{self, KeySource};
use crate::stamp::{self, StampOp};
use crate::stamps::{self, StampRequest};
use crate::{annot, checks, formdesign, portfolio, search_index};

/// Tiempo que se da al hijo para terminar y limpiar tras `cancel` antes de matarlo.
pub const CANCEL_GRACE: Duration = Duration::from_secs(10);

/// Si es `1`, los trabajos corren en el proceso de la app (diagnóstico). Sin esto, nunca.
pub const IN_PROCESS_ENV: &str = "PLIEGO_JOBS_IN_PROCESS";

/// Un trabajo archivo → archivo con todo lo que necesita. Un valor por herramienta de la app.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "tool", rename_all = "snake_case")]
pub enum JobSpec {
    Actions {
        req: BatchRequest,
    },
    ExportXfdf {
        path: PathBuf,
        password: Option<String>,
        suffix: String,
    },
    ImportXfdf {
        path: PathBuf,
        password: Option<String>,
        xfdf: PathBuf,
        suffix: String,
    },
    TabOrder {
        path: PathBuf,
        password: Option<String>,
        suffix: String,
    },
    Compare {
        path: PathBuf,
        req: CompareRequest,
    },
    Export {
        path: PathBuf,
        req: ExportRequest,
    },
    Office {
        path: PathBuf,
        req: OfficeRequest,
    },
    Create {
        inputs: Vec<PathBuf>,
        req: CreateRequest,
    },
    Pdfa {
        path: PathBuf,
        level: String,
        label: String,
    },
    FormDetect {
        path: PathBuf,
        password: Option<String>,
        suffix: String,
    },
    IndexFolder {
        data_dir: PathBuf,
        folder: PathBuf,
    },
    PortfolioCreate {
        files: Vec<PathBuf>,
        suffix: String,
        lang: Lang,
    },
    PortfolioExtract {
        path: PathBuf,
        names: Vec<String>,
        password: Option<String>,
        suffix: String,
    },
    Compress {
        path: PathBuf,
        req: CompressRequest,
    },
    Merge {
        inputs: Vec<PathBuf>,
        options: MergeOptions,
        suffix: String,
    },
    Split {
        path: PathBuf,
        password: Option<String>,
        rule: SplitRule,
        suffix: String,
    },
    Insert {
        path: PathBuf,
        password: Option<String>,
        source: InsertSource,
        at: u32,
        suffix: String,
    },
    Number {
        path: PathBuf,
        password: Option<String>,
        request: NumberRequest,
        suffix: String,
    },
    Pages {
        path: PathBuf,
        password: Option<String>,
        edits: Vec<PageEdit>,
        suffix: String,
    },
    Protect {
        req: ProtectRequest,
    },
    Unprotect {
        path: PathBuf,
        password: String,
        suffix: Option<String>,
    },
    Sanitize {
        req: SanitizeRequest,
    },
    Redact {
        req: RedactRequest,
    },
    Sign {
        path: PathBuf,
        key: KeySource,
        options: SignOptions,
        suffix: Option<String>,
    },
    FlowPrepare {
        path: PathBuf,
        signers: Vec<SignerSpec>,
        lang: SignLang,
        suffix: Option<String>,
    },
    FlowSign {
        path: PathBuf,
        key: KeySource,
        options: SignOptions,
        suffix: Option<String>,
    },
    Stamp {
        path: PathBuf,
        pages: String,
        password: Option<String>,
        op: StampOp,
    },
    StampAnnot {
        path: PathBuf,
        pages: String,
        password: Option<String>,
        request: StampRequest,
    },
}

impl JobSpec {
    /// Ejecuta el trabajo **en este proceso**. Es lo que hace el hijo; también se usa si no hay CLI (solo en depuración)
    /// o con [`IN_PROCESS_ENV`].
    pub fn run(self, ctx: &JobContext) -> Result<Report> {
        match self {
            JobSpec::Actions { req } => actions::run_batch(&req, ctx),
            JobSpec::ExportXfdf { path, password, suffix } => {
                annot::export_xfdf_copy(&path, password.as_deref(), &suffix)
            }
            JobSpec::ImportXfdf {
                path,
                password,
                xfdf,
                suffix,
            } => annot::import_xfdf_copy(&path, password.as_deref(), &xfdf, &suffix),
            JobSpec::TabOrder { path, password, suffix } => checks::tab_order_copy(&path, password.as_deref(), &suffix),
            JobSpec::Compare { path, req } => compare::compare_copy(&path, &req, ctx),
            JobSpec::Export { path, req } => convert::export_copy(&path, &req, ctx),
            JobSpec::Office { path, req } => convert::office_copy(&path, &req, ctx),
            JobSpec::Create { inputs, req } => convert::create_copy(&inputs, &req, ctx),
            JobSpec::Pdfa { path, level, label } => convert::pdfa_copy(&path, &level, &label, ctx),
            JobSpec::FormDetect { path, password, suffix } => {
                formdesign::detect_copy(&path, password.as_deref(), &suffix, ctx)
            }
            JobSpec::IndexFolder { data_dir, folder } => search_index::index_folder(&data_dir, &folder, ctx),
            JobSpec::PortfolioCreate { files, suffix, lang } => portfolio::create_portfolio(&files, &suffix, lang, ctx),
            JobSpec::PortfolioExtract {
                path,
                names,
                password,
                suffix,
            } => portfolio::extract_copy(&path, &names, password.as_deref(), &suffix, ctx),
            JobSpec::Compress { path, req } => optimize::compress_copy(&path, &req, ctx),
            JobSpec::Merge {
                inputs,
                options,
                suffix,
            } => organize::merge_copy(&inputs, &options, &suffix, ctx),
            JobSpec::Split {
                path,
                password,
                rule,
                suffix,
            } => organize::split_copy(&path, password.as_deref(), &rule, &suffix, ctx),
            JobSpec::Insert {
                path,
                password,
                source,
                at,
                suffix,
            } => organize::insert_copy(&path, password.as_deref(), &source, at, &suffix),
            JobSpec::Number {
                path,
                password,
                request,
                suffix,
            } => organize::number_copy(&path, password.as_deref(), &request, &suffix, ctx),
            JobSpec::Pages {
                path,
                password,
                edits,
                suffix,
            } => organize::pages_copy(&path, password.as_deref(), &edits, &suffix, ctx),
            JobSpec::Protect { req } => protect::protect(&req, ctx),
            JobSpec::Unprotect { path, password, suffix } => {
                protect::unprotect(&path, &password, suffix.as_deref(), ctx)
            }
            JobSpec::Sanitize { req } => protect::sanitize(&req, ctx),
            JobSpec::Redact { req } => protect::redact(&req, ctx),
            JobSpec::Sign {
                path,
                key,
                options,
                suffix,
            } => {
                let out = unique_sibling(&path, suffix.as_deref().unwrap_or("firmado"), "pdf");
                sign::sign_pdf(&path, &out, &key, options, ctx)
            }
            JobSpec::FlowPrepare {
                path,
                signers,
                lang,
                suffix,
            } => {
                let out = unique_sibling(&path, suffix.as_deref().unwrap_or("para firmar"), "pdf");
                sign::flow_prepare(&path, &out, signers, lang, ctx)
            }
            JobSpec::FlowSign {
                path,
                key,
                options,
                suffix,
            } => {
                let out = unique_sibling(&path, suffix.as_deref().unwrap_or("firmado"), "pdf");
                sign::flow_sign(&path, &out, &key, options, ctx)
            }
            JobSpec::Stamp {
                path,
                pages,
                password,
                op,
            } => stamp::stamp_copy(&path, &pages, password.as_deref(), &op, ctx),
            JobSpec::StampAnnot {
                path,
                pages,
                password,
                request,
            } => stamps::stamp_annot_copy(&path, &pages, password.as_deref(), &request, ctx),
        }
    }

    /// Trabajos que se quedan en la app: firmar con un certificado del almacén de Windows. La clave puede estar en una
    /// tarjeta (DNIe) y el controlador pide el PIN con su propio diálogo, que solo es fiable desde el proceso de la
    /// interfaz. Firmar con un archivo PFX sí va al hijo.
    pub fn stays_in_app(&self) -> bool {
        matches!(
            self,
            JobSpec::Sign {
                key: KeySource::Store { .. },
                ..
            } | JobSpec::FlowSign {
                key: KeySource::Store { .. },
                ..
            }
        )
    }
}

/// Ejecuta `spec` donde corresponde: en el proceso hijo `pliego __job` (lo normal) o, si no hay forma de aislarlo, en
/// la app. Es lo que llaman los comandos de Tauri dentro de `jobs.spawn(...)`.
///
/// - Sin la CLI junto a la app el trabajo **falla** con un mensaje claro (como el OCR): nunca se degrada en silencio a
///   correr dentro de la app. La única excepción son las compilaciones de depuración (`tauri dev`, pruebas), donde
///   `pliego.exe` puede no estar compilado: ahí se avisa en el registro y se ejecuta en el proceso.
pub fn dispatch(spec: JobSpec, ctx: &JobContext) -> Result<Report> {
    if spec.stays_in_app() || std::env::var_os(IN_PROCESS_ENV).is_some_and(|v| v == "1") {
        return spec.run(ctx);
    }
    match find_cli() {
        Ok(cli) => run_in_child(&cli, &spec, ctx),
        Err(e) if cfg!(debug_assertions) => {
            pliego_core::log::warn("trabajos", format!("sin proceso aislado en depuración: {e}"));
            spec.run(ctx)
        }
        Err(e) => Err(e),
    }
}

fn child_command(cli: &Path) -> Command {
    let mut c = Command::new(cli);
    c.arg("__job")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    c.env_remove("RUST_BACKTRACE"); // las trazas del hijo no son un mensaje para la persona
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    c
}

/// Lanza `cli __job`, le entrega `spec`, reenvía su progreso a `ctx` y devuelve su informe. Cancela con `cancel` y, si no
/// responde, matándolo. Si el hijo muere sin dar resultado, el error lo dice y se anota en el registro.
pub fn run_in_child(cli: &Path, spec: &JobSpec, ctx: &JobContext) -> Result<Report> {
    run_in_child_with(cli, spec, ctx, CANCEL_GRACE)
}

/// Como [`run_in_child`] con el plazo de cancelación explícito (las pruebas lo acortan).
pub fn run_in_child_with(cli: &Path, spec: &JobSpec, ctx: &JobContext, grace: Duration) -> Result<Report> {
    let mut child: Child = child_command(cli).spawn().map_err(|e| Error::io_at(cli, e))?;
    let mut stdin = child.stdin.take().expect("stdin canalizada");
    let stdout = child.stdout.take().expect("stdout canalizada");
    let stderr = child.stderr.take().expect("stderr canalizada");

    let mut line =
        serde_json::to_vec(spec).map_err(|e| Error::Other(format!("no se pudo preparar el trabajo: {e}")))?;
    line.push(b'\n');
    // Si el hijo ya murió, la escritura falla; el motivo se ve al recoger su estado, más abajo.
    let _ = stdin.write_all(&line).and_then(|_| stdin.flush());
    drop(line);

    // Los dos canales se leen en hilos propios y llegan por una sola cola. No se espera a que se cierren las tuberías:
    // un proceso nieto que las heredara (o que sobreviviera a `kill`) las mantendría abiertas indefinidamente.
    enum Line {
        Out(String),
        Err(String),
    }
    let (tx, rx) = mpsc::channel::<Line>();
    let tx_out = tx.clone();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if tx_out.send(Line::Out(line)).is_err() {
                break;
            }
        }
    });
    std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines() {
            let Ok(line) = line else { break };
            if tx.send(Line::Err(line)).is_err() {
                break;
            }
        }
    });

    let mut output = String::new();
    let mut handle = |line: Line| match line {
        Line::Err(line) => {
            if let Some((done, total, message)) = parse_progress(&line) {
                ctx.progress.report(done, total, message);
            }
        }
        Line::Out(line) => {
            output.push_str(&line);
            output.push('\n');
        }
    };
    let mut cancel_sent: Option<Instant> = None;
    let mut killed = false;
    let status = loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(line) => handle(line),
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => {}
        }
        if let Some(status) = child.try_wait().map_err(Error::from)? {
            // Terminó: lo que ya escribió está en la cola (las tuberías son del sistema, no del hijo).
            while let Ok(line) = rx.recv_timeout(Duration::from_millis(200)) {
                handle(line);
            }
            break status;
        }
        if ctx.cancel.is_cancelled() {
            match cancel_sent {
                None => {
                    cancel_sent = Some(Instant::now());
                    send_cancel(&mut stdin);
                }
                Some(at) if !killed && at.elapsed() > grace => {
                    killed = true;
                    let _ = child.kill();
                }
                Some(_) => {}
            }
        }
    };
    drop(stdin);

    match parse_result(&output) {
        Some(result) => result,
        None if ctx.cancel.is_cancelled() => Err(Error::Cancelled),
        None => {
            pliego_core::log::error(
                "trabajos",
                format!("el proceso de la herramienta terminó sin resultado ({status})"),
            );
            Err(Error::Other(format!(
                "la herramienta se cerró de forma inesperada ({status}); el original no se ha tocado, pero la copia puede estar incompleta"
            )))
        }
    }
}

fn send_cancel(stdin: &mut ChildStdin) {
    let _ = stdin.write_all(b"cancel\n").and_then(|_| stdin.flush());
}

/// Respuesta del hijo: la última línea que sea un `{"ok": …}` o un `{"err": …}`.
fn parse_result(output: &str) -> Option<Result<Report>> {
    #[derive(Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum Reply {
        Ok(Report),
        Err(WireError),
    }
    output
        .lines()
        .rev()
        .find_map(|l| match serde_json::from_str::<Reply>(l.trim()) {
            Ok(Reply::Ok(r)) => Some(Ok(r)),
            Ok(Reply::Err(e)) => Some(Err(Error::from(e))),
            Err(_) => None,
        })
}

/// Progreso hacia la salida de error del hijo: `[hecho/total] mensaje` en una sola línea.
struct StderrProgress;

impl Progress for StderrProgress {
    fn report(&self, done: u64, total: u64, message: &str) {
        let message = message.replace(['\r', '\n'], " ");
        let _ = writeln!(std::io::stderr(), "[{done}/{total}] {message}");
    }
}

/// Punto de entrada de `pliego __job`: lee el trabajo de la entrada estándar, lo ejecuta y escribe el resultado.
/// Devuelve el código de salida del proceso (0 = entregó un resultado, sea bueno o un error de la herramienta).
pub fn child_main() -> i32 {
    // PDFium se carga de `pdfium.dll` junto al ejecutable; si falta, las herramientas que lo necesitan darán su error.
    let _ = pliego_pdf::configure_from_environment();
    let mut input = BufReader::new(std::io::stdin());
    let mut first = String::new();
    let spec = match input.read_line(&mut first) {
        Ok(n) if n > 0 => serde_json::from_str::<JobSpec>(first.trim()).map_err(|e| e.to_string()),
        _ => Err("no llegó ningún trabajo".to_owned()),
    };
    drop(first);
    let spec = match spec {
        Ok(s) => s,
        Err(e) => {
            reply(&Err(Error::InvalidInput(format!("trabajo no válido: {e}"))));
            return 0;
        }
    };

    let cancel = CancelToken::new();
    let watcher_cancel = cancel.clone();
    std::thread::spawn(move || {
        let mut line = String::new();
        loop {
            line.clear();
            match input.read_line(&mut line) {
                Ok(0) | Err(_) => {
                    // El padre cerró la tubería: terminó o murió. Cancelar y, si no se detiene, salir.
                    watcher_cancel.cancel();
                    std::thread::sleep(CANCEL_GRACE);
                    std::process::exit(3);
                }
                Ok(_) if line.trim() == "cancel" => watcher_cancel.cancel(),
                Ok(_) => {}
            }
        }
    });

    let ctx = JobContext::new(&StderrProgress, cancel);
    let result = catch_unwind(AssertUnwindSafe(|| spec.run(&ctx)))
        .unwrap_or_else(|_| Err(Error::Other("la herramienta falló al procesar el documento".into())));
    reply(&result);
    0
}

fn reply(result: &Result<Report>) {
    #[derive(Serialize)]
    #[serde(rename_all = "snake_case")]
    enum Reply<'a> {
        Ok(&'a Report),
        Err(WireError),
    }
    let reply = match result {
        Ok(r) => Reply::Ok(r),
        Err(e) => Reply::Err(WireError::from(e)),
    };
    let mut out = std::io::stdout().lock();
    let _ = serde_json::to_writer(&mut out, &reply);
    let _ = out.write_all(b"\n");
    let _ = out.flush();
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// Una «CLI» de pruebas: un script que hace de `pliego __job`. Recibe el trabajo en su entrada estándar.
    fn fake_cli(dir: &Path, body: &str) -> PathBuf {
        let p = dir.join("pliego");
        std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    fn spec() -> JobSpec {
        JobSpec::Unprotect {
            path: "/tmp/a.pdf".into(),
            password: "s3creta".into(),
            suffix: None,
        }
    }

    struct Collect(std::sync::Mutex<Vec<(u64, u64, String)>>);
    impl Progress for Collect {
        fn report(&self, done: u64, total: u64, message: &str) {
            self.0.lock().unwrap().push((done, total, message.to_owned()));
        }
    }

    #[test]
    fn specs_survive_the_wire_and_know_where_they_run() {
        let specs = vec![
            spec(),
            JobSpec::Insert {
                path: "a.pdf".into(),
                password: None,
                source: InsertSource::File {
                    path: "b.pdf".into(),
                    password: Some("x".into()),
                    pages: vec![0, 2],
                },
                at: 1,
                suffix: "insertado".into(),
            },
            JobSpec::Sign {
                path: "a.pdf".into(),
                key: KeySource::Pfx {
                    path: "k.pfx".into(),
                    password: "p".into(),
                },
                options: SignOptions::default(),
                suffix: None,
            },
            JobSpec::Merge {
                inputs: vec!["a.pdf".into(), "b.pdf".into()],
                options: MergeOptions::default(),
                suffix: "combinado".into(),
            },
        ];
        for s in specs {
            let json = serde_json::to_string(&s).unwrap();
            assert!(!json.contains('\n'), "una línea por trabajo");
            let back: JobSpec = serde_json::from_str(&json).unwrap();
            assert_eq!(serde_json::to_string(&back).unwrap(), json);
            assert!(!s.stays_in_app(), "{json}");
        }
        let store = JobSpec::FlowSign {
            path: "a.pdf".into(),
            key: KeySource::Store {
                thumbprint: "ab".into(),
            },
            options: SignOptions::default(),
            suffix: None,
        };
        assert!(
            store.stays_in_app(),
            "firmar con el almacén (PIN de tarjeta) se queda en la app"
        );
    }

    #[test]
    fn the_child_gets_the_job_on_stdin_and_its_report_comes_back_with_progress() {
        let _serial = crate::test_support::serial(); // escribir y ejecutar un script a la vez (ETXTBSY)
        let dir = tempfile::tempdir().unwrap();
        // Guarda lo que recibió (entrada y argumentos) para que la prueba lo mire.
        let cli = fake_cli(
            dir.path(),
            r#"d=$(dirname "$0")
read spec
echo "$spec" > "$d/got.json"
echo "$*" > "$d/args.txt"
env > "$d/env.txt"
echo "[1/2] Página 1" >&2
echo "[2/2] Página 2" >&2
echo "ruido que no es el resultado"
echo '{"ok":{"outputs":["/x/o.pdf"],"warnings":["aviso"],"summary":"2 páginas"}}'"#,
        );
        let progress = Collect(Default::default());
        let ctx = JobContext::new(&progress, Default::default());
        let r = run_in_child(&cli, &spec(), &ctx).unwrap();
        assert_eq!(r.summary, "2 páginas");
        assert_eq!(r.warnings, vec!["aviso"]);
        assert_eq!(r.outputs, vec![PathBuf::from("/x/o.pdf")]);
        assert_eq!(progress.0.lock().unwrap().len(), 2);

        let got = std::fs::read_to_string(dir.path().join("got.json")).unwrap();
        assert!(got.contains(r#""tool":"unprotect""#), "{got}");
        // La contraseña viaja por la entrada estándar, nunca por la línea de órdenes ni por el entorno.
        assert!(got.contains("s3creta"));
        assert_eq!(
            std::fs::read_to_string(dir.path().join("args.txt")).unwrap().trim(),
            "__job"
        );
        assert!(
            !std::fs::read_to_string(dir.path().join("env.txt"))
                .unwrap()
                .contains("s3creta")
        );
    }

    #[test]
    fn a_tool_error_keeps_its_kind() {
        let _serial = crate::test_support::serial();
        let dir = tempfile::tempdir().unwrap();
        let cli = fake_cli(dir.path(), r#"read spec; echo '{"err":"Password"}'"#);
        assert!(matches!(
            run_in_child(&cli, &spec(), &JobContext::silent()),
            Err(Error::Password)
        ));
        let cli = fake_cli(
            dir.path(),
            r#"read spec; echo '{"err":{"InvalidInput":"rango no válido"}}'"#,
        );
        let e = run_in_child(&cli, &spec(), &JobContext::silent()).unwrap_err();
        assert!(matches!(e, Error::InvalidInput(ref m) if m == "rango no válido"), "{e}");
    }

    #[test]
    fn a_child_that_dies_gives_an_error_and_does_not_hang() {
        let _serial = crate::test_support::serial();
        let dir = tempfile::tempdir().unwrap();
        // Como un fallo nativo de PDFium: progreso a medias y muerte sin resultado.
        let cli = fake_cli(dir.path(), r#"read spec; echo "[1/9] Página 1" >&2; kill -SEGV $$"#);
        let t0 = Instant::now();
        let e = run_in_child(&cli, &spec(), &JobContext::silent()).unwrap_err();
        assert!(e.to_string().contains("se cerró de forma inesperada"), "{e}");
        assert!(t0.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn a_missing_cli_is_an_error() {
        let e = run_in_child(Path::new("/no/existe/pliego"), &spec(), &JobContext::silent()).unwrap_err();
        assert!(matches!(e, Error::Io { .. }), "{e}");
    }

    #[test]
    fn cancelling_asks_the_child_to_stop_and_returns_its_cancelled() {
        let _serial = crate::test_support::serial();
        let dir = tempfile::tempdir().unwrap();
        let cli = fake_cli(
            dir.path(),
            r#"read spec
echo "[1/9] Página 1" >&2
while read l; do
  if [ "$l" = cancel ]; then echo '{"err":"Cancelled"}'; exit 0; fi
done"#,
        );
        let ctx = JobContext::silent();
        let cancel = ctx.cancel.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            cancel.cancel();
        });
        let t0 = Instant::now();
        let e = run_in_child(&cli, &spec(), &ctx).unwrap_err();
        assert!(matches!(e, Error::Cancelled), "{e}");
        assert!(
            t0.elapsed() < Duration::from_secs(5),
            "cancelación cooperativa, sin esperar al plazo"
        );
    }

    #[test]
    fn a_child_that_ignores_cancel_is_killed_after_the_grace_period() {
        let _serial = crate::test_support::serial();
        let dir = tempfile::tempdir().unwrap();
        let cli = fake_cli(dir.path(), r#"read spec; echo "[1/9] Página 1" >&2; sleep 30"#);
        let ctx = JobContext::silent();
        ctx.cancel.cancel();
        let t0 = Instant::now();
        let e = run_in_child_with(&cli, &spec(), &ctx, Duration::from_millis(500)).unwrap_err();
        assert!(matches!(e, Error::Cancelled), "{e}");
        assert!(t0.elapsed() < Duration::from_secs(10), "{:?}", t0.elapsed());
    }

    #[test]
    fn signing_with_the_windows_store_runs_in_the_app_even_when_dispatched() {
        // `dispatch` no lanza ningún hijo para estas firmas (el PIN de la tarjeta lo pide el proceso de la interfaz).
        let store = JobSpec::Sign {
            path: "a.pdf".into(),
            key: KeySource::Store {
                thumbprint: "ab".into(),
            },
            options: SignOptions::default(),
            suffix: None,
        };
        // Fuera de Windows el almacén no existe: el trabajo se ejecuta en la app y da su error, sin lanzar nada.
        let e = dispatch(store, &JobContext::silent()).unwrap_err();
        assert!(matches!(e, Error::Unsupported(_) | Error::Io { .. }), "{e}");
    }
}
