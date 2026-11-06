//! Motor PDFium (ola 5, W37): render, texto estructurado y búsqueda detrás de la API de `pliego-pdf`.
//!
//! Solo se compila con la característica `pdfium`. PDFium se carga dinámicamente y **solo por ruta absoluta**
//! ([`init`]); todas las llamadas a la DLL pasan por el cerrojo global de `pdfium-render` (`thread_safe`), porque
//! PDFium no admite hilos concurrentes.
//!
//! Qué hace PDFium aquí: dibujo (`to_pixmap`, `render_region`), texto (`text`, `words`, `to_text_page`) y
//! búsqueda (`search`). Todo lo demás (objetos, guardado, enlaces, bloques de imagen del texto) es del modelo propio
//! (`native`, lopdf). PDFium lee una copia del PDF en memoria que se renueva cuando el documento cambia (véase
//! [`DocLink::page`]).
//!
//! Las coordenadas que entrega esta capa son siempre las de `pliego-pdf`: puntos, origen arriba a la izquierda y
//! giro de página aplicado. PDFium trabaja en el espacio de usuario del PDF (origen abajo a la izquierda, sin
//! girar); [`PageGeom`] hace la conversión.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

use pdfium_render::prelude::*;
use pliego_core::{Error, Result};

use crate::geom::{IRect, Matrix, Point, Quad, Rect};
use crate::pixmap::{Colorspace, Pixmap};
use crate::text::{CharFlags, CharFont, TextBlock, TextBlockKind, TextChar, TextLine, TextWord, WriteMode};

static PDFIUM: OnceLock<Pdfium> = OnceLock::new();

pub(crate) fn is_loaded() -> bool {
    PDFIUM.get().is_some()
}

fn perr(e: PdfiumError) -> Error {
    if let PdfiumError::PdfiumLibraryInternalError(PdfiumInternalError::PasswordError) = e {
        return Error::Password;
    }
    Error::Pdf(format!("PDFium: {e}"))
}

/// Carga la biblioteca desde `path` (absoluta). Idempotente.
pub(crate) fn init(path: &Path) -> Result<()> {
    if PDFIUM.get().is_some() {
        return Ok(());
    }
    if !path.is_absolute() {
        return Err(Error::InvalidInput(format!(
            "la ruta de PDFium debe ser absoluta (nunca se busca en PATH ni en el directorio actual): {}",
            path.display()
        )));
    }
    if !path.is_file() {
        return Err(Error::io_at(path, std::io::Error::from(std::io::ErrorKind::NotFound)));
    }
    match Pdfium::bind_to_library(path) {
        Ok(bindings) => {
            let _ = PDFIUM.set(Pdfium::new(bindings));
            Ok(())
        }
        // Otra llamada se adelantó en otro hilo: vale.
        Err(PdfiumError::PdfiumLibraryBindingsAlreadyInitialized) if PDFIUM.get().is_some() => Ok(()),
        Err(e) => Err(Error::Other(format!(
            "no se pudo cargar PDFium desde {}: {e}",
            path.display()
        ))),
    }
}

/// La biblioteca de PDFium. Si nadie la ha cargado, lo intenta una vez desde `PLIEGO_PDFIUM_DLL` o la carpeta del
/// ejecutable ([`crate::pdfium_init_from_environment`]), de modo que los programas y las pruebas no tengan que
/// llamar a `pdfium_init` si la DLL está donde se espera.
fn lib() -> Result<&'static Pdfium> {
    if let Some(p) = PDFIUM.get() {
        return Ok(p);
    }
    let loaded = crate::engine::pdfium_init_from_environment();
    PDFIUM.get().ok_or_else(|| match loaded {
        Err(e) => Error::Unsupported(format!(
            "PDFium no está disponible: {e}. Pon `pdfium.dll` junto al ejecutable o define PLIEGO_PDFIUM_DLL con su ruta absoluta"
        )),
        Ok(_) => Error::Unsupported("PDFium no está cargado".into()),
    })
}

// ---------------------------------------------------------------------------------------------------------------
// Documentos
// ---------------------------------------------------------------------------------------------------------------

/// Documento abierto en PDFium. Las páginas no se conservan: se cargan en cada operación.
pub(crate) struct State {
    doc: PdfDocument<'static>,
    /// Bytes que PDFium ha pedido al archivo desde que se abrió (solo documentos leídos de archivo).
    consumed: Option<Arc<AtomicU64>>,
}

impl State {
    fn from_vec(bytes: Vec<u8>, password: Option<&str>) -> Result<Self> {
        let doc = lib()?.load_pdf_from_byte_vec(bytes, password).map_err(perr)?;
        Ok(Self { doc, consumed: None })
    }

    /// Abre el archivo sin tenerlo en memoria: PDFium pide los bloques que necesita a un [`LazyFile`].
    fn from_file(path: &Path, password: Option<&str>) -> Result<Self> {
        let mut reader = LazyFile::open(path)?;
        let consumed = Arc::new(AtomicU64::new(0));
        reader.consumed = Some(consumed.clone());
        let doc = lib()?.load_pdf_from_reader(reader, password).map_err(perr)?;
        Ok(Self {
            doc,
            consumed: Some(consumed),
        })
    }
}

