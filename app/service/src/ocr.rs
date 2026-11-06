//! OCR desde la app (flujo W24): `pliego ocr` como **proceso hijo**.
//!
//! La app no enlaza Tesseract (habría que empaquetar sus DLL junto a ella y configurar vcpkg en su CI): ejecuta la CLI
//! `pliego` que va en la misma carpeta (`app-<versión>\pliego.exe`, con las DLL de Tesseract), lee el progreso de su
//! salida de error (`[hecho/total] mensaje`), cancela matándola y recoge el informe JSON de la salida estándar.
//! La lógica vive aquí, sin Tauri, para probarla en Linux; `app/src-tauri/src/commands/ocr.rs` solo traduce.

use std::ffi::OsString;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use pliego_appdata::copyjob::unique_sibling;
use pliego_core::{Error, JobContext, Report, Result};
use serde::{Deserialize, Serialize};

/// Lo que pide el panel «Reconocer texto» / «Enderezar y limpiar».
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct OcrRequest {
    pub path: PathBuf,
    /// Idiomas de Tesseract separados por `+` (`spa+eng`).
    pub langs: String,
    /// Páginas desde 1 (`all`, `1-3,5`).
    pub pages: String,
    /// Reconocer también las páginas que ya tienen texto.
    pub force: bool,
    pub deskew: bool,
    pub rotate: bool,
    pub binarize: bool,
    pub denoise: bool,
    pub dpi: u32,
    /// Etiqueta del nombre de la copia, ya traducida («OCR»).
    pub suffix: String,
    /// Contraseña del PDF si está protegido. Va al proceso hijo por la variable de entorno `PLIEGO_PASSWORD`, nunca
    /// por la línea de órdenes (que otros procesos pueden leer). La copia conserva la protección.
    pub password: Option<String>,
}

impl Default for OcrRequest {
    fn default() -> Self {
        Self {
            path: PathBuf::new(),
            langs: "spa+eng".into(),
            pages: "all".into(),
            force: false,
            deskew: false,
            rotate: false,
            binarize: false,
            denoise: false,
            dpi: 300,
            suffix: "OCR".into(),
            password: None,
        }
    }
}

/// Idiomas instalados y descargables.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct OcrLanguages {
    pub installed: Vec<String>,
    pub downloadable: Vec<String>,
}

fn exe_name() -> &'static str {
    if cfg!(windows) { "pliego.exe" } else { "pliego" }
}

/// Dónde está la CLI: `PLIEGO_CLI`, o junto al ejecutable de la app. `Err` si no está («OCR no instalado»).
pub fn find_cli() -> Result<PathBuf> {
    if let Some(p) = std::env::var_os("PLIEGO_CLI") {
        let p = PathBuf::from(p);
        return if p.is_file() { Ok(p) } else { Err(missing(&p)) };
    }
    let here = std::env::current_exe().map_err(Error::from)?;
    let dir = here.parent().unwrap_or(Path::new("."));
    let candidate = dir.join(exe_name());
    if candidate.is_file() {
        Ok(candidate)
    } else {
        Err(missing(&candidate))
    }
}

fn missing(p: &Path) -> Error {
    Error::Unsupported(format!(
        "el OCR no está instalado: falta {} (reinstala Pliego)",
        p.display()
    ))
}

