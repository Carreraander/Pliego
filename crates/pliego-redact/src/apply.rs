use std::collections::BTreeMap;
use std::path::Path;

use mupdf::pdf::{
    PdfDocument, PdfRedactImageMethod, PdfRedactLineArtMethod, PdfRedactOptions, PdfRedactTextMethod, PdfWriteOptions,
};
use mupdf::{Rect, TextPageFlags};
use pliego_core::{Error, JobContext, Report, Result};
use serde::{Deserialize, Serialize};

use crate::patterns::{Area, Pattern, find_patterns, pdf_err};

/// Relleno de las zonas redactadas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Fill {
    /// Recuadro negro.
    #[default]
    Black,
    /// Sin recuadro: la zona queda en blanco.
    None,
}

/// Qué hacer con las imágenes que tocan una zona redactada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ImageMode {
    /// Borra solo los píxeles cubiertos.
    #[default]
    Pixels,
    /// Elimina la imagen entera.
    Remove,
    /// No toca las imágenes (el texto y los vectores sí se eliminan).
    Keep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedactOptions {
    pub fill: Fill,
    pub images: ImageMode,
    /// Elimina también los vectores (líneas, rellenos) cubiertos por la zona.
    pub remove_line_art: bool,
}

impl Default for RedactOptions {
    fn default() -> Self {
        Self {
            fill: Fill::Black,
            images: ImageMode::Pixels,
            remove_line_art: true,
        }
    }
}

/// Qué redactar: zonas marcadas a mano y/o patrones.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RedactJob {
    pub areas: Vec<Area>,
    pub patterns: Vec<Pattern>,
    pub options: RedactOptions,
}

/// Texto que se pudo extraer tras redactar (no debería haber ninguno).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Leftover {
    /// Página desde 0.
    pub page: u32,
    pub needle: String,
}

/// Añade marcas de redacción (sin aplicarlas). Devuelve cuántas.
pub fn mark(doc: &mut PdfDocument, areas: &[Area]) -> Result<usize> {
    let pages = doc.page_count().map_err(pdf_err)?.max(0) as u32;
    let mut by_page: BTreeMap<u32, Vec<&Area>> = BTreeMap::new();
    for a in areas {
        if a.page >= pages {
            return Err(Error::InvalidInput(format!(
                "la página {} no existe (el documento tiene {pages})",
                a.page + 1
            )));
        }
        if !(a.x1 > a.x0 && a.y1 > a.y0) {
            return Err(Error::InvalidInput(
                "una zona de redacción tiene tamaño cero o negativo".into(),
            ));
        }
        by_page.entry(a.page).or_default().push(a);
    }
    let mut n = 0;
    for (p, list) in by_page {
        let mut page = doc.load_pdf_page(p as i32).map_err(pdf_err)?;
        for a in list {
            page.add_redact_annotation(Rect {
                x0: a.x0,
                y0: a.y0,
                x1: a.x1,
                y1: a.y1,
            })
            .map_err(pdf_err)?;
            n += 1;
        }
        page.update().map_err(pdf_err)?;
    }
    Ok(n)
}

/// Aplica todas las marcas de redacción del documento: elimina el texto, las imágenes y los
/// vectores bajo ellas y quita las marcas. Devuelve en cuántas páginas hubo cambios.
pub fn apply_document(doc: &mut PdfDocument, options: &RedactOptions, ctx: &JobContext<'_>) -> Result<usize> {
    let raw = PdfRedactOptions {
        black_boxes: options.fill == Fill::Black,
        image_method: match options.images {
            ImageMode::Pixels => PdfRedactImageMethod::Pixels,
            ImageMode::Remove => PdfRedactImageMethod::Remove,
            ImageMode::Keep => PdfRedactImageMethod::None,
        },
        line_art: if options.remove_line_art {
            PdfRedactLineArtMethod::RemoveIfCovered
        } else {
            PdfRedactLineArtMethod::None
        },
        text: PdfRedactTextMethod::Remove,
    };
    let pages = doc.page_count().map_err(pdf_err)?.max(0);
    let mut changed = 0;
    for p in 0..pages {
        ctx.step(p as u64, pages as u64, "Aplicando redacciones")?;
        let mut page = doc.load_pdf_page(p).map_err(pdf_err)?;
        if page.redact_with_options(raw).map_err(pdf_err)? {
            changed += 1;
        }
    }
    ctx.step(pages as u64, pages as u64, "Redacción aplicada")?;
    Ok(changed)
}

