//! `pliego stamp header|watermark|background|bates`.

use std::path::PathBuf;

use anyhow::{Context, bail};
use clap::{Args, Subcommand};
use pliego_core::{JobContext, PageRange, Report};
use pliego_stamp::{
    Background, BackgroundFill, BatesOptions, Corner, HeaderFooter, Slots, StampContext, TextStyle, Watermark,
    WatermarkContent,
};

use super::pages::{Target, finish};

#[derive(Args)]
pub struct StampArgs {
    #[command(subcommand)]
    pub op: StampOp,
}

/// Opciones de texto comunes.
#[derive(Args)]
pub struct StyleArgs {
    /// Fuente estándar: Helvetica, Times-Roman, Courier (y sus variantes -Bold, -Oblique/-Italic).
    #[arg(long, default_value = "Helvetica")]
    pub font: String,
    /// Tamaño de letra en puntos.
    #[arg(long)]
    pub size: Option<f32>,
    /// Color "#RRGGBB" o "r,g,b" con valores de 0 a 1.
    #[arg(long, default_value = "#000000")]
    pub color: String,
}

impl StyleArgs {
    fn style(&self, default_size: f32) -> anyhow::Result<TextStyle> {
        Ok(TextStyle {
            font: self.font.clone(),
            size: self.size.unwrap_or(default_size),
            color: parse_color(&self.color)?,
        })
    }
}

#[derive(Subcommand)]
pub enum StampOp {
    /// Añade encabezado y pie. Variables: {page} {pages} {date} {file}.
    Header {
        #[command(flatten)]
        t: Target,
        #[arg(short, long, default_value = "all")]
        pages: PageRange,
        #[arg(long, default_value = "")]
        header_left: String,
        #[arg(long, default_value = "")]
        header_center: String,
        #[arg(long, default_value = "")]
        header_right: String,
        #[arg(long, default_value = "")]
        footer_left: String,
        #[arg(long, default_value = "")]
        footer_center: String,
        #[arg(long, default_value = "")]
        footer_right: String,
        /// Distancia al borde en puntos.
        #[arg(long, default_value_t = 36.0)]
        margin: f32,
        #[command(flatten)]
        style: StyleArgs,
    },
    /// Añade una marca de agua de texto o de imagen.
    Watermark {
        #[command(flatten)]
        t: Target,
        #[arg(short, long, default_value = "all")]
        pages: PageRange,
        #[arg(long, group = "content")]
        text: Option<String>,
        #[arg(long, group = "content")]
        image: Option<PathBuf>,
        /// Fracción del ancho de página que ocupa la imagen.
        #[arg(long, default_value_t = 0.5)]
        scale: f32,
        #[arg(long, default_value_t = 0.3)]
        opacity: f32,
        /// Grados antihorarios (solo texto).
        #[arg(long, default_value_t = 45.0, allow_hyphen_values = true)]
        rotation: f32,
        /// Colocarla detrás del contenido.
        #[arg(long)]
        behind: bool,
        #[command(flatten)]
        style: StyleArgs,
    },
    /// Pone un fondo de color o de imagen detrás del contenido.
    Background {
        #[command(flatten)]
        t: Target,
        #[arg(short, long, default_value = "all")]
        pages: PageRange,
        #[arg(long, group = "fill")]
        color: Option<String>,
        #[arg(long, group = "fill")]
        image: Option<PathBuf>,
        #[arg(long, default_value_t = 1.0)]
        opacity: f32,
    },
    /// Numeración Bates consecutiva.
    Bates {
        #[command(flatten)]
        t: Target,
        #[arg(short, long, default_value = "all")]
        pages: PageRange,
        #[arg(long, default_value = "")]
        prefix: String,
        #[arg(long, default_value = "")]
        suffix: String,
        #[arg(long, default_value_t = 6)]
        digits: u32,
        #[arg(long, default_value_t = 1)]
        start: u64,
        /// top-left, top-right, bottom-left o bottom-right.
        #[arg(long, default_value = "bottom-right")]
        corner: String,
        #[arg(long, default_value_t = 36.0)]
        margin: f32,
        #[command(flatten)]
        style: StyleArgs,
    },
}