/// Argumentos de `pliego ocr` para una petición.
pub fn ocr_args(req: &OcrRequest, output: &Path) -> Result<Vec<OsString>> {
    if req.langs.trim().is_empty()
        || !req
            .langs
            .split('+')
            .all(|l| !l.is_empty() && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
    {
        return Err(Error::InvalidInput(format!("idiomas no válidos: «{}»", req.langs)));
    }
    if req.dpi < 300 || req.dpi > 1200 {
        return Err(Error::InvalidInput("la resolución del OCR va de 300 a 1200 ppp".into()));
    }
    let mut a: Vec<OsString> = vec!["--json".into(), "ocr".into(), req.path.clone().into_os_string()];
    a.extend(["-o".into(), output.as_os_str().to_owned()]);
    a.extend(["--lang".into(), req.langs.clone().into()]);
    a.extend(["--pages".into(), req.pages.clone().into()]);
    a.extend(["--dpi".into(), req.dpi.to_string().into()]);
    for (on, flag) in [
        (req.force, "--force"),
        (req.deskew, "--deskew"),
        (req.rotate, "--rotate"),
        (req.binarize, "--binarize"),
        (req.denoise, "--denoise"),
    ] {
        if on {
            a.push(flag.into());
        }
    }
    Ok(a)
}

/// `[3/10] mensaje` → (3, 10, "mensaje").
pub fn parse_progress(line: &str) -> Option<(u64, u64, &str)> {
    let rest = line.strip_prefix('[')?;
    let (counts, message) = rest.split_once(']')?;
    let (done, total) = counts.split_once('/')?;
    Some((done.trim().parse().ok()?, total.trim().parse().ok()?, message.trim()))
}

/// Variable de entorno por la que la CLI recibe la contraseña del PDF.
pub const PASSWORD_ENV: &str = "PLIEGO_PASSWORD";

fn command(cli: &Path, args: &[OsString], password: Option<&str>) -> Command {
    let mut c = Command::new(cli);
    c.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // La CLI habla el idioma del sistema; aquí se pide español, que es lo que la interfaz sabe traducir (`engineText`).
    c.env("PLIEGO_LANG", "es");
    c.env_remove("RUST_BACKTRACE"); // las trazas de la CLI no son un mensaje para la persona
    // Siempre se fija o se quita, para que una variable del entorno de la persona no cambie el resultado.
    match password.filter(|p| !p.is_empty()) {
        Some(p) => c.env(PASSWORD_ENV, p),
        None => c.env_remove(PASSWORD_ENV),
    };
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    c
}

/// Ejecuta la CLI, reenvía su progreso a `ctx` y devuelve su salida estándar. Si `ctx` cancela, mata al hijo.
pub fn run_cli(cli: &Path, args: &[OsString], ctx: &JobContext) -> Result<String> {
    run_cli_with_password(cli, args, None, ctx)
}

/// Como [`run_cli`], con la contraseña del PDF en `PLIEGO_PASSWORD` del proceso hijo.
pub fn run_cli_with_password(
    cli: &Path,
    args: &[OsString],
    password: Option<&str>,
    ctx: &JobContext,
) -> Result<String> {
    let mut child: Child = command(cli, args, password).spawn().map_err(|e| Error::io_at(cli, e))?;
    let mut stdout = child.stdout.take().expect("stdout canalizada");
    let stderr = child.stderr.take().expect("stderr canalizada");
    let reader = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = stdout.read_to_string(&mut text);
        text
    });
    let mut last_error = String::new();
    for line in BufReader::new(stderr).lines() {
        let Ok(line) = line else { break };
        if let Some((done, total, message)) = parse_progress(&line) {
            if let Err(e) = ctx.step(done, total, message) {
                // Sin esperar al lector: si el hijo dejó nietos con la tubería abierta, nunca llegaría el final.
                let _ = child.kill();
                let _ = child.wait();
                return Err(e);
            }
        } else if !line.trim().is_empty() && !last_error.starts_with("Error:") {
            // El mensaje es la línea «Error: …»; lo que venga después (trazas) no lo sustituye.
            last_error = line;
        }
    }
    if ctx.cancel.is_cancelled() {
        let _ = child.kill();
        let _ = child.wait();
        return Err(Error::Cancelled);
    }
    let status = child.wait().map_err(Error::from)?;
    let out = reader.join().unwrap_or_default();
    if status.success() {
        Ok(out)
    } else {
        let msg = last_error.trim().trim_start_matches("Error:").trim();
        Err(Error::Other(if msg.is_empty() {
            format!("el reconocimiento terminó con error ({status})")
        } else {
            msg.to_owned()
        }))
    }
}

/// Reconoce el texto de `req.path` y guarda `<nombre> (<etiqueta>).pdf` junto a él. El original no cambia.
pub fn ocr_copy(cli: &Path, req: &OcrRequest, ctx: &JobContext) -> Result<Report> {
    let out = unique_sibling(&req.path, &req.suffix, "pdf");
    let args = ocr_args(req, &out)?;
    let result = run_cli_with_password(cli, &args, req.password.as_deref(), ctx);
    match result {
        Ok(stdout) => {
            let mut report: Report = serde_json::from_str(stdout.trim())
                .map_err(|e| Error::Other(format!("respuesta del OCR no válida: {e}")))?;
            if report.outputs.is_empty() {
                report.outputs = vec![out];
            }
            Ok(report)
        }
        Err(e) => {
            // Cancelado o fallido: no se deja una copia a medias.
            let _ = std::fs::remove_file(&out);
            Err(e)
        }
    }
}

