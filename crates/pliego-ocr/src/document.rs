//! `ocr_document`: añade una capa de texto invisible a un PDF escaneado sin tocar sus imágenes.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, mpsc};

use pliego_core::{Error, JobContext, PageRange, Report, Result};
use pliego_pdf::{Colorspace, Matrix, Object, PdfDocument, PdfPage, TextFlags, WriteOptions};
use serde::{Deserialize, Serialize};

use crate::preprocess::{binarize_sauvola, denoise_median3, estimate_skew, rotate, rotate_point, rotate_quarter};
use crate::textlayer::{FONT_RESOURCE, PlacedWord, build_content};
use crate::{GrayImage, OcrEngine, TesseractEngine, tessdata};

/// Qué hacer con las páginas que ya tienen texto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum OcrMode {
    /// Reconoce solo las páginas sin texto y deja las demás intactas (por defecto).
    #[default]
    SkipText,
    /// Reconoce todas las páginas, también las que ya tienen texto (puede duplicarlo).
    Force,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OcrOptions {
    /// Idiomas al estilo Tesseract: `"spa+eng"`.
    pub langs: String,
    pub mode: OcrMode,
    /// Páginas a procesar (`"1-3,5"`, `"all"`...), desde 1.
    pub pages: String,
    /// Resolución de reconocimiento (mínimo 300 ppp; se ignora un valor menor).
    pub dpi: u32,
    pub deskew: bool,
    /// Corrige páginas giradas 90/180/270° (necesita el modelo `osd`) cambiando su `/Rotate`.
    pub rotate_pages: bool,
    /// Binarización adaptativa (ayuda con iluminación irregular).
    pub binarize: bool,
    pub denoise: bool,
    /// Hilos de reconocimiento; 0 = automático (hasta 4).
    pub threads: usize,
    /// Descarta palabras con confianza inferior (0-100).
    pub min_confidence: f32,
    /// Carpeta de modelos; `None` = búsqueda habitual.
    pub tessdata_dir: Option<PathBuf>,
    /// Contraseña del PDF de entrada, si está protegido. La copia resultante conserva el cifrado.
    #[serde(skip_serializing)]
    pub password: Option<String>,
}

impl Default for OcrOptions {
    fn default() -> Self {
        Self {
            langs: "spa+eng".into(),
            mode: OcrMode::SkipText,
            pages: "all".into(),
            dpi: 300,
            deskew: false,
            rotate_pages: false,
            binarize: false,
            denoise: false,
            threads: 0,
            min_confidence: 0.0,
            tessdata_dir: None,
            password: None,
        }
    }
}

/// Confianza mínima de `osd` para girar una página.
const ORIENTATION_MIN_CONF: f32 = 5.0;
/// Lado máximo de la imagen de reconocimiento (evita carteles gigantes).
const MAX_SIDE_PX: f32 = 12_000.0;

struct Job {
    index: u32,
    img: GrayImage,
}

/// Palabra en píxeles de la imagen (ya orientada), con la línea base y el vector «arriba».
struct PixelWord {
    text: String,
    base0: (f32, f32),
    base1: (f32, f32),
    up: (f32, f32),
    height: f32,
    confidence: f32,
}

struct PageResult {
    index: u32,
    /// Giro en sentido horario (0/90/180/270) aplicado a la imagen; la página debe girarse igual.
    rotation: u32,
    skew: f32,
    words: Vec<PixelWord>,
}

fn process(engine: &mut TesseractEngine, job: Job, o: &OcrOptions) -> Result<PageResult> {
    let mut img = job.img;
    let mut rotation = 0;
    if o.rotate_pages
        && let Some((deg, conf)) = engine.detect_orientation(&img)?
        && deg != 0
        && conf >= ORIENTATION_MIN_CONF
    {
        img = rotate_quarter(&img, deg);
        rotation = deg;
    }
    let skew = if o.deskew { estimate_skew(&img, 8.0) } else { 0.0 };
    let (cx, cy) = (img.width as f32 / 2.0, img.height as f32 / 2.0);
    if skew.abs() >= 0.1 {
        img = rotate(&img, -skew);
    }
    if o.denoise {
        img = denoise_median3(&img);
    }
    if o.binarize {
        img = binarize_sauvola(&img, 25, 0.34);
    }
    let dpi = o.dpi.max(300);
    let words = engine.recognize(&img, dpi)?;
    let back = |p: (f32, f32)| {
        if skew.abs() >= 0.1 {
            rotate_point(p.0, p.1, cx, cy, skew)
        } else {
            p
        }
    };
    let (s, c) = skew.to_radians().sin_cos();
    let up = if skew.abs() >= 0.1 { (s, -c) } else { (0.0, -1.0) };
    let words = words
        .into_iter()
        .filter(|w| w.confidence >= o.min_confidence)
        .map(|w| {
            let h = (w.bbox.y1 - w.bbox.y0).max(1.0);
            // La línea base queda ~20 % por encima del borde inferior (los descendentes sobresalen).
            let by = w.bbox.y1 - 0.2 * h;
            PixelWord {
                text: w.text,
                base0: back((w.bbox.x0, by)),
                base1: back((w.bbox.x1, by)),
                up,
                height: h,
                confidence: w.confidence,
            }
        })
        .collect();
    Ok(PageResult {
        index: job.index,
        rotation,
        skew,
        words,
    })
}

