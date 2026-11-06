//! Banco de PDF grandes (hueco P1 #11): genera un PDF «escaneado» de ~1 GB o uno de 5 000 páginas y mide, con el
//! worker real, el tiempo y la memoria de abrir, desplazarse, buscar y pedir texto.
//!
//! ```text
//! big-bench gen-scan  D:\dev-cache\big\scan-1gb.pdf  --pages 300 --image-mb 3.5
//! big-bench gen-text  D:\dev-cache\big\texto-5000.pdf --pages 5000
//! big-bench run D:\dev-cache\big\scan-1gb.pdf --label escaneado-1gb --json docs\qa\pdf-grandes\salida.json
//! ```
//!
//! La memoria es la **privada** del proceso (lo que enseña el Administrador de tareas): en Linux `RssAnon` de
//! `/proc/<pid>/status`, en Windows `PrivateMemorySize64` (vía PowerShell). `app` es este proceso (cliente del
//! worker, que en la app real es `pliego-app`); `worker` es el proceso `pliego-worker`. El objetivo de Ander es
//! 150 MB entre los dos con un PDF de 100 páginas; con PDF grandes no debe crecer con el tamaño del archivo.

use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use pliego_core::{EngineDocument, PdfEngine, TileRequest};
use pliego_qa::bigpdf;
use pliego_worker::{WorkerConfig, WorkerEngine};
use serde::Serialize;

#[derive(Parser)]
#[command(name = "big-bench", about = "Genera PDF grandes y mide la memoria del worker")]
struct Args {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// PDF escaneado: una imagen JPEG de ruido por página y texto invisible.
    GenScan {
        out: PathBuf,
        #[arg(long, default_value_t = 300)]
        pages: usize,
        /// Tamaño de cada imagen, en MB (300 × 3,5 ≈ 1 GB).
        #[arg(long, default_value_t = 3.5)]
        image_mb: f64,
    },
    /// PDF de texto con muchas páginas y marcadores.
    GenText {
        out: PathBuf,
        #[arg(long, default_value_t = 5000)]
        pages: usize,
    },
    /// Mide un PDF con el worker.
    Run {
        pdf: PathBuf,
        /// Nombre del caso en el informe.
        #[arg(long)]
        label: Option<String>,
        /// Páginas a las que saltar al desplazarse (repartidas por el documento).
        #[arg(long, default_value_t = 40)]
        jumps: usize,
        /// Texto a buscar en todas las páginas (`--search-pages 0` = no buscar).
        #[arg(long, default_value = "contrato")]
        needle: String,
        #[arg(long, default_value_t = 0)]
        search_pages: usize,
        /// Escribe el informe en JSON.
        #[arg(long)]
        json: Option<PathBuf>,
        /// Ruta de `pliego-worker` (por defecto `PLIEGO_WORKER` o junto a este ejecutable).
        #[arg(long)]
        worker: Option<PathBuf>,
    },
}

#[derive(Serialize, Clone)]
struct Step {
    paso: String,
    ms: f64,
    app_mb: f64,
    worker_mb: f64,
    /// Pico del worker desde que arrancó (solo Linux: `VmHWM`).
    worker_pico_mb: Option<f64>,
}

#[derive(Serialize)]
struct Report {
    caso: String,
    archivo_mb: f64,
    paginas: u32,
    pasos: Vec<Step>,
    /// El máximo de app + worker en cualquier paso.
    max_app_mas_worker_mb: f64,
}

fn main() -> Result<()> {
    match Args::parse().cmd {
        Cmd::GenScan { out, pages, image_mb } => {
            mkdir_for(&out)?;
            let t = Instant::now();
            let g = bigpdf::write_scan(&out, pages, (image_mb * 1024.0 * 1024.0) as usize)?;
            println!(
                "{} · {} páginas · {:.0} MB · {:.1} s",
                out.display(),
                g.pages,
                g.bytes as f64 / 1_048_576.0,
                t.elapsed().as_secs_f64()
            );
        }
        Cmd::GenText { out, pages } => {
            mkdir_for(&out)?;
            let g = bigpdf::write_text(&out, pages)?;
            println!(
                "{} · {} páginas · {:.1} MB",
                out.display(),
                g.pages,
                g.bytes as f64 / 1_048_576.0
            );
        }
        Cmd::Run {
            pdf,
            label,
            jumps,
            needle,
            search_pages,
            json,
            worker,
        } => run(&pdf, label, jumps, &needle, search_pages, json.as_deref(), worker)?,
    }
    Ok(())
}

fn mkdir_for(path: &Path) -> Result<()> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    Ok(())
}

