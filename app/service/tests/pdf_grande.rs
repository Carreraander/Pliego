//! Edición de un PDF grande con el documento abierto (hueco P1 #11): mide la memoria de la app al comentar y deshacer.
//!
//! Es una prueba **manual** (`#[ignore]`): necesita un PDF de ~1 GB que genera `big-bench gen-scan` y tarda minutos.
//!
//! ```text
//! cargo run --release -p pliego-qa --bin big-bench -- gen-scan D:\dev-cache\big\scan-1gb.pdf
//! $env:PLIEGO_BIG_PDF = 'D:\dev-cache\big\scan-1gb.pdf'
//! cargo test --release -p pliego-app-service --test pdf_grande -- --ignored --nocapture
//! ```
//!
//! En Linux, glibc no devuelve al sistema la memoria que libera si la reparte entre muchas asignaciones de 3,5 MB; para
//! medir lo que de verdad retiene la app hay que fijar el umbral: `MALLOC_MMAP_THRESHOLD_=1048576 MALLOC_TRIM_THRESHOLD_=1048576`.
//! (En Windows el montón devuelve los bloques grandes al liberarlos.)
//!
//! Trabaja sobre una **copia** del PDF en una carpeta temporal (junto al original, para no llenar C:). La memoria es la
//! privada de este proceso (`RssAnon` en Linux, `PrivateMemorySize64` en Windows): en reposo, tras cada paso, no debe
//! crecer con el tamaño del archivo aunque durante la edición haya picos mientras se reescribe el PDF entero.

use std::path::{Path, PathBuf};
use std::time::Instant;

use pliego_app_service::DocumentService;
use pliego_app_service::annot::{AnnotSpec, Editor};
use pliego_worker::WorkerConfig;

fn worker_exe() -> PathBuf {
    if let Some(p) = std::env::var_os("PLIEGO_WORKER") {
        return PathBuf::from(p);
    }
    let dir = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    dir.join(format!("pliego-worker{}", std::env::consts::EXE_SUFFIX))
}

/// Pico de memoria del proceso `pid` desde que arrancó, en MB (Linux: `VmHWM`; Windows: `PeakWorkingSet64`).
fn peak_mb(pid: u32) -> f64 {
    #[cfg(target_os = "linux")]
    {
        let s = std::fs::read_to_string(format!("/proc/{pid}/status")).unwrap_or_default();
        s.lines()
            .find_map(|l| l.strip_prefix("VmHWM:"))
            .and_then(|v| v.trim().trim_end_matches("kB").trim().parse::<f64>().ok())
            .unwrap_or(0.0)
            / 1024.0
    }
    #[cfg(windows)]
    {
        let out = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                &format!("(Get-Process -Id {pid}).PeakWorkingSet64"),
            ])
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout)
            .trim()
            .parse::<f64>()
            .unwrap_or(0.0)
            / 1_048_576.0
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        let _ = pid;
        0.0
    }
}

/// Memoria privada del proceso `pid`, en MB.
fn private_mb(pid: u32) -> f64 {
    #[cfg(target_os = "linux")]
    {
        let s = std::fs::read_to_string(format!("/proc/{pid}/status")).unwrap_or_default();
        s.lines()
            .find_map(|l| l.strip_prefix("RssAnon:"))
            .and_then(|v| v.trim().trim_end_matches("kB").trim().parse::<f64>().ok())
            .unwrap_or(0.0)
            / 1024.0
    }
    #[cfg(windows)]
    {
        let out = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                &format!("(Get-Process -Id {pid}).PrivateMemorySize64"),
            ])
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout)
            .trim()
            .parse::<f64>()
            .unwrap_or(0.0)
            / 1_048_576.0
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        let _ = pid;
        0.0
    }
}

fn same_file(a: &Path, b: &Path) -> bool {
    use std::io::Read;
    let (mut fa, mut fb) = (std::fs::File::open(a).unwrap(), std::fs::File::open(b).unwrap());
    if fa.metadata().unwrap().len() != fb.metadata().unwrap().len() {
        return false;
    }
    let (mut ba, mut bb) = (vec![0u8; 1 << 20], vec![0u8; 1 << 20]);
    loop {
        let n = fa.read(&mut ba).unwrap();
        if n == 0 {
            return true;
        }
        fb.read_exact(&mut bb[..n]).unwrap();
        if ba[..n] != bb[..n] {
            return false;
        }
    }
}

#[test]
#[ignore = "manual: necesita PLIEGO_BIG_PDF con un PDF de ~1 GB"]
fn comentar_y_deshacer_un_pdf_de_un_gigabyte() {
    let Some(src) = std::env::var_os("PLIEGO_BIG_PDF").map(PathBuf::from) else {
        panic!("define PLIEGO_BIG_PDF con la ruta de un PDF grande (big-bench gen-scan)");
    };
    let me = std::process::id();
    let dir = tempfile::Builder::new()
        .prefix("pliego-grande-")
        .tempdir_in(src.parent().unwrap())
        .unwrap();
    let path = dir.path().join("grande.pdf");
    std::fs::copy(&src, &path).unwrap();
    let original = dir.path().join("original.pdf");
    std::fs::copy(&src, &original).unwrap();
    let mb = std::fs::metadata(&path).unwrap().len() as f64 / 1_048_576.0;

    let mut config = WorkerConfig::new(worker_exe());
    config.timeout = std::time::Duration::from_secs(600);
    let svc = DocumentService::new(config);
    let ed = Editor::default();
    let mut rows: Vec<(String, f64, f64)> = Vec::new();
    let mut mark = |what: &str, t: Instant| {
        let (ms, app) = (t.elapsed().as_secs_f64() * 1000.0, private_mb(me));
        println!(
            "{what:<28} {ms:>9.0} ms   app {app:>8.1} MB   (pico {:.0})",
            peak_mb(me)
        );
        rows.push((what.to_owned(), ms, app));
    };

    println!("{} ({mb:.0} MB)", src.display());
    let t = Instant::now();
    let id = svc.open_document(&path, None).unwrap();
    mark("abrir", t);

    for i in 1..=3 {
        let mut spec = AnnotSpec::new(
            0,
            pliego_annot::Geometry::Note {
                at: [100.0, 100.0 + 20.0 * i as f32],
            },
        );
        spec.contents = Some(format!("nota {i}"));
        let t = Instant::now();
        let saved = ed.add_saved(&path, None, &spec, "copia de seguridad").unwrap();
        mark(&format!("comentar {i} (guardar)"), t);
        let t = Instant::now();
        svc.reload_saved(id, &path, &saved).unwrap();
        drop(saved);
        mark(&format!("comentar {i} (recargar visor)"), t);
    }
    let idle_after_edits = private_mb(me);

    for i in 1..=3 {
        let t = Instant::now();
        let saved = ed.undo_saved(&path).unwrap().unwrap();
        svc.reload_saved(id, &path, &saved).unwrap();
        drop(saved);
        mark(&format!("deshacer {i}"), t);
    }
    assert!(
        same_file(&path, &original),
        "tras deshacer todo, el PDF debe quedar igual que al abrir"
    );
    let idle = private_mb(me);
    println!(
        "app en reposo tras editar: {idle_after_edits:.0} MB · tras deshacer: {idle:.0} MB (objetivo: no crecer con el archivo)"
    );
    // En reposo la app no guarda copias del PDF: ni los pasos de deshacer ni el contenido recargado.
    assert!(
        idle_after_edits < 300.0,
        "la app retiene el PDF: {idle_after_edits:.0} MB"
    );
    assert!(idle < 300.0, "la app retiene el PDF: {idle:.0} MB");
}
