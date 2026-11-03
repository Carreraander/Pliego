use mupdf::Rect;
use mupdf::pdf::{InsertPdfOptions, InsertPosition, PageSelection, PdfDocument};
use pliego_core::{Error, Result};
use serde::{Deserialize, Serialize};

use crate::outline::{read_outline, retarget, write_outline};
use crate::pdf_err;

pub fn page_count(doc: &PdfDocument) -> Result<u32> {
    Ok(doc.page_count().map_err(pdf_err)?.max(0) as u32)
}

fn check(doc: &PdfDocument, pages: &[u32]) -> Result<u32> {
    let n = page_count(doc)?;
    if let Some(&bad) = pages.iter().find(|&&p| p >= n) {
        return Err(Error::InvalidInput(format!(
            "la página {} no existe (el documento tiene {n})",
            bad + 1
        )));
    }
    Ok(n)
}

fn indices(pages: &[u32]) -> Vec<usize> {
    pages.iter().map(|&p| p as usize).collect()
}

/// Crea un documento nuevo con las páginas indicadas, en ese orden (se pueden repetir).
/// Conserva los marcadores que apuntan a esas páginas.
pub fn extract(doc: &PdfDocument, pages: &[u32]) -> Result<PdfDocument> {
    check(doc, pages)?;
    if pages.is_empty() {
        return Err(Error::InvalidInput("no hay páginas que extraer".into()));
    }
    let mut out = PdfDocument::new();
    out.insert_pdf(
        doc,
        InsertPdfOptions {
            source_pages: PageSelection::Pages(indices(pages)),
            ..Default::default()
        },
    )
    .map_err(pdf_err)?;
    let outline = read_outline(doc)?;
    let map = |old: u32| pages.iter().position(|&p| p == old).map(|i| i as u32);
    write_outline(&mut out, &retarget(&outline, &map))?;
    Ok(out)
}

/// Inserta antes de la página `at` (0..=n) las páginas `src_pages` de otro documento.
pub fn insert_from(doc: &mut PdfDocument, src: &PdfDocument, src_pages: &[u32], at: u32) -> Result<()> {
    let n = page_count(doc)?;
    if at > n {
        return Err(Error::InvalidInput(format!(
            "la posición {at} queda fuera del documento ({n} páginas)"
        )));
    }
    check(src, src_pages)?;
    if src_pages.is_empty() {
        return Ok(());
    }
    doc.insert_pdf(
        src,
        InsertPdfOptions {
            source_pages: PageSelection::Pages(indices(src_pages)),
            target: if at == n {
                InsertPosition::Append
            } else {
                InsertPosition::Before(at as usize)
            },
            ..Default::default()
        },
    )
    .map_err(pdf_err)?;
    Ok(())
}

/// Inserta `n` páginas en blanco antes de `at`. Sin tamaño, copian el de la página vecina (o A4).
pub fn insert_blank(doc: &mut PdfDocument, at: u32, n: u32, size: Option<(f32, f32)>) -> Result<()> {
    let total = page_count(doc)?;
    if at > total {
        return Err(Error::InvalidInput(format!(
            "la posición {at} queda fuera del documento ({total} páginas)"
        )));
    }
    let size = match size {
        Some(s) => s,
        None if total == 0 => (595.0, 842.0),
        None => {
            let neighbour = if at > 0 { at - 1 } else { 0 };
            let b = doc
                .load_page(neighbour as i32)
                .map_err(pdf_err)?
                .bounds()
                .map_err(pdf_err)?;
            (b.width(), b.height())
        }
    };
    for i in 0..n {
        doc.new_page_at((at + i) as i32, size).map_err(pdf_err)?;
    }
    Ok(())
}

/// Borra páginas. No permite dejar el documento sin páginas.
pub fn delete(doc: &mut PdfDocument, pages: &[u32]) -> Result<()> {
    let n = check(doc, pages)?;
    let mut unique = pages.to_vec();
    unique.sort_unstable();
    unique.dedup();
    if unique.is_empty() {
        return Ok(());
    }
    if unique.len() as u32 >= n {
        return Err(Error::InvalidInput(
            "no se pueden borrar todas las páginas del documento".into(),
        ));
    }
    doc.delete_pages(PageSelection::Pages(indices(&unique)))
        .map_err(pdf_err)
}

/// Gira las páginas `degrees` (múltiplo de 90, positivo = sentido horario) respecto a su giro actual.
pub fn rotate(doc: &mut PdfDocument, pages: &[u32], degrees: i32) -> Result<()> {
    check(doc, pages)?;
    if degrees % 90 != 0 {
        return Err(Error::InvalidInput("el giro debe ser múltiplo de 90 grados".into()));
    }
    for &p in pages {
        let mut page = doc.load_pdf_page(p as i32).map_err(pdf_err)?;
        let now = page.rotation().map_err(pdf_err)?;
        page.set_rotation((now + degrees).rem_euclid(360)).map_err(pdf_err)?;
    }
    Ok(())
}