fn parse_color(s: &str) -> anyhow::Result<[f32; 3]> {
    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() != 6 {
            bail!("color no válido: {s} (usa #RRGGBB)");
        }
        let v = u32::from_str_radix(hex, 16).with_context(|| format!("color no válido: {s}"))?;
        return Ok([
            (v >> 16 & 255) as f32 / 255.0,
            (v >> 8 & 255) as f32 / 255.0,
            (v & 255) as f32 / 255.0,
        ]);
    }
    let v: Vec<f32> = s
        .split(',')
        .map(|x| x.trim().parse::<f32>().with_context(|| format!("color no válido: {s}")))
        .collect::<Result<_, _>>()?;
    let [r, g, b] = <[f32; 3]>::try_from(v).map_err(|_| anyhow::anyhow!("color no válido: {s} (usa r,g,b)"))?;
    if [r, g, b].iter().any(|c| !(0.0..=1.0).contains(c)) {
        bail!("los componentes del color van de 0 a 1");
    }
    Ok([r, g, b])
}

pub fn run(a: StampArgs, json: bool) -> anyhow::Result<()> {
    use StampOp::*;
    let job = JobContext::silent();
    let (t, summary) = match a.op {
        Header {
            t,
            pages,
            header_left,
            header_center,
            header_right,
            footer_left,
            footer_center,
            footer_right,
            margin,
            style,
        } => {
            let mut doc = t.open()?;
            let sel = pages.resolve(pliego_organize::page_count(&doc)?)?;
            let hf = HeaderFooter {
                header: Slots {
                    left: header_left,
                    center: header_center,
                    right: header_right,
                },
                footer: Slots {
                    left: footer_left,
                    center: footer_center,
                    right: footer_right,
                },
                style: style.style(10.0)?,
                margin,
            };
            let file = t
                .input
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            pliego_stamp::header_footer(&mut doc, &sel, &hf, &StampContext::today(file), &job)?;
            finish_save(&t, &doc)?;
            (t, format!("encabezado y pie en {} páginas", sel.len()))
        }
        Watermark {
            t,
            pages,
            text,
            image,
            scale,
            opacity,
            rotation,
            behind,
            style,
        } => {
            let mut doc = t.open()?;
            let sel = pages.resolve(pliego_organize::page_count(&doc)?)?;
            let content = match (text, image) {
                (Some(text), None) => WatermarkContent::Text {
                    text,
                    style: style.style(72.0)?,
                },
                (None, Some(path)) => WatermarkContent::Image { path, scale },
                _ => bail!("indica --text o --image"),
            };
            pliego_stamp::watermark(
                &mut doc,
                &sel,
                &self::Watermark {
                    content,
                    opacity,
                    rotation,
                    behind,
                },
                &job,
            )?;
            finish_save(&t, &doc)?;
            (t, format!("marca de agua en {} páginas", sel.len()))
        }
        Background {
            t,
            pages,
            color,
            image,
            opacity,
        } => {
            let mut doc = t.open()?;
            let sel = pages.resolve(pliego_organize::page_count(&doc)?)?;
            let fill = match (color, image) {
                (Some(c), None) => BackgroundFill::Color { rgb: parse_color(&c)? },
                (None, Some(path)) => BackgroundFill::Image { path },
                _ => bail!("indica --color o --image"),
            };
            pliego_stamp::background(&mut doc, &sel, &self::Background { fill, opacity }, &job)?;
            finish_save(&t, &doc)?;
            (t, format!("fondo en {} páginas", sel.len()))
        }
        Bates {
            t,
            pages,
            prefix,
            suffix,
            digits,
            start,
            corner,
            margin,
            style,
        } => {
            let mut doc = t.open()?;
            let sel = pages.resolve(pliego_organize::page_count(&doc)?)?;
            let corner = match corner.as_str() {
                "top-left" => Corner::TopLeft,
                "top-right" => Corner::TopRight,
                "bottom-left" => Corner::BottomLeft,
                "bottom-right" => Corner::BottomRight,
                other => bail!("esquina desconocida: {other}"),
            };
            let o = BatesOptions {
                prefix,
                suffix,
                digits,
                start,
                corner,
                style: style.style(10.0)?,
                margin,
            };
            pliego_stamp::bates(&mut doc, &sel, &o, &job)?;
            finish_save(&t, &doc)?;
            (t, format!("numeración Bates en {} páginas", sel.len()))
        }
    };
    finish(Report::with_output(t.out()?.to_path_buf(), summary), json)
}

fn finish_save(t: &Target, doc: &pliego_organize::PdfDocument) -> anyhow::Result<()> {
    Ok(pliego_organize::save(doc, t.out()?)?)
}
