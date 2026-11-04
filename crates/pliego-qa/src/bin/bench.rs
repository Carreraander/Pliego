//! Benchmarks reproducibles del motor (objetivos de rendimiento de ARCHITECTURE.md y W02).
//!
//! Uso: `cargo run --release -p pliego-qa --bin bench -- <pdf-de-500-paginas> [--json]`
//! (genera el PDF con `python tools/gen_bench_pdf.py`). La memoria máxima la mide `tools/bench.ps1` desde fuera.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use clap::Parser;
use pliego_core::{EngineDocument, PdfEngine, TileRequest};
use pliego_engine::DefaultEngine;

#[derive(Parser)]
#[command(name = "bench", about = "Mide apertura, render de tesela y de página")]
struct Args {
    /// PDF de texto de muchas páginas.
    pdf: PathBuf,
    /// Repeticiones de apertura.
    #[arg(long, default_value_t = 7)]
    opens: usize,
    /// Teselas de 512 px a renderizar.
    #[arg(long, default_value_t = 100)]
    tiles: usize,
    /// Salida JSON en una línea.
    #[arg(long)]
    json: bool,
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    let idx = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[idx]
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let engine = DefaultEngine;

    // Apertura (lee estructura y cuenta páginas); se descarta la primera como calentamiento.
    let mut opens = Vec::new();
    let mut pages = 0;
    for i in 0..args.opens + 1 {
        let t = Instant::now();
        let doc = engine.open(&args.pdf, None)?;
        pages = doc.page_count()?;
        let d = ms(t.elapsed());
        if i > 0 {
            opens.push(d);
        }
    }
    opens.sort_by(f64::total_cmp);

    // Render de teselas de 512 px a 1.0x repartidas por distintas páginas.
    let doc = engine.open(&args.pdf, None)?;
    let mut tiles = Vec::new();
    for i in 0..args.tiles {
        let req = TileRequest {
            page: (i as u32 * 7) % pages,
            zoom: 1.0,
            col: (i as u32) % 2,
            row: (i as u32 / 2) % 2,
            size: 512,
        };
        let t = Instant::now();
        doc.render_tile(req)?;
        tiles.push(ms(t.elapsed()));
    }
    tiles.sort_by(f64::total_cmp);
    let tile_mean = tiles.iter().sum::<f64>() / tiles.len() as f64;

    // Página completa a 2x (≈144 ppp).
    let t = Instant::now();
    doc.render_page(0, 2.0)?;
    let page_ms = ms(t.elapsed());

    if args.json {
        println!(
            "{{\"pages\":{pages},\"open_median_ms\":{:.2},\"open_p95_ms\":{:.2},\"tile512_mean_ms\":{tile_mean:.2},\"tile512_p95_ms\":{:.2},\"page_2x_ms\":{page_ms:.2}}}",
            percentile(&opens, 0.5),
            percentile(&opens, 0.95),
            percentile(&tiles, 0.95)
        );
    } else {
        println!("páginas: {pages}");
        println!(
            "abrir (mediana / p95): {:.1} / {:.1} ms   [objetivo < 1500 ms]",
            percentile(&opens, 0.5),
            percentile(&opens, 0.95)
        );
        println!(
            "tesela 512 px (media / p95): {tile_mean:.1} / {:.1} ms   [objetivo < 30 ms]",
            percentile(&tiles, 0.95)
        );
        println!("página a 2x: {page_ms:.1} ms");
    }
    Ok(())
}