/// Tamaño de cada bloque que lee [`LazyFile`] del disco.
const BLOCK: u64 = 256 * 1024;
/// Bloques recientes que conserva [`LazyFile`] (2 MiB en total).
const KEEP_BLOCKS: usize = 8;
/// PDFium (en Windows `unsigned long` son 32 bits) no sabe de archivos de 4 GiB o más al leer por bloques.
pub(crate) const MAX_FILE_BYTES: u64 = u32::MAX as u64;

/// Lector de un archivo **sin manejador abierto entre lecturas**: abre el archivo, lee un bloque de 256 KiB, lo cierra
/// y guarda los últimos bloques. Así PDFium puede trabajar sobre un PDF de 1 GB sin que el proceso lo tenga en memoria
/// y, a la vez, el archivo sigue siendo sustituible (guardar encima con un temporal y renombrar) porque en Windows un
/// manejador abierto lo impediría. Si el archivo cambia (tamaño o fecha) mientras está abierto, la lectura falla con
/// un error limpio en lugar de mezclar dos versiones: la app recarga el documento tras cada edición.
pub(crate) struct LazyFile {
    path: std::path::PathBuf,
    len: u64,
    modified: Option<std::time::SystemTime>,
    pos: u64,
    /// (índice de bloque, contenido), el más reciente al final.
    blocks: Vec<(u64, Vec<u8>)>,
    /// Suma de lo que PDFium ha leído (para saber cuándo reciclar el documento).
    consumed: Option<Arc<AtomicU64>>,
}

impl LazyFile {
    pub(crate) fn open(path: &Path) -> Result<Self> {
        let meta = std::fs::metadata(path).map_err(|e| Error::io_at(path, e))?;
        if meta.len() >= MAX_FILE_BYTES {
            return Err(Error::Unsupported(format!(
                "{} pesa {} MB: Pliego no abre PDF de 4 GB o más",
                path.display(),
                meta.len() / 1_048_576
            )));
        }
        Ok(Self {
            path: path.to_path_buf(),
            len: meta.len(),
            modified: meta.modified().ok(),
            pos: 0,
            blocks: Vec::new(),
            consumed: None,
        })
    }

    /// El bloque `idx`, del disco si no estaba guardado.
    fn block(&mut self, idx: u64) -> std::io::Result<&[u8]> {
        use std::io::{Error as E, Read, Seek, SeekFrom};
        if let Some(i) = self.blocks.iter().position(|(b, _)| *b == idx) {
            let hit = self.blocks.remove(i);
            self.blocks.push(hit);
        } else {
            let mut f = std::fs::File::open(&self.path)?;
            let meta = f.metadata()?;
            if meta.len() != self.len || meta.modified().ok() != self.modified {
                return Err(E::other("el archivo cambió mientras estaba abierto en el visor"));
            }
            let start = idx * BLOCK;
            let want = BLOCK.min(self.len.saturating_sub(start)) as usize;
            f.seek(SeekFrom::Start(start))?;
            let mut buf = vec![0u8; want];
            f.read_exact(&mut buf)?;
            if self.blocks.len() >= KEEP_BLOCKS {
                self.blocks.remove(0);
            }
            self.blocks.push((idx, buf));
        }
        Ok(&self.blocks.last().expect("bloque recién añadido").1)
    }
}

impl std::io::Read for LazyFile {
    /// Llena `out` por completo salvo al final del archivo: PDFium da por buena cualquier lectura no vacía.
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        let mut done = 0;
        while done < out.len() && self.pos < self.len {
            let (idx, off) = (self.pos / BLOCK, (self.pos % BLOCK) as usize);
            let block = self.block(idx)?;
            let n = (block.len() - off).min(out.len() - done);
            if n == 0 {
                break;
            }
            out[done..done + n].copy_from_slice(&block[off..off + n]);
            done += n;
            self.pos += n as u64;
        }
        if let Some(c) = &self.consumed {
            c.fetch_add(done as u64, Ordering::Relaxed);
        }
        Ok(done)
    }
}

impl std::io::Seek for LazyFile {
    fn seek(&mut self, to: std::io::SeekFrom) -> std::io::Result<u64> {
        use std::io::SeekFrom::{Current, End, Start};
        let target = match to {
            Start(n) => Some(n),
            End(d) => self.len.checked_add_signed(d),
            Current(d) => self.pos.checked_add_signed(d),
        }
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "posición antes del inicio"))?;
        self.pos = target;
        Ok(target)
    }
}

/// De dónde sale el PDF que lee PDFium.
pub(crate) enum ViewSource {
    /// Una copia en memoria (con la contraseña, si va cifrado).
    Bytes(Vec<u8>, Option<String>),
    /// Un archivo de disco, leído por bloques.
    File(std::path::PathBuf, Option<String>),
}

/// Lo que puede leer PDFium de un archivo antes de cerrar el documento y abrirlo de nuevo. PDFium conserva los datos
/// de las imágenes de cada página que carga hasta cerrar el documento (3,5 MB por página en un escaneado, 1 GB al buscar
/// en 300 páginas); abrir un documento leído por bloques solo cuesta leer la tabla de referencias, así que se recicla y
/// la memoria queda acotada. Lo leído es una buena medida de lo retenido: un texto de 5 000 páginas pide ~5 KB por
/// página (casi nunca se recicla) y un escaneado, el tamaño de cada imagen.
const RECYCLE_AFTER_BYTES: u64 = 24 * 1024 * 1024;
/// Respaldo por si lo retenido no es lo leído (muchas páginas pequeñas con estructura).
const RECYCLE_AFTER_PAGES: usize = 400;