/// Convierte las palabras (píxeles) al espacio de usuario de la página.
/// `ctm`: usuario → dispositivo de la página; `scale`: píxeles por punto del renderizado.
fn place_words(words: &[PixelWord], ctm: &Matrix, scale: f32) -> Vec<PlacedWord> {
    let Some(inv) = ctm.invert() else { return Vec::new() };
    let to_user = |p: (f32, f32)| inv.transform_xy(p.0 / scale, p.1 / scale);
    let vec_user = |v: (f32, f32)| {
        let (a, b) = (inv.a * v.0 + inv.c * v.1, inv.b * v.0 + inv.d * v.1);
        let n = (a * a + b * b).sqrt().max(1e-6);
        (a / n, b / n)
    };
    words
        .iter()
        .map(|w| {
            let o = to_user(w.base0);
            let e = to_user(w.base1);
            let width = ((e.0 - o.0).powi(2) + (e.1 - o.1).powi(2)).sqrt();
            let dir = (e.0 - o.0, e.1 - o.1);
            let dn = (dir.0 * dir.0 + dir.1 * dir.1).sqrt().max(1e-6);
            PlacedWord {
                text: w.text.clone(),
                origin: o,
                dir: (dir.0 / dn, dir.1 / dn),
                up: vec_user(w.up),
                // Courier: ascendente 0,63 + descendente 0,16 ≈ 0,79 em; el alto de la caja ≈ altura de texto.
                font_size: (w.height / scale * 1.1).max(1.0),
                width,
            }
        })
        .collect()
}

fn has_text(page: &PdfPage) -> Result<bool> {
    let tp = page.to_text_page(TextFlags::empty())?;
    Ok(!tp.to_text()?.trim().is_empty())
}

fn ensure_font(doc: &mut PdfDocument) -> Result<Object> {
    let font =
        doc.new_object_from_str("<< /Type /Font /Subtype /Type1 /BaseFont /Courier /Encoding /WinAnsiEncoding >>")?;
    doc.add_object(&font)
}

fn add_text_layer(
    doc: &mut PdfDocument,
    font: &Object,
    index: u32,
    rotation: u32,
    words: &[PixelWord],
    scale: f32,
) -> Result<()> {
    let mut page = doc.load_pdf_page(index as i32)?;
    let mut obj = page.object();
    if rotation != 0 {
        let current = obj
            .get_dict_inheritable("Rotate")?
            .and_then(|o| o.as_int().ok())
            .unwrap_or(0);
        let new = (current + rotation as i32).rem_euclid(360);
        obj.dict_put("Rotate", Object::new_int(new)?)?;
    }
    let ctm = obj.page_ctm()?;
    let placed = place_words(words, &ctm, scale);
    if placed.is_empty() {
        return Ok(());
    }
    let mut resources = page.resources()?;
    let mut fonts = match resources.get_dict("Font")? {
        Some(f) if f.is_dict()? => f,
        _ => {
            let d = doc.new_dict()?;
            resources.dict_put("Font", d)?;
            resources
                .get_dict("Font")?
                .ok_or_else(|| Error::Pdf("no se pudo crear /Font".into()))?
        }
    };
    fonts.dict_put(FONT_RESOURCE, font.clone())?;
    // Equilibra el estado gráfico del contenido original antes de añadir la capa.
    page.wrap_contents(doc)?;
    page.insert_contents(doc, &build_content(&placed), true)?;
    Ok(())
}