/// Idiomas instalados (`--list-languages`) y descargables (`--list-downloadable`).
pub fn languages(cli: &Path) -> Result<OcrLanguages> {
    let ctx = JobContext::silent();
    let list = |flag: &str| -> Result<Vec<String>> {
        let out = run_cli(cli, &["--json".into(), "ocr".into(), flag.into()], &ctx)?;
        serde_json::from_str(out.trim()).map_err(|e| Error::Other(format!("respuesta del OCR no válida: {e}")))
    };
    Ok(OcrLanguages {
        installed: list("--list-languages")?,
        downloadable: list("--list-downloadable")?,
    })
}

/// Descarga el modelo del idioma `code` (con verificación de huella) a la carpeta de usuario.
pub fn download_language(cli: &Path, code: &str, ctx: &JobContext) -> Result<Report> {
    if code.is_empty() || !code.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(Error::InvalidInput(format!("idioma no válido: «{code}»")));
    }
    let args: Vec<OsString> = vec!["--json".into(), "ocr".into(), "--download".into(), code.into()];
    let out = run_cli(cli, &args, ctx)?;
    let paths: Vec<PathBuf> = serde_json::from_str(out.trim()).unwrap_or_default();
    Ok(Report {
        outputs: paths,
        warnings: Vec::new(),
        summary: format!("Idioma «{code}» instalado"),
    })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// Una «CLI» de pruebas: un script que escribe progreso en stderr y un informe en stdout.
    fn fake_cli(dir: &Path, body: &str) -> PathBuf {
        let p = dir.join("pliego");
        std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    struct Collect(std::sync::Mutex<Vec<(u64, u64, String)>>);
    impl pliego_core::Progress for Collect {
        fn report(&self, done: u64, total: u64, message: &str) {
            self.0.lock().unwrap().push((done, total, message.to_owned()));
        }
    }

    fn req(dir: &Path) -> OcrRequest {
        let path = dir.join("scan.pdf");
        std::fs::write(&path, b"%PDF-1.4").unwrap();
        OcrRequest {
            path,
            ..Default::default()
        }
    }

    #[test]
    fn parses_progress_lines() {
        assert_eq!(parse_progress("[3/10] Reconociendo"), Some((3, 10, "Reconociendo")));
        assert_eq!(parse_progress("Aviso: algo"), None);
        assert_eq!(parse_progress("[a/b] x"), None);
    }

    #[test]
    fn builds_the_cli_arguments_and_rejects_bad_ones() {
        let r = OcrRequest {
            path: "a.pdf".into(),
            force: true,
            deskew: true,
            langs: "spa+eng".into(),
            ..Default::default()
        };
        let a: Vec<String> = ocr_args(&r, Path::new("o.pdf"))
            .unwrap()
            .iter()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        assert_eq!(a[..3], ["--json", "ocr", "a.pdf"]);
        assert!(a.contains(&"--force".to_string()) && a.contains(&"--deskew".to_string()));
        assert!(!a.contains(&"--denoise".to_string()));
        for bad in [
            OcrRequest {
                langs: "".into(),
                ..r.clone()
            },
            OcrRequest {
                langs: "spa+;rm".into(),
                ..r.clone()
            },
            OcrRequest { dpi: 72, ..r.clone() },
        ] {
            assert!(matches!(
                ocr_args(&bad, Path::new("o.pdf")),
                Err(Error::InvalidInput(_))
            ));
        }
    }

    #[test]
    fn runs_the_cli_reports_progress_and_returns_its_report() {
        let _serial = crate::test_support::serial(); // escribir y ejecutar un script a la vez (ETXTBSY)
        let dir = tempfile::tempdir().unwrap();
        let cli = fake_cli(
            dir.path(),
            r#"echo "[1/2] Página 1" >&2; echo "[2/2] Página 2" >&2; echo '{"outputs":["/x/o.pdf"],"warnings":["aviso"],"summary":"2 páginas"}'"#,
        );
        let progress = Collect(Default::default());
        let ctx = JobContext::new(&progress, Default::default());
        let r = ocr_copy(&cli, &req(dir.path()), &ctx).unwrap();
        assert_eq!(r.summary, "2 páginas");
        assert_eq!(r.warnings, vec!["aviso"]);
        assert_eq!(progress.0.lock().unwrap().len(), 2);
    }

    #[test]
    fn a_failing_cli_returns_its_message_and_leaves_no_copy() {
        let _serial = crate::test_support::serial(); // escribir y ejecutar un script a la vez (ETXTBSY)
        let dir = tempfile::tempdir().unwrap();
        let cli = fake_cli(dir.path(), r#"echo "Error: no hay modelos de idioma" >&2; exit 1"#);
        let r = req(dir.path());
        let e = ocr_copy(&cli, &r, &JobContext::silent()).unwrap_err();
        assert!(e.to_string().contains("no hay modelos de idioma"), "{e}");
        assert!(!dir.path().join("scan (OCR).pdf").exists());
    }

    #[test]
    fn the_password_goes_in_the_environment_not_in_the_arguments() {
        let _serial = crate::test_support::serial(); // escribir y ejecutar un script a la vez (ETXTBSY)
        let dir = tempfile::tempdir().unwrap();
        // Informa de lo que recibió: el entorno (PLIEGO_PASSWORD) y si algún argumento lleva la contraseña.
        let cli = fake_cli(
            dir.path(),
            r#"case "$*" in *s3creta*) leak=si;; *) leak=no;; esac
echo "{\"outputs\":[],\"warnings\":[],\"summary\":\"pw=$PLIEGO_PASSWORD fuga=$leak\"}""#,
        );
        let mut r = req(dir.path());
        r.password = Some("s3creta".into());
        let out = ocr_copy(&cli, &r, &JobContext::silent()).unwrap();
        assert_eq!(out.summary, "pw=s3creta fuga=no");
        // Sin contraseña se quita la variable del hijo: no hereda una del entorno de la persona.
        let c = command(&cli, &[], None);
        let envs: Vec<_> = c.get_envs().filter(|(k, _)| *k == PASSWORD_ENV).collect();
        assert_eq!(envs, [(std::ffi::OsStr::new(PASSWORD_ENV), None)]);
    }

    #[test]
    fn the_error_message_is_the_error_line_not_the_trace() {
        let _serial = crate::test_support::serial(); // escribir y ejecutar un script a la vez (ETXTBSY)
        let dir = tempfile::tempdir().unwrap();
        let cli = fake_cli(
            dir.path(),
            r#"echo "Error: el documento necesita contraseña" >&2; echo "Stack backtrace:" >&2; echo "  20: _start" >&2; exit 1"#,
        );
        let e = ocr_copy(&cli, &req(dir.path()), &JobContext::silent()).unwrap_err();
        assert_eq!(e.to_string(), "el documento necesita contraseña");
    }

    #[test]
    fn cancelling_kills_the_child_and_leaves_no_copy() {
        let _serial = crate::test_support::serial(); // escribir y ejecutar un script a la vez (ETXTBSY)
        let dir = tempfile::tempdir().unwrap();
        let cli = fake_cli(
            dir.path(),
            r#"echo "[1/9] Página 1" >&2; sleep 30; echo "[2/9] tarde" >&2"#,
        );
        let ctx = JobContext::silent();
        ctx.cancel.cancel();
        let t0 = std::time::Instant::now();
        let r = req(dir.path());
        assert!(matches!(ocr_copy(&cli, &r, &ctx), Err(Error::Cancelled)));
        assert!(t0.elapsed() < std::time::Duration::from_secs(10), "no esperó al hijo");
    }

    #[test]
    fn lists_languages_and_downloads_one() {
        let _serial = crate::test_support::serial(); // escribir y ejecutar un script a la vez (ETXTBSY)
        let dir = tempfile::tempdir().unwrap();
        let cli = fake_cli(
            dir.path(),
            r#"case "$*" in *--list-languages*) echo '["eng","spa"]';; *--list-downloadable*) echo '["eng","fra","spa"]';; *--download*) echo '["/m/fra.traineddata"]';; esac"#,
        );
        let l = languages(&cli).unwrap();
        assert_eq!(l.installed, ["eng", "spa"]);
        assert_eq!(l.downloadable, ["eng", "fra", "spa"]);
        let r = download_language(&cli, "fra", &JobContext::silent()).unwrap();
        assert_eq!(r.outputs, vec![PathBuf::from("/m/fra.traineddata")]);
        assert!(download_language(&cli, "fr;a", &JobContext::silent()).is_err());
    }

    #[test]
    fn a_missing_cli_is_a_clear_error() {
        let e = ocr_copy(
            Path::new("/no/hay/pliego"),
            &req(tempfile::tempdir().unwrap().path()),
            &JobContext::silent(),
        );
        assert!(e.is_err());
    }
}