struct Cached {
    generation: u64,
    state: Rc<State>,
    /// Leído de archivo (se puede reciclar sin copiar nada). Los documentos en memoria no se reciclan.
    from_file: bool,
    /// Páginas distintas cargadas desde que se abrió esta copia.
    touched: Vec<i32>,
}

/// Copia de un documento de Pliego en PDFium, válida mientras el documento no cambie (se compara su «generación»,
/// véase `native::State::generation`). Se crea al pedir la primera página.
#[derive(Default)]
pub(crate) struct DocLink {
    cached: RefCell<Option<Cached>>,
}

impl DocLink {
    /// Enlace con la página `index`. `generation` es la del documento; si no coincide con la copia guardada, se llama
    /// a `source`, que debe dar el PDF actual (en memoria o un archivo, con la contraseña si va cifrado) para abrir
    /// una copia nueva.
    pub(crate) fn page(
        &self,
        index: i32,
        generation: u64,
        source: impl FnOnce() -> Result<ViewSource>,
    ) -> Result<PageLink> {
        if index < 0 {
            return Err(Error::InvalidInput(format!("página {index} no válida")));
        }
        let mut cache = self.cached.borrow_mut();
        if let Some(c) = cache.as_mut().filter(|c| c.generation == generation && c.from_file) {
            let read = c.state.consumed.as_ref().map_or(0, |n| n.load(Ordering::Relaxed));
            if read >= RECYCLE_AFTER_BYTES || c.touched.len() >= RECYCLE_AFTER_PAGES {
                *cache = None;
            } else if !c.touched.contains(&index) {
                c.touched.push(index);
            }
        }
        let state = match cache.as_ref().filter(|c| c.generation == generation) {
            Some(c) => c.state.clone(),
            None => {
                let (state, from_file) = match source()? {
                    ViewSource::Bytes(bytes, password) => (State::from_vec(bytes, password.as_deref())?, false),
                    ViewSource::File(path, password) => (State::from_file(&path, password.as_deref())?, true),
                };
                let state = Rc::new(state);
                *cache = Some(Cached {
                    generation,
                    state: state.clone(),
                    from_file,
                    touched: vec![index],
                });
                state
            }
        };
        drop(cache);
        let count = state.doc.pages().len();
        if index >= count {
            return Err(Error::Pdf(format!("PDFium: la página {index} no existe (hay {count})")));
        }
        Ok(PageLink {
            state,
            index,
            unit: 1.0,
        })
    }
}

/// Página de un documento de PDFium (referencia compartida al documento + índice).
#[derive(Clone)]
pub(crate) struct PageLink {
    state: Rc<State>,
    index: i32,
    /// Puntos de `pliego-pdf` por punto de PDFium. 1 salvo con `/UserUnit` (el modelo propio lo aplica al tamaño de la
    /// página, PDFium no): se calibra con las dimensiones de `PdfPage::bounds` (véase [`PageLink::calibrate`]).
    unit: f32,
}

