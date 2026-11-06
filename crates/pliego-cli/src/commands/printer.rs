//! `pliego printer`: imprimir un PDF en una impresora de Windows (W45). La app lo lanza como proceso hijo
//! (`pliego printer send`), así un PDF que tumbe PDFium no cierra la interfaz.

use std::path::PathBuf;

use anyhow::bail;
use clap::{Args, Subcommand, ValueEnum};
use pliego_core::{CancelToken, JobContext, Progress};
use pliego_printer::{Booklet, Duplex, NupOrder, Orientation, PrintOptions, ScaleMode, Subset};

#[derive(Subcommand)]
pub enum PrinterCommand {
    /// Lista las impresoras instaladas (la predeterminada primero).
    List,
    /// Papeles y capacidades de una impresora.
    Caps(CapsArgs),
    /// Hoja de una impresora en puntos: tamaño del papel y zona imprimible.
    Sheet(SheetArgs),
    /// Imprime un PDF.
    Send(SendArgs),
}

#[derive(Args)]
pub struct CapsArgs {
    /// Nombre de la impresora.
    pub printer: String,
}

#[derive(Args)]
pub struct SheetArgs {
    pub printer: String,
    /// Identificador de papel de Windows (9 = A4, 1 = Carta…). Por defecto, el de la impresora.
    #[arg(long)]
    pub paper: Option<i16>,
    #[arg(long)]
    pub landscape: bool,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum OrientationArg {
    Auto,
    Portrait,
    Landscape,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum ScaleArg {
    Fit,
    Shrink,
    Actual,
    Custom,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum DuplexArg {
    Off,
    Long,
    Short,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum SubsetArg {
    All,
    Odd,
    Even,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum NupOrderArg {
    Horizontal,
    HorizontalReversed,
    Vertical,
    VerticalReversed,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum BookletArg {
    Off,
    /// Anverso y reverso a doble cara.
    Both,
    /// Solo los anversos (luego se da la vuelta a las hojas).
    Front,
    /// Solo los reversos.
    Back,
}

#[derive(Args)]
pub struct SendArgs {
    /// PDF a imprimir.
    pub input: PathBuf,
    /// Impresora (por defecto, la predeterminada de Windows).
    #[arg(short = 'P', long)]
    pub printer: Option<String>,
    /// Páginas (desde 1): `all`, `3`, `1-3,5`.
    #[arg(short, long, default_value = "all")]
    pub pages: String,
    /// Solo las páginas impares o pares de la selección.
    #[arg(long, value_enum, default_value = "all")]
    pub subset: SubsetArg,
    /// Imprime de la última a la primera.
    #[arg(long)]
    pub reverse: bool,
    #[arg(short, long, default_value_t = 1)]
    pub copies: u32,
    /// Con varias copias, 1,1,2,2 en vez de 1,2,1,2.
    #[arg(long)]
    pub no_collate: bool,
    #[arg(long, value_enum, default_value = "auto")]
    pub orientation: OrientationArg,
    /// No girar las páginas que así caben mejor.
    #[arg(long)]
    pub no_auto_rotate: bool,
    #[arg(long, value_enum, default_value = "fit")]
    pub scale: ScaleArg,
    /// Porcentaje con `--scale custom` (10 a 400).
    #[arg(long, default_value_t = 100)]
    pub scale_percent: u32,
    #[arg(long, value_enum, default_value = "off")]
    pub duplex: DuplexArg,
    /// Imprime en escala de grises.
    #[arg(long)]
    pub grayscale: bool,
    /// No incluye comentarios ni marcas.
    #[arg(long)]
    pub no_comments: bool,
    /// Identificador de papel de Windows (9 = A4…). Por defecto, el de la impresora.
    #[arg(long)]
    pub paper: Option<i16>,
    /// Páginas por hoja: 1, 2, 4, 6, 9 o 16 (con varias, la escala es siempre «ajustar»).
    #[arg(long, default_value_t = 1)]
    pub per_sheet: u32,
    /// Orden de las páginas en la hoja con `--per-sheet`.
    #[arg(long, value_enum, default_value = "horizontal")]
    pub nup_order: NupOrderArg,
    /// Dibuja un marco alrededor de cada página (varias por hoja y folleto).
    #[arg(long)]
    pub borders: bool,
    /// Folleto: dos páginas por cara, ordenadas para doblar y grapar.
    #[arg(long, value_enum, default_value = "off")]
    pub booklet: BookletArg,
    /// Folleto de derecha a izquierda (lomo a la derecha).
    #[arg(long)]
    pub booklet_rtl: bool,
    /// Imprime en un archivo (impresoras «a PDF»; sirve para probar sin gastar papel).
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    /// Contraseña del PDF. Si falta se usa `PLIEGO_PASSWORD`.
    #[arg(long)]
    pub password: Option<String>,
}

impl SendArgs {
    fn options(&self) -> PrintOptions {
        PrintOptions {
            printer: self.printer.clone(),
            pages: self.pages.clone(),
            subset: match self.subset {
                SubsetArg::All => Subset::All,
                SubsetArg::Odd => Subset::Odd,
                SubsetArg::Even => Subset::Even,
            },
            reverse: self.reverse,
            copies: self.copies,
            collate: !self.no_collate,
            orientation: match self.orientation {
                OrientationArg::Auto => Orientation::Auto,
                OrientationArg::Portrait => Orientation::Portrait,
                OrientationArg::Landscape => Orientation::Landscape,
            },
            auto_rotate: !self.no_auto_rotate,
            scale: match self.scale {
                ScaleArg::Fit => ScaleMode::Fit,
                ScaleArg::Shrink => ScaleMode::Shrink,
                ScaleArg::Actual => ScaleMode::Actual,
                ScaleArg::Custom => ScaleMode::Custom,
            },
            scale_percent: self.scale_percent,
            duplex: match self.duplex {
                DuplexArg::Off => Duplex::Off,
                DuplexArg::Long => Duplex::LongEdge,
                DuplexArg::Short => Duplex::ShortEdge,
            },
            grayscale: self.grayscale,
            comments: !self.no_comments,
            paper: self.paper,
            output: self.output.clone(),
            pages_per_sheet: self.per_sheet,
            nup_order: match self.nup_order {
                NupOrderArg::Horizontal => NupOrder::Horizontal,
                NupOrderArg::HorizontalReversed => NupOrder::HorizontalReversed,
                NupOrderArg::Vertical => NupOrder::Vertical,
                NupOrderArg::VerticalReversed => NupOrder::VerticalReversed,
            },
            borders: self.borders,
            booklet: match self.booklet {
                BookletArg::Off => Booklet::Off,
                BookletArg::Both => Booklet::Both,
                BookletArg::Front => Booklet::FrontOnly,
                BookletArg::Back => Booklet::BackOnly,
            },
            booklet_rtl: self.booklet_rtl,
        }
    }
}

struct Bar;

impl Progress for Bar {
    fn report(&self, done: u64, total: u64, message: &str) {
        if total > 0 {
            eprintln!("[{done}/{total}] {message}");
        }
    }
}

pub fn run(c: PrinterCommand, json: bool) -> anyhow::Result<()> {
    match c {
        PrinterCommand::List => {
            let printers = pliego_printer::list_printers()?;
            if json {
                println!("{}", serde_json::to_string_pretty(&printers)?);
            } else if printers.is_empty() {
                println!("No hay impresoras instaladas.");
            } else {
                for p in printers {
                    println!("{}{}", if p.is_default { "* " } else { "  " }, p.name);
                }
            }
        }
        PrinterCommand::Caps(a) => {
            let caps = pliego_printer::capabilities(&a.printer)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&caps)?);
            } else {
                println!(
                    "{}: color {}, doble cara {}, hasta {} copias{}",
                    caps.name,
                    yes_no(caps.color),
                    yes_no(caps.duplex),
                    caps.max_copies,
                    if caps.collate { ", intercala" } else { "" }
                );
                for p in &caps.papers {
                    println!("  {:>4}  {} ({} × {} mm)", p.id, p.name, p.width_mm, p.height_mm);
                }
            }
        }
        PrinterCommand::Sheet(a) => {
            let sheet = pliego_printer::sheet_in_points(&a.printer, a.paper, a.landscape)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&sheet)?);
            } else {
                println!(
                    "Hoja {:.0} × {:.0} pt, zona imprimible {:.0} × {:.0} pt (margen izquierdo {:.1}, superior {:.1})",
                    sheet.phys_w, sheet.phys_h, sheet.area_w, sheet.area_h, sheet.off_x, sheet.off_y
                );
            }
        }
        PrinterCommand::Send(a) => {
            let options = a.options();
            if let Err(e) = options.validate() {
                bail!("{e}");
            }
            let password = a
                .password
                .clone()
                .or_else(|| std::env::var("PLIEGO_PASSWORD").ok().filter(|p| !p.is_empty()));
            let ctx = JobContext::new(&Bar, CancelToken::new());
            let report = pliego_printer::print_file(&a.input, password.as_deref(), &options, &ctx)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                for w in &report.warnings {
                    eprintln!("Aviso: {w}");
                }
                println!("{}", report.summary);
            }
        }
    }
    Ok(())
}

fn yes_no(b: bool) -> &'static str {
    if b { "sí" } else { "no" }
}