/// Mueve una página a otra posición (la página queda en el índice `to`).
pub fn move_page(doc: &mut PdfDocument, from: u32, to: u32) -> Result<()> {
    check(doc, &[from, to])?;
    doc.move_page(from as usize, to as usize).map_err(pdf_err)
}

/// Reordena según `new_order`: la posición `i` del resultado es la página `new_order[i]` actual.
/// Debe ser una permutación de todas las páginas. Los marcadores siguen apuntando a su página.
pub fn reorder(doc: &mut PdfDocument, new_order: &[u32]) -> Result<()> {
    let n = page_count(doc)?;
    let mut seen = vec![false; n as usize];
    for &p in new_order {
        if p >= n || std::mem::replace(&mut seen[p as usize], true) {
            return Err(Error::InvalidInput(
                "el nuevo orden debe contener cada página exactamente una vez".into(),
            ));
        }
    }
    if new_order.len() != n as usize {
        return Err(Error::InvalidInput(format!(
            "el nuevo orden tiene {} páginas y el documento {n}",
            new_order.len()
        )));
    }
    let mut current: Vec<u32> = (0..n).collect();
    for (target, &page) in new_order.iter().enumerate() {
        let from = current.iter().position(|&p| p == page).expect("permutación validada");
        if from != target {
            doc.move_page(from, target).map_err(pdf_err)?;
            let moved = current.remove(from);
            current.insert(target, moved);
        }
    }
    Ok(())
}

/// Duplica cada página indicada justo después de sí misma.
pub fn duplicate(doc: &mut PdfDocument, pages: &[u32]) -> Result<()> {
    check(doc, pages)?;
    let mut sorted = pages.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    for &p in sorted.iter().rev() {
        doc.duplicate_page(p as usize).map_err(pdf_err)?;
    }
    Ok(())
}

/// Sustituye `len` páginas desde `start` por `src_pages` de otro documento.
pub fn replace(doc: &mut PdfDocument, start: u32, len: u32, src: &PdfDocument, src_pages: &[u32]) -> Result<()> {
    let n = page_count(doc)?;
    if len == 0 || start.saturating_add(len) > n {
        return Err(Error::InvalidInput("las páginas a reemplazar no existen".into()));
    }
    if src_pages.is_empty() {
        return Err(Error::InvalidInput("no hay páginas de reemplazo".into()));
    }
    check(src, src_pages)?;
    // Insertar primero evita que el documento quede vacío si se reemplazan todas las páginas.
    insert_from(doc, src, src_pages, start + len)?;
    let old: Vec<u32> = (start..start + len).collect();
    delete(doc, &old)
}

/// CropBox (o MediaBox) en el espacio de usuario sin girar. `PdfPage::crop_box` mezcla las dimensiones
/// visuales con las del espacio de usuario en páginas giradas, así que se lee del diccionario de la página.
fn user_box(page: &mupdf::pdf::PdfPage) -> Result<Rect> {
    let obj = page.object();
    for key in ["CropBox", "MediaBox"] {
        let Some(arr) = obj.get_dict_inheritable(key).map_err(pdf_err)? else {
            continue;
        };
        if arr.len().map_err(pdf_err)? < 4 {
            continue;
        }
        let mut v = [0.0f32; 4];
        let mut ok = true;
        for (i, slot) in v.iter_mut().enumerate() {
            match arr.get_array(i as i32).map_err(pdf_err)?.map(|n| n.as_float()) {
                Some(Ok(f)) => *slot = f,
                _ => ok = false,
            }
        }
        if ok {
            return Ok(Rect::new(
                v[0].min(v[2]),
                v[1].min(v[3]),
                v[0].max(v[2]),
                v[1].max(v[3]),
            ));
        }
    }
    Err(Error::Pdf("la página no tiene MediaBox".into()))
}

/// Recorte de página (CropBox), en puntos sobre la página sin girar.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum CropSpec {
    /// Rectángulo absoluto `x0, y0, x1, y1` (origen abajo a la izquierda).
    Rect { x0: f32, y0: f32, x1: f32, y1: f32 },
    /// Quita estos márgenes del recorte actual.
    Margins {
        left: f32,
        top: f32,
        right: f32,
        bottom: f32,
    },
}

/// Aplica el recorte a las páginas indicadas.
pub fn crop(doc: &mut PdfDocument, pages: &[u32], spec: CropSpec) -> Result<()> {
    check(doc, pages)?;
    for &p in pages {
        let mut page = doc.load_pdf_page(p as i32).map_err(pdf_err)?;
        let rect = match spec {
            CropSpec::Rect { x0, y0, x1, y1 } => Rect::new(x0, y0, x1, y1),
            CropSpec::Margins {
                left,
                top,
                right,
                bottom,
            } => {
                let b = user_box(&page)?;
                Rect::new(b.x0 + left, b.y0 + bottom, b.x1 - right, b.y1 - top)
            }
        };
        if !(rect.x1 > rect.x0 && rect.y1 > rect.y0) {
            return Err(Error::InvalidInput(format!(
                "el recorte de la página {} queda vacío",
                p + 1
            )));
        }
        page.set_crop_box(rect).map_err(pdf_err)?;
    }
    Ok(())
}