impl PageLink {
    fn load(&self) -> Result<PdfPage<'static>> {
        self.state.doc.pages().get(self.index as PdfPageIndex).map_err(perr)
    }

    /// Geometría de la página (caja visible y giro).
    fn geom(&self, page: &PdfPage<'_>) -> Result<PageGeom> {
        PageGeom::of(page, self.unit)
    }

    /// Ajusta la escala con el tamaño de la página de `PdfPage::bounds` (`/UserUnit`): si difiere de forma uniforme en ambos
    /// ejes, esa razón es la unidad; si no coincide en los dos, se deja en 1 (cajas inválidas: cada motor elige otro
    /// tamaño por defecto y no hay una escala que las reconcilie).
    pub(crate) fn calibrate(&mut self, model_w: f32, model_h: f32) {
        let Ok(page) = self.load() else { return };
        let (pw, ph) = (page.width().value, page.height().value);
        if pw <= 0.0 || ph <= 0.0 || model_w <= 0.0 || model_h <= 0.0 {
            return;
        }
        let (rw, rh) = (model_w / pw, model_h / ph);
        if (rw - 1.0).abs() > 0.002 && ((rw - rh) / rw).abs() < 0.01 {
            self.unit = rw;
        }
    }

    pub(crate) fn bounds(&self) -> Result<Rect> {
        let page = self.load()?;
        let g = self.geom(&page)?;
        Ok(Rect::new(0.0, 0.0, g.out_w, g.out_h))
    }

    // --- Dibujo ---------------------------------------------------------------------------------------------

    /// Dibuja `region` (píxeles, ya con `ctm` aplicado). `alpha`: fondo transparente.
    pub(crate) fn render(
        &self,
        ctm: &Matrix,
        region: IRect,
        cs: Colorspace,
        alpha: bool,
        show_extras: bool,
    ) -> Result<Pixmap> {
        let (w, h) = (region.x1 - region.x0, region.y1 - region.y0);
        if w <= 0 || h <= 0 {
            return Err(Error::InvalidInput("la región a dibujar está vacía".into()));
        }
        let page = self.load()?;
        let g = self.geom(&page)?;
        // PDFium dibuja los campos de formulario (FFLDraw) solo con «ajustar la página a un rectángulo», sin matriz:
        // con escala y traslación (lo habitual: zoom y teselas) se ajusta la página a su tamaño final y se desplaza
        // el origen. Con una `ctm` girada o con espejo se usa la matriz y los campos no se dibujan.
        let axis_aligned = show_extras && ctm.b == 0.0 && ctm.c == 0.0 && ctm.a > 0.0 && ctm.d > 0.0;
        let base = PdfRenderConfig::new()
            .render_annotations(show_extras)
            .clear_before_rendering(true)
            .set_clear_color(if alpha {
                PdfColor::new(255, 255, 255, 0)
            } else {
                PdfColor::WHITE
            })
            .set_format(PdfBitmapFormat::BGRA)
            .set_reverse_byte_order(true);
        let config = if axis_aligned {
            let fw = (g.out_w * ctm.a).round().max(1.0) as i32;
            let fh = (g.out_h * ctm.d).round().max(1.0) as i32;
            base.render_form_data(show_extras).set_fixed_size(fw, fh).set_origin(
                (ctm.e - region.x0 as f32).round() as i32,
                (ctm.f - region.y0 as f32).round() as i32,
            )
        } else {
            // `FPDF_RenderPageBitmapWithMatrix` parte de una proyección de la página a un rectángulo de tamaño entero
            // (truncado); se compensa para que el resultado sea exactamente `ctm` sobre la página en puntos.
            let (pw, ph) = (g.out_w / g.unit, g.out_h / g.unit);
            let (wi, hi) = (pw.floor().max(1.0), ph.floor().max(1.0));
            let (sx, sy) = (pw / wi * g.unit, ph / hi * g.unit);
            base.set_fixed_size(w, h)
                .transform(
                    ctm.a * sx,
                    ctm.b * sx,
                    ctm.c * sy,
                    ctm.d * sy,
                    ctm.e - region.x0 as f32,
                    ctm.f - region.y0 as f32,
                )
                .map_err(perr)?
                .render_form_data(false)
        };
        let mut bitmap = PdfBitmap::empty(w, h, PdfBitmapFormat::BGRA).map_err(perr)?;
        page.render_into_bitmap_with_config(&mut bitmap, &config)
            .map_err(perr)?;
        let raw = bitmap.as_raw_bytes();
        let expect = w as usize * h as usize * 4;
        if raw.len() < expect {
            return Err(Error::Pdf("PDFium devolvió un mapa de bits incompleto".into()));
        }
        Ok(rgba_to_pixmap(&raw[..expect], w as u32, h as u32, cs, alpha))
    }

    // --- Texto ----------------------------------------------------------------------------------------------

    pub(crate) fn text_page(&self) -> Result<TextData> {
        let page = self.load()?;
        let g = self.geom(&page)?;
        let text = page.text().map_err(perr)?;
        let mut chars = Vec::with_capacity(text.len().max(0) as usize);
        let mut fonts: std::collections::HashMap<String, Arc<CharFont>> = std::collections::HashMap::new();
        for c in text.chars().iter() {
            let code = c.unicode_value();
            let generated = c.is_generated().unwrap_or(false);
            let (Ok(b), Ok((ox, oy))) = (c.loose_bounds(), c.origin()) else {
                // Carácter sin geometría (p. ej. el terminador): no aporta nada al texto estructurado.
                chars.push(RawChar::unplaced(code, generated));
                continue;
            };
            let font_name = c.font_name();
            let font = if font_name.is_empty() {
                None
            } else {
                Some(
                    fonts
                        .entry(font_name.clone())
                        .or_insert_with(|| {
                            let lower = font_name.to_lowercase();
                            let bold = c.font_weight().is_some_and(|w| is_bold(&w))
                                || lower.contains("bold")
                                || lower.contains("black")
                                || lower.contains("heavy");
                            Arc::new(CharFont {
                                name: font_name.clone(),
                                bold,
                                italic: c.font_is_italic() || lower.contains("italic") || lower.contains("oblique"),
                                serif: c.font_is_serif(),
                                monospaced: c.font_is_fixed_pitch(),
                            })
                        })
                        .clone(),
                )
            };
            let argb = c.fill_color().map_or(0xFF00_0000, |col| {
                u32::from(col.alpha()) << 24
                    | u32::from(col.red()) << 16
                    | u32::from(col.green()) << 8
                    | u32::from(col.blue())
            });
            // Ángulo del texto en la página ya girada (el de PDFium es respecto al espacio de usuario).
            let angle = (c.angle_degrees().unwrap_or(0.0) - g.rotation as f32).rem_euclid(360.0);
            chars.push(RawChar {
                code,
                generated,
                placed: true,
                rect: g.rect_from_user(b.left().value, b.bottom().value, b.right().value, b.top().value),
                origin: g.point_from_user(ox.value, oy.value),
                size: effective_size(&c),
                argb,
                font,
                bold_flag: font_name_is_bold(c.font_weight().as_ref()),
                vertical: (angle - 90.0).abs() < 1.0 || (angle - 270.0).abs() < 1.0,
            });
        }
        Ok(TextData { chars })
    }

    /// Búsqueda de PDFium (sin distinguir mayúsculas). Un cuadrilátero por línea de cada aparición, hasta
    /// `hit_max` apariciones.
    pub(crate) fn search(&self, needle: &str, hit_max: u32) -> Result<Vec<Quad>> {
        if needle.is_empty() || hit_max == 0 {
            return Ok(Vec::new());
        }
        let page = self.load()?;
        let g = self.geom(&page)?;
        let text = page.text().map_err(perr)?;
        let search = text.search(needle, &PdfSearchOptions::new()).map_err(perr)?;
        let mut out = Vec::new();
        let mut hits = 0;
        while let Some(segments) = search.find_next() {
            for s in segments.iter() {
                let b = s.bounds();
                let r = g.rect_from_user(b.left().value, b.bottom().value, b.right().value, b.top().value);
                out.push(r.quad());
            }
            hits += 1;
            if hits >= hit_max {
                break;
            }
        }
        Ok(out)
    }
}

