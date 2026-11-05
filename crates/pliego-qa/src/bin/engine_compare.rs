//! Arnés de comparación de motores (W34).
//!
//! ```text
//! engine-compare tests/corpus                                                 # línea base: PDFium contra PDFium
//! engine-compare tests/corpus tests/corpus/external --candidate pdfium       # corpus externo
//! engine-compare tests/corpus --validate-dir salida/                         # además, qpdf --check / veraPDF de salidas
//! engine-compare tests/corpus --mutate 5 --seed 1                            # humo de robustez con PDF corrompidos
//! ```
//!
//! Cada PDF se procesa en un proceso hijo con tiempo límite (una caída del motor no tira el arnés). Escribe un
//! informe JSON y otro Markdown. Sale con error si hay PDF fuera de umbral o caídas que no estén en la lista de
//! diferencias conocidas (`--known`).

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::{Duration, Instant};

use clap::Parser;
use pliego_qa::compare::{FileReport, Status, Thresholds, compare_file};
use pliego_qa::engines;
use pliego_qa::validate::{self, Validation};

#[derive(Parser)]
#[command(name = "engine-compare", about = "Compara dos motores PDF sobre un corpus")]
struct Args {
    /// Directorios (recursivo) o archivos PDF.
    #[arg(default_value = "tests/corpus")]
    inputs: Vec<PathBuf>,
    #[arg(long, default_value = "pdfium")]
    reference: String,
    #[arg(long, default_value = "pdfium")]
    candidate: String,
    #[arg(long, default_value_t = 60)]
    timeout: u64,
    /// Diferencia máxima por canal para igualar dos píxeles.
    #[arg(long, default_value_t = 16)]
    pixel_tolerance: u8,
    /// Fracción máxima de píxeles distintos por página (0,005 = 0,5 %).
    #[arg(long, default_value_t = 0.005)]
    max_diff: f64,
    #[arg(long, default_value_t = 0.98)]
    min_text_similarity: f64,
    #[arg(long, default_value_t = 20)]
    max_pages: u32,
    /// PDF con diferencias o caídas aceptadas (nombre de archivo por línea, `#` comenta).
    #[arg(long, default_value = "tests/corpus/engine-diff-known.txt")]
    known: PathBuf,
    /// Informe JSON.
    #[arg(long, default_value = "target/engine-compare/report.json")]
    json: PathBuf,
    /// Informe Markdown.
    #[arg(long, default_value = "target/engine-compare/report.md")]
    md: PathBuf,
    /// Valida con `qpdf --check` (y veraPDF si está) todos los PDF de estos directorios (salidas de las herramientas).
    #[arg(long)]
    validate_dir: Vec<PathBuf>,
    /// Humo de robustez: genera N copias corrompidas de cada PDF y exige que el candidato no caiga ni se cuelgue.
    #[arg(long)]
    mutate: Option<u32>,
    #[arg(long, default_value_t = 1)]
    seed: u64,
    /// Modo interno.
    #[arg(long, hide = true)]
    one: Option<PathBuf>,
    /// Modo interno: solo abre y renderiza con el candidato.
    #[arg(long, hide = true)]
    smoke: bool,
}

fn main() -> ExitCode {
    let args = Args::parse();
    let th = Thresholds {
        pixel_tolerance: args.pixel_tolerance,
        max_diff_fraction: args.max_diff,
        min_text_similarity: args.min_text_similarity,
        max_pages: args.max_pages,
    };
    let result = match &args.one {
        Some(f) => one(&args, f, &th),
        None => run_all(&args, &th),
    };
    match result {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::from(2)
        }
    }
}

fn one(args: &Args, file: &Path, th: &Thresholds) -> anyhow::Result<ExitCode> {
    let cand = engines::by_name(&args.candidate)?;
    if args.smoke {
        if let Ok(doc) = cand.open(file)
            && doc.page_count().unwrap_or(0) > 0
        {
            let _ = doc.render_page(0, 0.5);
            let _ = doc.page_text(0);
        }
        return Ok(ExitCode::SUCCESS);
    }
    let reference = engines::by_name(&args.reference)?;
    let rep = compare_file(reference.as_ref(), cand.as_ref(), file, th);
    println!("{}", serde_json::to_string(&rep)?);
    Ok(ExitCode::SUCCESS)
}