/// Añade a `input` una capa de texto invisible reconocida con Tesseract y escribe el resultado en `output`.
///
/// Las imágenes originales no se recomprimen. Nunca sobrescribe `input`.
pub fn ocr_document(input: &Path, output: &Path, opts: &OcrOptions, ctx: &JobContext) -> Result<Report> {
    if output.exists() && same_file(input, output) {
        return Err(Error::InvalidInput(
            "el archivo de salida no puede ser el de entrada".into(),
        ));
    }
    let data_dir = match &opts.tessdata_dir {
        Some(d) => d.clone(),
        None => tessdata::resolve_for(&opts.langs)?,
    };
    let mut doc = PdfDocument::open_with_password(input, opts.password.as_deref()).map_err(|e| match e {
        Error::Password => Error::Password,
        e => Error::Pdf(format!("no se pudo abrir {}: {e}", input.display())),
    })?;
    let page_count = doc.page_count()? as u32;
    let range: PageRange = opts.pages.parse()?;
    let selected = range.resolve(page_count)?;

    // Decide qué páginas necesitan OCR.
    let mut todo: Vec<u32> = Vec::new();
    let mut skipped = 0u32;
    for &i in &selected {
        ctx.step(0, 0, "Analizando páginas")?;
        let page = doc.load_pdf_page(i as i32)?;
        if opts.mode == OcrMode::SkipText && has_text(&page)? {
            skipped += 1;
        } else {
            todo.push(i);
        }
    }

    let mut warnings = Vec::new();
    let mut total_words = 0usize;
    let total = todo.len() as u64;
    if !todo.is_empty() {
        let font = ensure_font(&mut doc)?;
        let threads = if opts.threads == 0 {
            std::thread::available_parallelism().map_or(1, |n| n.get()).min(4)
        } else {
            opts.threads
        }
        .clamp(1, todo.len().max(1));
        let (job_tx, job_rx) = mpsc::sync_channel::<Job>(threads);
        let job_rx = Mutex::new(job_rx);
        let (res_tx, res_rx) = mpsc::channel::<Result<(PageResult, f32)>>();
        let dpi = opts.dpi.max(300) as f32;

        let outcome: Result<()> = std::thread::scope(|scope| {
            for _ in 0..threads {
                let res_tx = res_tx.clone();
                let job_rx = &job_rx;
                let data_dir = data_dir.clone();
                scope.spawn(move || {
                    let mut engine = match TesseractEngine::new(&opts.langs, Some(data_dir)) {
                        Ok(e) => e,
                        Err(e) => {
                            let _ = res_tx.send(Err(e));
                            return;
                        }
                    };
                    loop {
                        let job = match job_rx.lock().map(|r| r.recv()) {
                            Ok(Ok(j)) => j,
                            _ => break,
                        };
                        let r = process(&mut engine, job, opts).map(|p| (p, 0.0));
                        if res_tx.send(r).is_err() {
                            break;
                        }
                    }
                });
            }
            drop(res_tx);

            let mut done = 0u64;
            let mut scales: std::collections::HashMap<u32, f32> = Default::default();
            let mut handle = |r: Result<(PageResult, f32)>,
                              doc: &mut PdfDocument,
                              scales: &std::collections::HashMap<u32, f32>|
             -> Result<()> {
                let (res, _) = r?;
                if res.words.is_empty() {
                    warnings.push(format!("La página {} no contiene texto reconocible.", res.index + 1));
                } else {
                    let low = res.words.iter().filter(|w| w.confidence < 40.0).count();
                    if low * 2 > res.words.len() {
                        warnings.push(format!("La página {} se reconoció con poca confianza.", res.index + 1));
                    }
                }
                total_words += res.words.len();
                let scale = scales.get(&res.index).copied().unwrap_or(dpi / 72.0);
                add_text_layer(doc, &font, res.index, res.rotation, &res.words, scale)?;
                let _ = res.skew;
                done += 1;
                ctx.step(done, total, &format!("Reconociendo página {}", res.index + 1))
            };

            let mut sent = 0usize;
            let mut received = 0usize;
            for &i in &todo {
                ctx.step(received as u64, total, &format!("Preparando página {}", i + 1))?;
                let page = doc.load_pdf_page(i as i32)?;
                let b = page.bounds()?;
                let (pw, ph) = ((b.x1 - b.x0).max(1.0), (b.y1 - b.y0).max(1.0));
                let scale = (dpi / 72.0).min(MAX_SIDE_PX / pw.max(ph));
                let pix = page.to_pixmap(&Matrix::new_scale(scale, scale), Colorspace::Gray, false, false)?;
                let img = GrayImage::from_interleaved(pix.width(), pix.height(), pix.n() as usize, pix.samples())?;
                scales.insert(i, scale);
                job_tx
                    .send(Job { index: i, img })
                    .map_err(|_| Error::Other("el reconocimiento se detuvo".into()))?;
                sent += 1;
                while let Ok(r) = res_rx.try_recv() {
                    handle(r, &mut doc, &scales)?;
                    received += 1;
                }
            }
            drop(job_tx);
            while received < sent {
                match res_rx.recv() {
                    Ok(r) => {
                        handle(r, &mut doc, &scales)?;
                        received += 1;
                    }
                    Err(_) => return Err(Error::Other("el reconocimiento terminó antes de tiempo".into())),
                }
            }
            Ok(())
        });
        outcome?;
    }

    write_atomically(&doc, output)?;
    let summary = format!(
        "{} página(s) reconocida(s), {} omitida(s) por tener ya texto, {} palabras.",
        todo.len(),
        skipped,
        total_words
    );
    Ok(Report {
        outputs: vec![output.to_path_buf()],
        warnings,
        summary,
    })
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

fn write_atomically(doc: &PdfDocument, output: &Path) -> Result<()> {
    let dir = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let tmp = tempfile::Builder::new()
        .prefix(".pliego-ocr-")
        .suffix(".pdf")
        .tempfile_in(dir)
        .map_err(|e| Error::io_at(dir, e))?;
    let mut opts = WriteOptions::default();
    opts.set_compress(true);
    let tmp_str = tmp
        .path()
        .to_str()
        .ok_or_else(|| Error::InvalidInput("ruta de salida no válida".into()))?;
    doc.save_with_options(tmp_str, &opts)?;
    tmp.persist(output).map_err(|e| Error::io_at(output, e.error))?;
    Ok(())
}