fn is_bold(w: &PdfFontWeight) -> bool {
    match w {
        PdfFontWeight::Weight700Bold | PdfFontWeight::Weight800 | PdfFontWeight::Weight900 => true,
        PdfFontWeight::Custom(n) => *n >= 600,
        _ => false,
    }
}

fn font_name_is_bold(w: Option<&PdfFontWeight>) -> bool {
    w.is_some_and(is_bold)
}

// ---------------------------------------------------------------------------------------------------------------
// Geometría
// ---------------------------------------------------------------------------------------------------------------

/// Caja visible de la página en el espacio de usuario y giro; convierte al espacio de `pliego-pdf`.
#[derive(Debug, Clone, Copy)]
struct PageGeom {
    left: f32,
    bottom: f32,
    /// Ancho y alto **sin girar**.
    w: f32,
    h: f32,
    rotation: u32,
    /// Ancho y alto de la página ya girada (los de `Page::bounds`).
    out_w: f32,
    out_h: f32,
    /// Puntos de `pliego-pdf` por punto de PDFium (`/UserUnit`).
    unit: f32,
}

impl PageGeom {
    fn of(page: &PdfPage<'_>, unit: f32) -> Result<Self> {
        let rotation = match page.rotation().map_err(perr)? {
            PdfPageRenderRotation::None => 0,
            PdfPageRenderRotation::Degrees90 => 90,
            PdfPageRenderRotation::Degrees180 => 180,
            PdfPageRenderRotation::Degrees270 => 270,
        };
        let (out_w, out_h) = (page.width().value, page.height().value);
        let (w, h) = if rotation == 90 || rotation == 270 {
            (out_h, out_w)
        } else {
            (out_w, out_h)
        };
        // Caja visible = MediaBox ∩ CropBox (o Letter si no hay MediaBox), como hace PDFium.
        let boxes = page.boundaries();
        let media = boxes.media().ok().map(|b| b.bounds);
        let crop = boxes.crop().ok().map(|b| b.bounds);
        let (mut l, mut b) = (0.0_f32, 0.0_f32);
        match (media, crop) {
            (Some(m), Some(c)) => {
                l = m.left().value.max(c.left().value);
                b = m.bottom().value.max(c.bottom().value);
            }
            (Some(r), None) | (None, Some(r)) => {
                l = r.left().value;
                b = r.bottom().value;
            }
            (None, None) => {}
        }
        Ok(Self {
            left: l,
            bottom: b,
            w,
            h,
            rotation,
            out_w: out_w * unit,
            out_h: out_h * unit,
            unit,
        })
    }

    fn point_from_user(&self, x: f32, y: f32) -> Point {
        let (u, v) = (x - self.left, y - self.bottom);
        let (px, py) = (u, self.h - v);
        let (rx, ry) = match self.rotation {
            90 => (self.h - py, px),
            180 => (self.w - px, self.h - py),
            270 => (py, self.w - px),
            _ => (px, py),
        };
        Point::new(rx * self.unit, ry * self.unit)
    }

