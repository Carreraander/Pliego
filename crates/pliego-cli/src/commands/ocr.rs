use std::path::PathBuf;

use anyhow::{Context, bail};
use clap::Args;
use pliego_core::{CancelToken, JobContext, Progress};
use pliego_ocr::{OcrMode, OcrOptions};

#[derive(Args)]
pub struct OcrArgs {
    /// PDF escaneado de entrada.
    #[arg(required_unless_present_any = ["list_languages", "list_downloadable", "download"])]
    pub input: Option<PathBuf>,
    /// PDF de salida con texto buscable (por defecto `<entrada>-ocr.pdf`).
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    /// Idiomas de Tesseract separados por `+`.
    #[arg(short, long, default_value = "spa+eng")]
    pub lang: String,
    /// Salta las páginas que ya tienen texto (por defecto).
    #[arg(long, conflicts_with = "force")]
    pub skip_text: bool,
    /// Reconoce todas las páginas aunque ya tengan texto.
    #[arg(long)]
    pub force: bool,
    /// Páginas a procesar (desde 1): "1-3,5", "all".
    #[arg(short, long, default_value = "all")]
    pub pages: String,
    /// Endereza las páginas torcidas antes de reconocer.
    #[arg(long)]
    pub deskew: bool,
    /// Gira las páginas que estén de lado o boca abajo (necesita el modelo `osd`).
    #[arg(long)]
    pub rotate: bool,
    /// Binarización adaptativa (iluminación irregular).
    #[arg(long)]
    pub binarize: bool,
    /// Elimina ruido de la imagen antes de reconocer.
    #[arg(long)]
    pub denoise: bool,
    /// Resolución de reconocimiento (mínimo 300).
    #[arg(long, default_value_t = 300)]
    pub dpi: u32,
    /// Hilos de reconocimiento (0 = automático).
    #[arg(short, long, default_value_t = 0)]
    pub jobs: usize,
    /// Carpeta con los modelos `*.traineddata`.
    #[arg(long)]
    pub tessdata: Option<PathBuf>,
    /// Lista los idiomas instalados y termina.
    #[arg(long)]
    pub list_languages: bool,
    /// Lista los idiomas que se pueden descargar y termina.
    #[arg(long)]
    pub list_downloadable: bool,
    /// Descarga (con verificación de huella) los modelos de idioma indicados, p. ej. `--download fra`, y termina.
    #[arg(long, value_name = "IDIOMA")]
    pub download: Vec<String>,
}

struct Bar;

impl Progress for Bar {
    fn report(&self, done: u64, total: u64, message: &str) {
        if total > 0 {
            eprintln!("[{done}/{total}] {message}");
        }
    }
}

pub fn run(a: OcrArgs, json: bool) -> anyhow::Result<()> {
    if a.list_languages {
        let langs = pliego_ocr::available_languages();
        if json {
            println!("{}", serde_json::to_string_pretty(&langs)?);
        } else if langs.is_empty() {
            println!("No hay idiomas instalados. Carpeta de modelos: {}", dirs_hint());
        } else {
            println!("{}", langs.join("\n"));
        }
        return Ok(());
    }
    if a.list_downloadable {
        let langs = pliego_ocr::downloadable_languages();
        if json {
            println!("{}", serde_json::to_string_pretty(&langs)?);
        } else {
            println!("{}", langs.join("\n"));
        }
        return Ok(());
    }
    if !a.download.is_empty() {
        let ctx = JobContext::new(&Bar, CancelToken::new());
        let mut paths = Vec::new();
        for code in &a.download {
            paths.push(pliego_ocr::download_language(code, &ctx)?);
        }
        if json {
            println!("{}", serde_json::to_string_pretty(&paths)?);
        } else {
            for p in &paths {
                println!("Modelo instalado: {}", p.display());
            }
        }
        return Ok(());
    }
    let input = a.input.context("falta el PDF de entrada")?;
    if a.skip_text && a.force {
        bail!("--skip-text y --force no se pueden usar juntos");
    }
    let output = a.output.unwrap_or_else(|| {
        let stem = input
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "salida".into());
        input.with_file_name(format!("{stem}-ocr.pdf"))
    });
    let opts = OcrOptions {
        langs: a.lang,
        mode: if a.force { OcrMode::Force } else { OcrMode::SkipText },
        pages: a.pages,
        dpi: a.dpi,
        deskew: a.deskew,
        rotate_pages: a.rotate,
        binarize: a.binarize,
        denoise: a.denoise,
        threads: a.jobs,
        tessdata_dir: a.tessdata,
        ..OcrOptions::default()
    };
    let ctx = JobContext::new(&Bar, CancelToken::new());
    let report = pliego_ocr::ocr_document(&input, &output, &opts, &ctx)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        for w in &report.warnings {
            eprintln!("Aviso: {w}");
        }
        println!("{}\nGuardado en {}", report.summary, output.display());
    }
    Ok(())
}

fn dirs_hint() -> String {
    pliego_ocr::tessdata::user_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "(desconocida)".into())
}
