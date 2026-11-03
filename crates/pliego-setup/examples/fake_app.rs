//! `pliego-app.exe` de pruebas para el instalador y el lanzador (no se distribuye).
//!
//! - Con un archivo `crash` junto al ejecutable (o `PLIEGO_FAKE_CRASH=1`) sale enseguida con código 3.
//! - Si no, vive 12 s y escribe `healthy` como haría la app real tras 10 s de funcionamiento.
//! - `--exit <n>` sale con ese código. Escribe sus argumentos en `last-args.txt` junto al ejecutable.
use std::time::Duration;

fn main() {
    let exe = std::env::current_exe().unwrap_or_default();
    let dir = exe.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let _ = std::fs::write(dir.join("last-args.txt"), args.join("\n"));
    if let Some(i) = args.iter().position(|a| a == "--exit") {
        std::process::exit(args.get(i + 1).and_then(|v| v.parse().ok()).unwrap_or(0));
    }
    if dir.join("crash").exists() || std::env::var_os("PLIEGO_FAKE_CRASH").is_some() {
        std::process::exit(3);
    }
    std::thread::sleep(Duration::from_secs(12));
    let _ = std::fs::write(dir.join("healthy"), b"ok");
}
