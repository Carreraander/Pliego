//! Ejecuta el corpus externo: abre y renderiza cada PDF en un proceso hijo con tiempo límite.
//!
//! Un proceso por archivo aísla caídas y bloqueos de MuPDF. Resultado de cada archivo:
//! - `ok`: se abrió y se renderizó la primera página.
//! - `rechazado`: el motor devolvió un error limpio (aceptado: archivo roto, cifrado, etc.).
//! - `caída`: el proceso hijo murió por una señal o código anormal (NO aceptado).
//! - `bloqueo`: el hijo superó el tiempo límite (NO aceptado).

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::{Duration, Instant};

use clap::Parser;
use pliego_core::{EngineDocument, PdfEngine};
use pliego_engine::DefaultEngine;
use pliego_worker::{WorkerConfig, WorkerEngine};

/// Código de salida del hijo cuando el motor devuelve un error limpio.
const EXIT_REJECTED: u8 = 3;

#[derive(Parser)]
#[command(name = "corpus-runner", about = "Abre y renderiza todos los PDF de un directorio")]
struct Args {
    /// Directorio con PDF (se recorre recursivamente).
    #[arg(default_value = "tests/corpus/external")]
    dir: PathBuf,
    /// Tiempo límite por archivo, en segundos.
    #[arg(long, default_value_t = 60)]
    timeout: u64,
    /// Archivo con nombres de PDF con caída/bloqueo ya conocidos (uno por línea, `#` comenta).
    /// Se listan pero no hacen fallar; un fallo fuera de la lista sí.
    #[arg(long, default_value = "tests/corpus/known-issues.txt")]
    known: PathBuf,
    /// Renderiza mediante `pliego-worker` (proceso auxiliar que corta por tiempo y relanza), como hace la app.
    /// Con esto los fallos del motor deben quedar contenidos (error limpio) en lugar de caída o bloqueo.
    /// Requiere `pliego-worker` junto a este ejecutable o en `PLIEGO_WORKER`.
    #[arg(long)]
    worker: bool,
    /// Modo interno: procesa un solo archivo.
    #[arg(long, hide = true)]
    one: Option<PathBuf>,
}

fn main() -> ExitCode {
    let args = Args::parse();
    match args.one {
        Some(file) => worker(&file, args.worker),
        None => match run_all(&args.dir, &args.known, Duration::from_secs(args.timeout), args.worker) {
            Ok(true) => ExitCode::SUCCESS,
            Ok(false) => ExitCode::FAILURE,
            Err(e) => {
                eprintln!("error: {e:#}");
                ExitCode::from(2)
            }
        },
    }
}

fn open_and_render<E: PdfEngine>(engine: &E, file: &Path) -> pliego_core::Result<()> {
    let doc = engine.open(file, None)?;
    if doc.page_count()? > 0 {
        doc.render_page(0, 0.5)?;
    }
    Ok(())
}

fn worker(file: &Path, use_worker: bool) -> ExitCode {
    let result = if use_worker {
        match WorkerConfig::discover() {
            Ok(mut cfg) => {
                cfg.timeout = Duration::from_secs(10);
                open_and_render(&WorkerEngine::new(cfg), file)
            }
            Err(e) => {
                eprintln!("error: {e}");
                return ExitCode::from(2);
            }
        }
    } else {
        open_and_render(&DefaultEngine, file)
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::from(EXIT_REJECTED),
    }
}

fn collect_pdfs(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_pdfs(&path, out)?;
        } else if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf")) {
            out.push(path);
        }
    }
    Ok(())
}

enum Outcome {
    Ok,
    Rejected,
    Crash(String),
    Hang,
}

fn run_one(exe: &Path, file: &Path, timeout: Duration, use_worker: bool) -> std::io::Result<Outcome> {
    let mut cmd = Command::new(exe);
    if use_worker {
        cmd.arg("--worker");
    }
    let mut child = cmd
        .arg("--one")
        .arg(file)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(match status.code() {
                Some(0) => Outcome::Ok,
                Some(c) if c == i32::from(EXIT_REJECTED) => Outcome::Rejected,
                _ => Outcome::Crash(status.to_string()),
            });
        }
        if start.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(Outcome::Hang);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Devuelve `true` si no hubo caídas ni bloqueos.
fn run_all(dir: &Path, known_file: &Path, timeout: Duration, use_worker: bool) -> anyhow::Result<bool> {
    let mut files = Vec::new();
    collect_pdfs(dir, &mut files)?;
    files.sort();
    let exe = std::env::current_exe()?;
    let (mut ok, mut rejected) = (0usize, Vec::new());
    let known: Vec<String> = std::fs::read_to_string(known_file)
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(String::from)
        .collect();
    let (mut bad, mut known_bad) = (Vec::new(), Vec::new());
    for file in &files {
        match run_one(&exe, file, timeout, use_worker)? {
            Outcome::Ok => ok += 1,
            Outcome::Rejected => rejected.push(file.clone()),
            Outcome::Crash(why) => classify(&mut bad, &mut known_bad, &known, file, format!("caída ({why})")),
            Outcome::Hang => classify(
                &mut bad,
                &mut known_bad,
                &known,
                file,
                format!("bloqueo (>{}s)", timeout.as_secs()),
            ),
        }
    }
    println!(
        "archivos: {}  ok: {ok}  rechazados: {}  conocidos: {}  caídas/bloqueos: {}",
        files.len(),
        rejected.len(),
        known_bad.len(),
        bad.len()
    );
    for k in &known_bad {
        println!("conocido: {k}");
    }
    for f in &rejected {
        println!("rechazado: {}", f.display());
    }
    for b in &bad {
        println!("{b}");
    }
    Ok(bad.is_empty())
}

fn classify(bad: &mut Vec<String>, known_bad: &mut Vec<String>, known: &[String], file: &Path, what: String) {
    let name = file.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    let line = format!("{what}: {}", file.display());
    if known.iter().any(|k| k == name) {
        known_bad.push(line);
    } else {
        bad.push(line);
    }
}