fn squash(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

/// Comprueba que ninguna de las cadenas se pueda extraer del texto ni encontrar con la búsqueda.
pub fn verify(doc: &PdfDocument, needles: &[String]) -> Result<Vec<Leftover>> {
    let pages = doc.page_count().map_err(pdf_err)?.max(0);
    let mut left = Vec::new();
    for p in 0..pages {
        let page = doc.load_page(p).map_err(pdf_err)?;
        let text = page
            .to_text_page(TextPageFlags::empty())
            .map_err(pdf_err)?
            .to_text()
            .map_err(pdf_err)?;
        let squashed = squash(&text);
        for n in needles {
            let sn = squash(n);
            if sn.is_empty() {
                continue;
            }
            let found = squashed.contains(&sn) || !page.search(n, 1).map_err(pdf_err)?.is_empty();
            if found {
                left.push(Leftover {
                    page: p as u32,
                    needle: n.clone(),
                });
            }
        }
    }
    Ok(left)
}

/// Inset vertical de las zonas de coincidencia para no tocar líneas vecinas.
fn shrink(a: &Area) -> Area {
    let dy = (a.y1 - a.y0) * 0.15;
    Area {
        y0: a.y0 + dy,
        y1: a.y1 - dy,
        ..*a
    }
}

/// Redacta `input` según `job` y escribe `output`. Antes de escribir, abre el resultado y comprueba
/// que el texto de las coincidencias ya no se puede extraer ni buscar; si queda algo, devuelve error
/// y no deja ningún archivo.
pub fn apply(
    input: &Path,
    output: &Path,
    password: Option<&str>,
    job: &RedactJob,
    ctx: &JobContext<'_>,
) -> Result<Report> {
    let mut doc = open_pdf(input, password)?;
    let matches = find_patterns(&doc, &job.patterns)?;
    let mut areas: Vec<Area> = job.areas.clone();
    areas.extend(matches.iter().flat_map(|m| m.areas.iter().map(shrink)));
    if areas.is_empty() {
        return Ok(Report {
            summary: "No se encontró nada que redactar; no se ha escrito ningún archivo.".into(),
            ..Default::default()
        });
    }
    let marked = mark(&mut doc, &areas)?;
    apply_document(&mut doc, &job.options, ctx)?;

    let same = match (std::fs::canonicalize(input), std::fs::canonicalize(output)) {
        (Ok(a), Ok(b)) => a == b,
        _ => input == output,
    };
    if same {
        return Err(Error::InvalidInput(
            "el archivo de salida es el mismo que el de entrada; elige otro nombre para no perder el original".into(),
        ));
    }
    let dir = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let tmp = tempfile::Builder::new()
        .prefix(".pliego-")
        .suffix(".tmp")
        .tempfile_in(dir)
        .map_err(|e| Error::io_at(dir, e))?;
    let tmp_str = tmp
        .path()
        .to_str()
        .ok_or_else(|| Error::InvalidInput("la ruta de salida debe ser UTF-8".into()))?;
    let mut write = PdfWriteOptions::default();
    // Nivel 3: descarta los objetos que ya no se usan (los contenidos y las imágenes originales).
    write.set_garbage(true).set_garbage_level(3);
    doc.save_with_options(tmp_str, write).map_err(pdf_err)?;

    let needles: Vec<String> = matches.iter().map(|m| m.text.clone()).collect();
    // El documento reabierto se cierra antes de renombrar: en Windows no se puede mover un archivo abierto.
    let left = {
        let reopened = open_pdf(tmp.path(), None)?;
        verify(&reopened, &needles)?
    };
    if !left.is_empty() {
        return Err(Error::Other(format!(
            "la verificación falló: {} cadena(s) redactada(s) todavía se pueden extraer (p. ej. «{}» en la página {}); no se ha escrito el archivo",
            left.len(),
            left[0].needle,
            left[0].page + 1
        )));
    }
    tmp.persist(output).map_err(|e| Error::io_at(output, e.error))?;

    let mut report = Report::with_output(
        output,
        format!(
            "{marked} zona(s) redactada(s) ({} coincidencia(s) de patrones); verificado: el texto ya no se puede extraer.",
            matches.len()
        ),
    );
    if matches.is_empty() && job.patterns.is_empty() {
        report
            .warnings
            .push("Solo se redactaron las zonas indicadas; el texto fuera de ellas no se ha comprobado.".into());
    }
    Ok(report)
}

fn open_pdf(path: &Path, password: Option<&str>) -> Result<PdfDocument> {
    if !path.exists() {
        return Err(Error::io_at(path, std::io::Error::from(std::io::ErrorKind::NotFound)));
    }
    let s = path
        .to_str()
        .ok_or_else(|| Error::InvalidInput("la ruta del archivo debe ser UTF-8".into()))?;
    let mut doc = PdfDocument::open(s).map_err(pdf_err)?;
    if doc.needs_password().map_err(pdf_err)? {
        match password {
            Some(p) if doc.authenticate(p).map_err(pdf_err)? => {}
            _ => return Err(Error::Password),
        }
    }
    Ok(doc)
}