fn collect(path: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if path.is_dir() {
        let mut entries: Vec<_> = std::fs::read_dir(path)?.collect::<Result<_, _>>()?;
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            collect(&e.path(), out)?;
        }
    } else if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf")) {
        out.push(path.to_owned());
    }
    Ok(())
}

enum Child {
    Done(String),
    Crash(String),
    Hang,
}

fn spawn(exe: &Path, args: &Args, file: &Path, smoke: bool) -> std::io::Result<Child> {
    let mut cmd = Command::new(exe);
    cmd.args(["--reference", &args.reference, "--candidate", &args.candidate]);
    cmd.args(["--pixel-tolerance", &args.pixel_tolerance.to_string()]);
    cmd.args(["--max-diff", &args.max_diff.to_string()]);
    cmd.args(["--min-text-similarity", &args.min_text_similarity.to_string()]);
    cmd.args(["--max-pages", &args.max_pages.to_string()]);
    if smoke {
        cmd.arg("--smoke");
    }
    let mut child = cmd
        .arg("--one")
        .arg(file)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let start = Instant::now();
    let timeout = Duration::from_secs(args.timeout);
    // La salida es pequeña (un JSON); se lee al final para no complicar con hilos.
    loop {
        if let Some(status) = child.try_wait()? {
            let mut s = String::new();
            if let Some(mut o) = child.stdout.take() {
                std::io::Read::read_to_string(&mut o, &mut s)?;
            }
            return Ok(if status.success() {
                Child::Done(s)
            } else {
                Child::Crash(status.to_string())
            });
        }
        if start.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(Child::Hang);
        }
        std::thread::sleep(Duration::from_millis(15));
    }
}

fn crash_report(file: &Path, why: String) -> FileReport {
    FileReport {
        file: file
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        status: Status::Crash,
        reasons: vec![why],
        pages_a: None,
        pages_b: None,
        pages: Vec::new(),
    }
}

fn read_known(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(String::from)
        .collect()
}