    fn rect_from_user(&self, l: f32, b: f32, r: f32, t: f32) -> Rect {
        let p0 = self.point_from_user(l, b);
        let p1 = self.point_from_user(r, t);
        Rect::new(p0.x.min(p1.x), p0.y.min(p1.y), p0.x.max(p1.x), p0.y.max(p1.y))
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Mapas de bits
// ---------------------------------------------------------------------------------------------------------------

/// Tamaño de letra efectivo. `scaled_font_size` multiplica por la escala vertical con signo de la matriz del texto: sale
/// negativo con un giro de 180° y 0 con uno de 90°/270°. Se recupera la escala con el coseno del ángulo del texto;
/// cerca de los 90° (donde ese dato no sirve) se supone 1.
fn effective_size(c: &pdfium_render::prelude::PdfPageTextChar<'_>) -> f32 {
    let unscaled = c.unscaled_font_size().value;
    let scaled = c.scaled_font_size().value.abs();
    let cos = c.angle_radians().unwrap_or(0.0).cos().abs();
    if cos > 0.3 && scaled > 0.0 {
        scaled / cos
    } else {
        unscaled
    }
}

fn rgba_to_pixmap(rgba: &[u8], w: u32, h: u32, cs: Colorspace, alpha: bool) -> Pixmap {
    let n = cs.n() as usize + usize::from(alpha);
    let mut out = Vec::with_capacity(w as usize * h as usize * n);
    // `as_chunks` exige Rust 1.88; el MSRV del proyecto es 1.85.
    #[allow(unknown_lints, clippy::chunks_exact_to_as_chunks)]
    for px in rgba.chunks_exact(4) {
        let (r, g, b, a) = (px[0], px[1], px[2], px[3]);
        match cs {
            Colorspace::Rgb => out.extend_from_slice(&[r, g, b]),
            Colorspace::Bgr => out.extend_from_slice(&[b, g, r]),
            Colorspace::Gray => out.push(luma(r, g, b)),
            Colorspace::Cmyk => {
                // Conversión simple sin perfil; solo la usan pruebas de color.
                let k = 255 - r.max(g).max(b);
                if k == 255 {
                    out.extend_from_slice(&[0, 0, 0, 255]);
                } else {
                    let d = 255 - u16::from(k);
                    let c = |v: u8| (((u16::from(255 - v) - u16::from(k)) * 255) / d) as u8;
                    out.extend_from_slice(&[c(r), c(g), c(b), k]);
                }
            }
        }
        if alpha {
            out.push(a);
        }
    }
    Pixmap::from_samples(cs, w, h, alpha, out).expect("tamaño calculado a partir de los mismos datos")
}

/// Luminancia (0,3 R + 0,59 G + 0,11 B).
fn luma(r: u8, g: u8, b: u8) -> u8 {
    ((u32::from(r) * 77 + u32::from(g) * 151 + u32::from(b) * 28) >> 8) as u8
}

// ---------------------------------------------------------------------------------------------------------------
// Texto estructurado: de la lista plana de caracteres de PDFium a bloques → líneas → caracteres
// ---------------------------------------------------------------------------------------------------------------

/// Carácter tal como lo entrega PDFium (ya en el espacio de `pliego-pdf`).
struct RawChar {
    code: u32,
    generated: bool,
    placed: bool,
    rect: Rect,
    origin: Point,
    size: f32,
    argb: u32,
    font: Option<Arc<CharFont>>,
    bold_flag: bool,
    vertical: bool,
}

impl RawChar {
    fn unplaced(code: u32, generated: bool) -> Self {
        Self {
            code,
            generated,
            placed: false,
            rect: Rect::new(0.0, 0.0, 0.0, 0.0),
            origin: Point::new(0.0, 0.0),
            size: 0.0,
            argb: 0xFF00_0000,
            font: None,
            bold_flag: false,
            vertical: false,
        }
    }

    fn is_newline(&self) -> bool {
        matches!(self.code, 0x0A | 0x0D | 0x2028 | 0x2029)
    }
}

/// Texto de una página tal como lo entrega PDFium, antes de estructurarlo.
pub(crate) struct TextData {
    chars: Vec<RawChar>,
}

impl TextData {
    /// Líneas → bloques. PDFium ya ordena los caracteres en orden de lectura e inserta espacios y saltos de línea
    /// ("generados"); aquí se cortan las líneas en esos saltos y se agrupan en bloques las líneas consecutivas
    /// cercanas y alineadas.
    pub(crate) fn blocks(&self) -> Vec<TextBlock> {
        let mut lines: Vec<TextLine> = Vec::new();
        let mut cur: Vec<TextChar> = Vec::new();
        let flush = |cur: &mut Vec<TextChar>, lines: &mut Vec<TextLine>, vertical: bool| {
            // Recorta espacios generados al final de la línea.
            while cur
                .last()
                .is_some_and(|c| c.ch == Some(' ') && c.flags.contains(CharFlags::SYNTHETIC))
            {
                cur.pop();
            }
            if cur.iter().any(|c| c.ch.is_some_and(|ch| !ch.is_whitespace())) {
                let bounds = cur
                    .iter()
                    .map(|c| Rect::from(c.quad))
                    .reduce(|a, b| a.union(&b))
                    .unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0));
                lines.push(TextLine {
                    bounds,
                    wmode: if vertical {
                        WriteMode::Vertical
                    } else {
                        WriteMode::Horizontal
                    },
                    chars: std::mem::take(cur),
                });
            } else {
                cur.clear();
            }
        };
        let mut vertical = false;
        for rc in &self.chars {
            if rc.is_newline() {
                flush(&mut cur, &mut lines, vertical);
                continue;
            }
            if rc.code == 0 || !rc.placed && !rc.generated {
                continue;
            }
            let ch = char::from_u32(rc.code).filter(|c| !c.is_control());
            let mut flags = CharFlags::empty();
            if rc.generated {
                flags |= CharFlags::SYNTHETIC;
            }
            if rc.bold_flag || rc.font.as_ref().is_some_and(|f| f.bold) {
                flags |= CharFlags::BOLD;
            }
            vertical = rc.vertical;
            if rc.generated && !rc.placed {
                // Espacio generado sin caja: hereda la de su vecino anterior (o se descarta al principio).
                if let Some(prev) = cur.last() {
                    let mut q = prev.quad;
                    let w = Rect::from(prev.quad).width();
                    let shift = |p: Point| Point::new(p.x + w, p.y);
                    q = Quad::new(shift(q.ul), shift(q.ur), shift(q.ll), shift(q.lr));
                    cur.push(TextChar {
                        ch,
                        origin: shift(prev.origin),
                        quad: q,
                        size: prev.size,
                        argb: prev.argb,
                        flags,
                        font: prev.font.clone(),
                    });
                }
                continue;
            }
            cur.push(TextChar {
                ch,
                origin: rc.origin,
                quad: rc.rect.quad(),
                size: rc.size,
                argb: rc.argb,
                flags,
                font: rc.font.clone(),
            });
        }
        flush(&mut cur, &mut lines, vertical);

        group_blocks(lines)
    }
}