/// Memoria privada de un proceso, en MB, y su pico si el sistema lo da.
fn private_mb(pid: u32) -> (f64, Option<f64>) {
    #[cfg(target_os = "linux")]
    {
        let status = std::fs::read_to_string(format!("/proc/{pid}/status")).unwrap_or_default();
        let kb = |key: &str| {
            status
                .lines()
                .find_map(|l| l.strip_prefix(key))
                .and_then(|v| v.trim().trim_end_matches("kB").trim().parse::<f64>().ok())
        };
        // RssAnon = memoria anónima residente (sin las bibliotecas mapeadas de archivo).
        (kb("RssAnon:").unwrap_or(0.0) / 1024.0, kb("VmHWM:").map(|k| k / 1024.0))
    }
    #[cfg(windows)]
    {
        let out = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                &format!("(Get-Process -Id {pid} -ErrorAction SilentlyContinue).PrivateMemorySize64"),
            ])
            .output();
        let bytes = out
            .ok()
            .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse::<f64>().ok())
            .unwrap_or(0.0);
        (bytes / 1_048_576.0, None)
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        let _ = pid;
        (0.0, None)
    }
}

fn run(
    pdf: &Path,
    label: Option<String>,
    jumps: usize,
    needle: &str,
    search_pages: usize,
    json: Option<&Path>,
    worker: Option<PathBuf>,
) -> Result<()> {
    let mut cfg = match worker {
        Some(w) => WorkerConfig::new(w),
        None => WorkerConfig::discover().context("falta pliego-worker: pásalo con --worker o PLIEGO_WORKER")?,
    };
    // Sin el límite de tiempo de la app: aquí se mide, no se protege.
    cfg.timeout = std::time::Duration::from_secs(600);
    // El límite del job object (2 GB) forma parte de lo que se prueba: no se toca.
    let engine = WorkerEngine::new(cfg);
    let me = std::process::id();
    let size_mb = std::fs::metadata(pdf)?.len() as f64 / 1_048_576.0;
    let mut steps: Vec<Step> = Vec::new();
    let mut pid: Option<u32> = None;
    let mut mark = |name: &str, t0: Instant, pid: Option<u32>| {
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        let (app, _) = private_mb(me);
        let (worker, peak) = pid.map(private_mb).unwrap_or((0.0, None));
        println!(
            "{name:<34} {ms:>9.0} ms   app {app:>7.1} MB   worker {worker:>8.1} MB{}",
            peak.map(|p| format!("   (pico {p:.0})")).unwrap_or_default()
        );
        steps.push(Step {
            paso: name.to_owned(),
            ms,
            app_mb: app,
            worker_mb: worker,
            worker_pico_mb: peak,
        });
    };

    println!("{} ({size_mb:.0} MB)", pdf.display());
    let t = Instant::now();
    let doc = engine.open(pdf, None)?;
    pid = doc.worker_pid().or(pid);
    mark("abrir", t, pid);

    let t = Instant::now();
    let pages = doc.page_count()?;
    let info = doc.info()?;
    let _ = doc.page_size(0)?;
    mark(&format!("páginas ({pages}) e info"), t, pid);

    let t = Instant::now();
    let outline = doc.outline()?;
    mark(&format!("marcadores ({})", outline.len()), t, pid);

    let tile = |page: u32| TileRequest {
        page,
        zoom: 1.0,
        col: 0,
        row: 0,
        size: 512,
    };
    let t = Instant::now();
    doc.render_tile_cached(tile(0))?;
    mark("primera tesela", t, pid);

    // Desplazarse: saltos repartidos por todo el documento y de vuelta, como arrastrar la barra.
    let t = Instant::now();
    let mut slowest = 0.0f64;
    for j in 0..jumps {
        let page = (u64::from(pages) * j as u64 / jumps.max(1) as u64) as u32;
        let t1 = Instant::now();
        doc.render_tile_cached(tile(page.min(pages.saturating_sub(1))))?;
        slowest = slowest.max(t1.elapsed().as_secs_f64() * 1000.0);
    }
    mark(&format!("{jumps} saltos (la peor {slowest:.0} ms)"), t, pid);

    // La última página, por si el árbol o la xref penalizan el final.
    let t = Instant::now();
    doc.render_tile_cached(tile(pages.saturating_sub(1)))?;
    mark("última página", t, pid);

    let t = Instant::now();
    let text = doc.page_text(pages / 2)?;
    mark(&format!("texto de una página ({} car.)", text.chars().count()), t, pid);

    if search_pages > 0 {
        let t = Instant::now();
        let n = search_pages.min(pages as usize);
        let mut hits = 0usize;
        for p in 0..n {
            hits += doc.search_page(p as u32, needle, 50)?.quads.len();
        }
        mark(&format!("buscar «{needle}» en {n} págs ({hits} coincidencias)"), t, pid);
    }

    let max = steps.iter().map(|s| s.app_mb + s.worker_mb).fold(0.0f64, f64::max);
    println!("máximo app + worker: {max:.1} MB (objetivo 150 MB en reposo con 100 páginas)");
    let report = Report {
        caso: label.unwrap_or_else(|| {
            pdf.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        }),
        archivo_mb: size_mb,
        paginas: info.page_count,
        pasos: steps,
        max_app_mas_worker_mb: max,
    };
    if let Some(j) = json {
        mkdir_for(j)?;
        std::fs::write(j, serde_json::to_string_pretty(&report)?)?;
    }
    if report.pasos.is_empty() {
        bail!("sin pasos");
    }
    Ok(())
}