fn run_all(args: &Args, th: &Thresholds) -> anyhow::Result<ExitCode> {
    let mut files = Vec::new();
    for i in &args.inputs {
        collect(i, &mut files)?;
    }
    let exe = std::env::current_exe()?;
    let known = read_known(&args.known);
    let is_known = |name: &str| known.iter().any(|k| k == name);

    // 1. Comparación.
    let mut reports = Vec::new();
    for f in &files {
        let rep = match spawn(&exe, args, f, false)? {
            Child::Done(s) => {
                serde_json::from_str(s.trim()).unwrap_or_else(|e| crash_report(f, format!("salida ilegible: {e}")))
            }
            Child::Crash(s) => crash_report(f, format!("el proceso murió ({s})")),
            Child::Hang => crash_report(f, format!("bloqueo (> {} s)", args.timeout)),
        };
        reports.push(rep);
    }
    let unexpected: Vec<&FileReport> = reports
        .iter()
        .filter(|r| matches!(r.status, Status::Fail | Status::Crash) && !is_known(&r.file))
        .collect();
    let count = |s: Status| reports.iter().filter(|r| r.status == s).count();

    // 2. Humo de robustez.
    let mut mutate_bad = Vec::new();
    let mut mutated = 0u32;
    if let Some(n) = args.mutate {
        let dir = tempfile::tempdir()?;
        for (idx, f) in files.iter().enumerate() {
            let data = std::fs::read(f)?;
            for k in 0..n {
                let mut rng = args.seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ ((idx as u64) << 20) ^ u64::from(k) | 1;
                let out = dir.path().join(format!("m{idx}-{k}.pdf"));
                std::fs::write(&out, mutate(&data, &mut rng))?;
                mutated += 1;
                match spawn(&exe, args, &out, true)? {
                    Child::Done(_) => {}
                    Child::Crash(s) => mutate_bad.push(format!(
                        "{} (variante {k}, semilla {}): caída ({s})",
                        f.display(),
                        args.seed
                    )),
                    Child::Hang => mutate_bad.push(format!(
                        "{} (variante {k}, semilla {}): bloqueo",
                        f.display(),
                        args.seed
                    )),
                }
            }
        }
    }

    // 3. Validadores externos de salidas.
    let mut invalid = Vec::new();
    let mut missing = false;
    let mut out_files = Vec::new();
    for d in &args.validate_dir {
        collect(d, &mut out_files)?;
    }
    for f in &out_files {
        for (tool, v) in [("qpdf", validate::qpdf_check(f)), ("veraPDF", validate::verapdf(f))] {
            match v {
                Validation::Fail(m) => invalid.push(format!("{tool}: {} — {m}", f.display())),
                Validation::Missing => missing = true,
                Validation::Ok | Validation::Warn(_) => {}
            }
        }
    }

    // Informes.
    let mut md = String::new();
    md.push_str(&format!(
        "# Comparación de motores: {} frente a {}\n\n",
        args.reference, args.candidate
    ));
    md.push_str(&format!(
        "Umbrales: tolerancia por canal {}, píxeles distintos ≤ {:.3} %, similitud de texto ≥ {:.3}, {} páginas máx.\n\n",
        th.pixel_tolerance,
        th.max_diff_fraction * 100.0,
        th.min_text_similarity,
        th.max_pages
    ));
    md.push_str(&format!(
        "PDF: {} · correctos: {} · rechazados por ambos: {} · fuera de umbral: {} · caídas: {} · inesperados: {}\n\n",
        reports.len(),
        count(Status::Pass),
        count(Status::BothRejected),
        count(Status::Fail),
        count(Status::Crash),
        unexpected.len()
    ));
    for r in &reports {
        if matches!(r.status, Status::Fail | Status::Crash) {
            let tag = if is_known(&r.file) { "conocido" } else { "NUEVO" };
            md.push_str(&format!("- **{}** [{tag}]: {}\n", r.file, r.reasons.join("; ")));
        }
    }
    if args.mutate.is_some() {
        md.push_str(&format!(
            "\nHumo de robustez: {mutated} variantes, {} caídas/bloqueos\n",
            mutate_bad.len()
        ));
        for m in &mutate_bad {
            md.push_str(&format!("- {m}\n"));
        }
    }
    if !out_files.is_empty() {
        md.push_str(&format!(
            "\nValidación de salidas: {} archivos, {} fallos{}\n",
            out_files.len(),
            invalid.len(),
            if missing {
                " (alguna herramienta no está instalada)"
            } else {
                ""
            }
        ));
        for m in &invalid {
            md.push_str(&format!("- {m}\n"));
        }
    }
    for p in [&args.json, &args.md] {
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d)?;
        }
    }
    std::fs::write(&args.json, serde_json::to_string_pretty(&reports)?)?;
    std::fs::write(&args.md, &md)?;
    print!("{md}");
    println!("informes: {} y {}", args.json.display(), args.md.display());
    Ok(
        if unexpected.is_empty() && mutate_bad.is_empty() && invalid.is_empty() {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        },
    )
}

fn next(s: &mut u64) -> u64 {
    *s ^= *s << 13;
    *s ^= *s >> 7;
    *s ^= *s << 17;
    *s
}

/// Corrompe una copia: sobrescribe bytes, trunca o duplica un trozo (determinista según la semilla).
fn mutate(data: &[u8], rng: &mut u64) -> Vec<u8> {
    let mut v = data.to_vec();
    if v.is_empty() {
        return v;
    }
    match next(rng) % 3 {
        0 => {
            for _ in 0..1 + next(rng) % 16 {
                let i = (next(rng) as usize) % v.len();
                v[i] = next(rng) as u8;
            }
        }
        1 => v.truncate(1 + (next(rng) as usize) % v.len()),
        _ => {
            let a = (next(rng) as usize) % v.len();
            let b = (a + (next(rng) as usize) % 512).min(v.len());
            let chunk = v[a..b].to_vec();
            let at = (next(rng) as usize) % v.len();
            v.splice(at..at, chunk);
        }
    }
    v
}