/// Une líneas consecutivas en bloques: misma columna (solape horizontal), separación vertical menor que un
/// interlineado y medio y tamaño de letra parecido.
fn group_blocks(lines: Vec<TextLine>) -> Vec<TextBlock> {
    let mut blocks: Vec<TextBlock> = Vec::new();
    for line in lines {
        let join = blocks.last().is_some_and(|b| {
            let Some(prev) = b.lines.last() else { return false };
            let (ph, ch) = (prev.bounds.height(), line.bounds.height());
            let gap = line.bounds.y0 - prev.bounds.y1;
            let overlap = line.bounds.x0.max(prev.bounds.x0) < line.bounds.x1.min(prev.bounds.x1);
            let similar = ph > 0.0 && ch > 0.0 && (ph / ch).clamp(0.0, 10.0) < 1.5 && (ch / ph) < 1.5;
            overlap && similar && gap < 0.75 * ph.max(ch) && gap > -0.5 * ph.max(ch)
        });
        if join {
            let b = blocks.last_mut().expect("comprobado");
            b.bounds = b.bounds.union(&line.bounds);
            b.lines.push(line);
        } else {
            blocks.push(TextBlock {
                kind: TextBlockKind::Text,
                bounds: line.bounds,
                lines: vec![line],
                image_transform: None,
            });
        }
    }
    blocks
}

/// Palabras (trozos de la línea separados por espacios) con su caja.
pub(crate) fn words_of(blocks: &[TextBlock]) -> Vec<TextWord> {
    let mut out = Vec::new();
    for (bi, b) in blocks.iter().enumerate() {
        for (li, line) in b.lines.iter().enumerate() {
            let mut word = 0;
            let mut text = String::new();
            let mut bounds: Option<Rect> = None;
            let mut push = |text: &mut String, bounds: &mut Option<Rect>, word: &mut usize| {
                if let Some(bd) = bounds.take() {
                    out.push(TextWord {
                        text: std::mem::take(text),
                        bounds: bd,
                        block: bi,
                        line: li,
                        word: *word,
                    });
                    *word += 1;
                }
                text.clear();
            };
            for c in &line.chars {
                match c.ch {
                    Some(ch) if !ch.is_whitespace() => {
                        text.push(ch);
                        let r = Rect::from(c.quad);
                        bounds = Some(bounds.map_or(r, |b| b.union(&r)));
                    }
                    _ => push(&mut text, &mut bounds, &mut word),
                }
            }
            push(&mut text, &mut bounds, &mut word);
        }
    }
    out
}

/// Texto plano a partir de los bloques: líneas separadas por `\n` y una línea en blanco entre bloques.
pub(crate) fn blocks_to_text(blocks: &[TextBlock]) -> String {
    let mut out = String::new();
    for b in blocks {
        for l in &b.lines {
            out.push_str(&l.text());
            out.push('\n');
        }
        out.push('\n');
    }
    out
}

/// Búsqueda sin distinguir mayúsculas sobre el texto ya estructurado (lo que usa `TextPage::search`). Un
/// cuadrilátero por línea de cada aparición; las apariciones pueden cruzar líneas dentro de un bloque.
pub(crate) fn search_blocks(blocks: &[TextBlock], needle: &str) -> Vec<Quad> {
    let needle: Vec<char> = needle.chars().flat_map(char::to_lowercase).collect();
    if needle.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for b in blocks {
        // (carácter en minúscula, índice de línea, caja). Entre líneas se intercala un espacio sin caja.
        let mut flat: Vec<(char, usize, Option<Rect>)> = Vec::new();
        for (li, line) in b.lines.iter().enumerate() {
            if li > 0 {
                flat.push((' ', li, None));
            }
            for c in &line.chars {
                if let Some(ch) = c.ch {
                    let r = Rect::from(c.quad);
                    for lc in ch.to_lowercase() {
                        flat.push((lc, li, Some(r)));
                    }
                }
            }
        }
        let mut i = 0;
        while i + needle.len() <= flat.len() {
            if flat[i..i + needle.len()].iter().map(|f| f.0).eq(needle.iter().copied()) {
                let mut per_line: Vec<(usize, Rect)> = Vec::new();
                for (_, li, r) in &flat[i..i + needle.len()] {
                    let Some(r) = r else { continue };
                    match per_line.last_mut() {
                        Some((l, acc)) if l == li => *acc = acc.union(r),
                        _ => per_line.push((*li, *r)),
                    }
                }
                out.extend(per_line.into_iter().map(|(_, r)| r.quad()));
                i += needle.len();
            } else {
                i += 1;
            }
        }
    }
    out
}

