use std::collections::BTreeMap;
use std::path::Path;

use pliego_core::{Error, JobContext, Report, Result};
use pliego_pdf::{
    PdfDocument, Rect, RedactImageMethod, RedactLineArtMethod, RedactOptions as PdfRedactOptions, RedactTextMethod,
    TextFlags, WriteOptions,
};
use serde::{Deserialize, Serialize};

use crate::patterns::{Area, Pattern, find_patterns};

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
    let pages = doc.page_count()?.max(0) as u32;
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
        let mut page = doc.load_pdf_page(p as i32)?;
        for a in list {
            page.add_redact_annotation(Rect {
                x0: a.x0,
                y0: a.y0,
                x1: a.x1,
                y1: a.y1,
            })?;
            n += 1;
        }
        page.update()?;
    }
    Ok(n)
}

/// Aplica todas las marcas de redacción del documento: elimina el texto, las imágenes y los
/// vectores bajo ellas y quita las marcas. Devuelve en cuántas páginas hubo cambios.
pub fn apply_document(doc: &mut PdfDocument, options: &RedactOptions, ctx: &JobContext<'_>) -> Result<usize> {
    let raw = PdfRedactOptions {
        black_boxes: options.fill == Fill::Black,
        image_method: match options.images {
            ImageMode::Pixels => RedactImageMethod::Pixels,
            ImageMode::Remove => RedactImageMethod::Remove,
            ImageMode::Keep => RedactImageMethod::None,
        },
        line_art: if options.remove_line_art {
            RedactLineArtMethod::RemoveIfCovered
        } else {
            RedactLineArtMethod::None
        },
        text: RedactTextMethod::Remove,
    };
    let pages = doc.page_count()?.max(0);
    let mut changed = 0;
    for p in 0..pages {
        ctx.step(p as u64, pages as u64, "Aplicando redacciones")?;
        let mut page = doc.load_pdf_page(p)?;
        if page.redact(raw)? {
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
    let pages = doc.page_count()?.max(0);
    let mut left = Vec::new();
    for p in 0..pages {
        let page = doc.load_page(p)?;
        let text = page.to_text_page(TextFlags::empty())?.to_text()?;
        let squashed = squash(&text);
        for n in needles {
            let sn = squash(n);
            if sn.is_empty() {
                continue;
            }
            let found = squashed.contains(&sn) || !page.search(n, 1)?.is_empty();
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
    let mut write = WriteOptions::default();
    // Nivel 3: descarta los objetos que ya no se usan (los contenidos y las imágenes originales).
    write.set_garbage(true).set_garbage_level(3);
    doc.save_with_options(tmp_str, &write)?;

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

/// Busca `patterns` en el archivo `path` (vista previa de coincidencias, sin modificar nada).
pub fn find_in_file(path: &Path, password: Option<&str>, patterns: &[Pattern]) -> Result<Vec<crate::patterns::Match>> {
    let doc = open_pdf(path, password)?;
    find_patterns(&doc, patterns)
}

fn open_pdf(path: &Path, password: Option<&str>) -> Result<PdfDocument> {
    if !path.exists() {
        return Err(Error::io_at(path, std::io::Error::from(std::io::ErrorKind::NotFound)));
    }
    let mut doc = PdfDocument::open(path)?;
    if doc.needs_password()? {
        match password {
            Some(p) if doc.authenticate(p)? => {}
            _ => return Err(Error::Password),
        }
    }
    Ok(doc)
}
