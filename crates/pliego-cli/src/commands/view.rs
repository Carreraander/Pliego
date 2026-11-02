use std::path::PathBuf;

use anyhow::Context;
use clap::Args;
use pliego_core::{EngineDocument, PageRange, PdfEngine};
use pliego_engine_mupdf::{MupdfDocument, MupdfEngine, encode_png};

#[derive(Args)]
pub struct OpenArgs {
    /// Archivo PDF de entrada.
    pub input: PathBuf,
    /// Contraseña del documento, si la tiene.
    #[arg(long)]
    pub password: Option<String>,
}

impl OpenArgs {
    pub fn open(&self) -> anyhow::Result<MupdfDocument> {
        MupdfEngine
            .open(&self.input, self.password.as_deref())
            .with_context(|| format!("no se pudo abrir {}", self.input.display()))
    }
}

#[derive(Args)]
pub struct InfoArgs {
    #[command(flatten)]
    pub open: OpenArgs,
}

#[derive(Args)]
pub struct RenderArgs {
    #[command(flatten)]
    pub open: OpenArgs,
    /// Páginas (desde 1): "1-3,5", "all", "last".
    #[arg(short, long, default_value = "1")]
    pub pages: PageRange,
    /// Resolución en puntos por pulgada.
    #[arg(long, default_value_t = 150.0)]
    pub dpi: f32,
    /// Carpeta de salida.
    #[arg(short, long, default_value = ".")]
    pub output: PathBuf,
}

#[derive(Args)]
pub struct TextArgs {
    #[command(flatten)]
    pub open: OpenArgs,
    #[arg(short, long, default_value = "all")]
    pub pages: PageRange,
}

#[derive(Args)]
pub struct SearchArgs {
    #[command(flatten)]
    pub open: OpenArgs,
    /// Texto a buscar (sin distinguir mayúsculas).
    pub needle: String,
}

pub fn info(a: InfoArgs, json: bool) -> anyhow::Result<()> {
    let info = a.open.open()?.info()?;
    if json {
        println!("{}", serde_json::to_string_pretty(&info)?);
    } else {
        println!("Páginas:    {}", info.page_count);
        let show = |k: &str, v: &Option<String>| {
            if let Some(v) = v {
                println!("{k:<11} {v}");
            }
        };
        show("Título:", &info.title);
        show("Autor:", &info.author);
        show("Asunto:", &info.subject);
        show("Creador:", &info.creator);
        show("Productor:", &info.producer);
        show("Formato:", &info.format);
        show("Cifrado:", &info.encryption);
    }
    Ok(())
}

pub fn render(a: RenderArgs, json: bool) -> anyhow::Result<()> {
    let doc = a.open.open()?;
    let pages = a.pages.resolve(doc.page_count()?)?;
    std::fs::create_dir_all(&a.output)?;
    let stem = a.open.input.file_stem().and_then(|s| s.to_str()).unwrap_or("pagina");
    let mut written = Vec::new();
    for p in pages {
        let img = doc.render_page(p, a.dpi / 72.0)?;
        let path = a.output.join(format!("{stem}-{:03}.png", p + 1));
        std::fs::write(&path, encode_png(&img)?).with_context(|| format!("no se pudo escribir {}", path.display()))?;
        if !json {
            println!("{}", path.display());
        }
        written.push(path);
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&written)?);
    }
    Ok(())
}

pub fn text(a: TextArgs, json: bool) -> anyhow::Result<()> {
    let doc = a.open.open()?;
    let pages = a.pages.resolve(doc.page_count()?)?;
    let mut out = Vec::new();
    for p in pages {
        out.push(serde_json::json!({ "page": p + 1, "text": doc.page_text(p)? }));
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        for item in out {
            print!("{}", item["text"].as_str().unwrap_or_default());
        }
    }
    Ok(())
}

pub fn search(a: SearchArgs, json: bool) -> anyhow::Result<()> {
    let doc = a.open.open()?;
    let mut hits = Vec::new();
    for p in 0..doc.page_count()? {
        let h = doc.search_page(p, &a.needle, 500)?;
        if !h.quads.is_empty() {
            hits.push(h);
        }
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&hits)?);
    } else if hits.is_empty() {
        println!("Sin coincidencias.");
    } else {
        for h in &hits {
            println!("Página {}: {} coincidencia(s)", h.page + 1, h.quads.len());
        }
    }
    Ok(())
}