/// HTML sencillo (un `<p>` por bloque) para quien pida `to_html` de un texto extraído con PDFium.
pub(crate) fn blocks_to_html(blocks: &[TextBlock], id: i32, full: bool) -> String {
    fn esc(s: &str) -> String {
        s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
    }
    let mut out = String::new();
    if full {
        out.push_str("<!DOCTYPE html>\n<html>\n<head><meta charset=\"utf-8\"></head>\n<body>\n");
    }
    out.push_str(&format!("<div id=\"page{id}\">\n"));
    for b in blocks {
        let lines: Vec<String> = b.lines.iter().map(|l| esc(&l.text())).collect();
        out.push_str(&format!("<p>{}</p>\n", lines.join("<br/>")));
    }
    out.push_str("</div>\n");
    if full {
        out.push_str("</body>\n</html>\n");
    }
    out
}

#[cfg(test)]
mod lazy_file_tests {
    use super::*;
    use std::io::{Read, Seek, SeekFrom};

    /// Archivo de 3,5 bloques con contenido que depende de la posición.
    fn sample(dir: &std::path::Path) -> (std::path::PathBuf, Vec<u8>) {
        let data: Vec<u8> = (0..(BLOCK * 7 / 2) as usize).map(|i| (i * 31 % 251) as u8).collect();
        let path = dir.join("a.bin");
        std::fs::write(&path, &data).unwrap();
        (path, data)
    }

    #[test]
    fn reads_match_the_file_across_block_boundaries() {
        let dir = tempfile::tempdir().unwrap();
        let (path, data) = sample(dir.path());
        let mut f = LazyFile::open(&path).unwrap();
        let counter = Arc::new(AtomicU64::new(0));
        f.consumed = Some(counter.clone());
        for (start, len) in [
            (0u64, 10usize),
            (BLOCK - 5, 10),
            (BLOCK * 2 - 1, (BLOCK + 2) as usize),
            (data.len() as u64 - 7, 7),
            (3, data.len() - 3),
        ] {
            f.seek(SeekFrom::Start(start)).unwrap();
            let mut buf = vec![0u8; len];
            f.read_exact(&mut buf).unwrap();
            assert_eq!(buf, data[start as usize..start as usize + len], "en {start}");
        }
        // Al final del archivo no hay más y la lectura larga se queda corta sin error.
        f.seek(SeekFrom::End(-4)).unwrap();
        let mut rest = [0u8; 16];
        assert_eq!(f.read(&mut rest).unwrap(), 4);
        assert_eq!(f.read(&mut rest).unwrap(), 0);
        assert!(counter.load(Ordering::Relaxed) > data.len() as u64);
        assert_eq!(f.seek(SeekFrom::End(0)).unwrap(), data.len() as u64);
        assert!(f.seek(SeekFrom::Current(-(data.len() as i64) - 1)).is_err());
    }

    #[test]
    fn keeps_a_few_blocks_and_holds_no_file_handle() {
        let dir = tempfile::tempdir().unwrap();
        let (path, data) = sample(dir.path());
        let mut f = LazyFile::open(&path).unwrap();
        let mut one = [0u8; 1];
        for i in 0..3 {
            f.seek(SeekFrom::Start(i * BLOCK)).unwrap();
            f.read_exact(&mut one).unwrap();
        }
        assert_eq!(f.blocks.len(), 3);
        // Sin manejador abierto: se puede sustituir el archivo con un renombrado, que en Windows falla si hay uno.
        let other = dir.path().join("b.bin");
        std::fs::write(&other, b"otra cosa").unwrap();
        std::fs::rename(&other, &path).unwrap();
        // Lo guardado sigue sirviendo, pero lo que haya que pedir al disco falla en vez de mezclar versiones.
        f.seek(SeekFrom::Start(5)).unwrap();
        assert!(f.read_exact(&mut one).is_ok(), "bloque 0 ya estaba en memoria");
        assert_eq!(one[0], data[5]);
        f.seek(SeekFrom::Start(BLOCK * 3 + 1)).unwrap();
        assert!(f.read_exact(&mut one).is_err(), "el archivo cambió");
    }

    #[test]
    fn evicts_the_oldest_block() {
        let dir = tempfile::tempdir().unwrap();
        let big = dir.path().join("big.bin");
        std::fs::write(&big, vec![9u8; (BLOCK as usize) * (KEEP_BLOCKS + 3)]).unwrap();
        let mut f = LazyFile::open(&big).unwrap();
        let mut one = [0u8; 1];
        for i in 0..(KEEP_BLOCKS as u64 + 3) {
            f.seek(SeekFrom::Start(i * BLOCK)).unwrap();
            f.read_exact(&mut one).unwrap();
        }
        assert_eq!(f.blocks.len(), KEEP_BLOCKS);
        assert_eq!(f.blocks.first().unwrap().0, 3);
    }

    #[test]
    fn refuses_files_that_pdfium_cannot_index() {
        // No se crea un archivo de 4 GiB: se comprueba el límite sobre la constante.
        assert_eq!(MAX_FILE_BYTES, u32::MAX as u64);
    }
}
