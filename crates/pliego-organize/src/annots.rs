//! Traspaso de anotaciones al combinar: `insert_pdf` de MuPDF solo copia el contenido de las páginas, así
//! que enlaces, comentarios y campos de formulario se llevan aparte, con un mapa de injerto común para que
//! los campos que comparten padre sigan compartiéndolo.

use std::collections::{HashMap, HashSet};

use mupdf::pdf::{PdfDocument, PdfObject};
use pliego_core::Result;

use crate::pdf_err;

/// Estado del formulario del documento de destino mientras se combinan archivos.
#[derive(Default)]
pub(crate) struct FormState {
    used_names: HashSet<String>,
    fields: Option<PdfObject>,
    renamed: usize,
}

impl FormState {
    /// Campos que se han renombrado para evitar colisiones.
    pub(crate) fn renamed(&self) -> usize {
        self.renamed
    }
}

fn subtype(obj: &PdfObject) -> Result<String> {
    Ok(match obj.get_dict("Subtype").map_err(pdf_err)? {
        Some(s) => String::from_utf8_lossy(&s.as_name().unwrap_or_default()).into_owned(),
        None => String::new(),
    })
}

/// Sustituye en el array de destino la referencia a página por el índice de la página de resultado (entero
/// directo), para que el injerto no arrastre la página de origen entera. `false` si esa página no se copia.
fn detach_dest(arr: &mut PdfObject, page_of: &HashMap<i32, u32>) -> Result<bool> {
    let Some(first) = arr.get_array(0).map_err(pdf_err)? else {
        return Ok(true);
    };
    let Ok(xref) = first.as_indirect() else { return Ok(true) };
    let Some(&idx) = page_of.get(&xref) else {
        return Ok(false);
    };
    arr.array_put(0, PdfObject::new_int(idx as i32).map_err(pdf_err)?)
        .map_err(pdf_err)?;
    Ok(true)
}

/// Prepara un enlace del origen: los destinos a páginas que no se copian se eliminan (el enlace queda inerte).
fn detach_link(annot: &mut PdfObject, page_of: &HashMap<i32, u32>) -> Result<()> {
    if let Some(mut d) = annot.get_dict("Dest").map_err(pdf_err)?
        && d.is_array().map_err(pdf_err)?
        && !detach_dest(&mut d, page_of)?
    {
        annot.dict_delete("Dest").map_err(pdf_err)?;
    }
    if let Some(action) = annot.get_dict("A").map_err(pdf_err)?
        && let Some(mut d) = action.get_dict("D").map_err(pdf_err)?
        && d.is_array().map_err(pdf_err)?
        && !detach_dest(&mut d, page_of)?
    {
        annot.dict_delete("A").map_err(pdf_err)?;
    }
    Ok(())
}

/// Arrays de destino de un enlace: `/Dest` y `/A /D`.
fn link_dests(annot: &PdfObject) -> Result<Vec<PdfObject>> {
    let mut found = Vec::new();
    let mut candidates = vec![annot.get_dict("Dest").map_err(pdf_err)?];
    if let Some(action) = annot.get_dict("A").map_err(pdf_err)? {
        candidates.push(action.get_dict("D").map_err(pdf_err)?);
    }
    for d in candidates.into_iter().flatten() {
        if d.is_array().map_err(pdf_err)? {
            found.push(d);
        }
    }
    Ok(found)
}

/// Tras el injerto, vuelve a poner la página de destino real donde había un índice.
fn attach_dest(dest: &PdfDocument, arr: &mut PdfObject) -> Result<()> {
    let Some(first) = arr.get_array(0).map_err(pdf_err)? else {
        return Ok(());
    };
    if !first.is_int().map_err(pdf_err)? {
        return Ok(());
    }
    let idx = first.as_int().map_err(pdf_err)?.max(0) as u32;
    let page = dest.find_page(idx as i32).map_err(pdf_err)?;
    arr.array_put(0, page).map_err(pdf_err)
}

/// Raíz de la jerarquía de un campo (sigue `/Parent` hasta el final).
fn field_root(widget: &PdfObject) -> Result<PdfObject> {
    let mut cur = widget.try_clone().map_err(pdf_err)?;
    for _ in 0..64 {
        match cur.get_dict("Parent").map_err(pdf_err)? {
            Some(p) if p.is_dict().map_err(pdf_err)? => cur = p,
            _ => break,
        }
    }
    Ok(cur)
}

/// Copia las anotaciones de páginas de `src` a páginas de `dest` ya creadas: `pairs` son parejas
/// `(página de origen, página de destino)`. Si una página de origen aparece varias veces, solo la primera
/// copia conserva las anotaciones.
pub(crate) fn carry_annotations(
    dest: &mut PdfDocument,
    src: &mut PdfDocument,
    pairs: &[(u32, u32)],
    form: &mut FormState,
) -> Result<()> {
    let mut page_of: HashMap<i32, u32> = HashMap::new();
    let mut src_pages = Vec::new();
    for &(from, to) in pairs {
        let page = src.find_page(from as i32).map_err(pdf_err)?;
        if let Ok(x) = page.as_indirect() {
            page_of.entry(x).or_insert(to);
        }
        src_pages.push((page, to));
    }
    let mut map = dest.new_graft_map().map_err(pdf_err)?;
    let mut roots: Vec<PdfObject> = Vec::new();
    let mut root_seen: HashSet<i32> = HashSet::new();
    let mut src_acro: Option<PdfObject> = None;

    let mut done: HashSet<i32> = HashSet::new();
    for (src_page, to) in &src_pages {
        if src_page.as_indirect().is_ok_and(|x| !done.insert(x)) {
            continue;
        }
        let Some(annots) = src_page.get_dict("Annots").map_err(pdf_err)? else {
            continue;
        };
        if !annots.is_array().map_err(pdf_err)? {
            continue;
        }
        let mut copied = dest.new_array().map_err(pdf_err)?;
        for k in 0..annots.len().map_err(pdf_err)? {
            let Some(mut annot) = annots.get_array(k as i32).map_err(pdf_err)? else {
                continue;
            };
            if !annot.is_dict().map_err(pdf_err)? {
                continue;
            }
            let kind = subtype(&annot)?;
            // Se corta el vínculo a la página de origen para que el injerto no la copie entera.
            let _ = annot.dict_delete("P");
            if kind == "Link" {
                detach_link(&mut annot, &page_of)?;
            }
            let g = map.graft_object(&annot).map_err(pdf_err)?;
            if kind == "Link" {
                for mut d in link_dests(&g)? {
                    attach_dest(dest, &mut d)?;
                }
            }
            if kind == "Widget" {
                let root = field_root(&g)?;
                if root.as_indirect().is_ok_and(|x| root_seen.insert(x)) {
                    roots.push(root);
                }
            }
            copied.array_push(g).map_err(pdf_err)?;
        }
        if copied.len().map_err(pdf_err)? > 0 {
            let mut page = dest.find_page(*to as i32).map_err(pdf_err)?;
            page.dict_put("Annots", copied).map_err(pdf_err)?;
        }
    }

    if roots.is_empty() {
        return Ok(());
    }
    if src_acro.is_none() {
        src_acro = src.catalog().map_err(pdf_err)?.get_dict("AcroForm").map_err(pdf_err)?;
    }
    let mut catalog = dest.catalog().map_err(pdf_err)?;
    let mut acro = match catalog.get_dict("AcroForm").map_err(pdf_err)? {
        Some(a) if a.is_dict().map_err(pdf_err)? => a,
        _ => {
            let d = dest.new_dict().map_err(pdf_err)?;
            let mut d = dest.add_object(&d).map_err(pdf_err)?;
            d.dict_put("NeedAppearances", PdfObject::new_bool(true))
                .map_err(pdf_err)?;
            catalog
                .dict_put("AcroForm", d.try_clone().map_err(pdf_err)?)
                .map_err(pdf_err)?;
            d
        }
    };
    if let Some(sa) = &src_acro {
        for key in ["DA", "DR"] {
            if acro.get_dict(key).map_err(pdf_err)?.is_some() {
                continue;
            }
            if let Some(v) = sa.get_dict(key).map_err(pdf_err)? {
                let g = map.graft_object(&v).map_err(pdf_err)?;
                acro.dict_put(key, g).map_err(pdf_err)?;
            }
        }
    }
    if form.fields.is_none() {
        let arr = dest.new_array().map_err(pdf_err)?;
        acro.dict_put("Fields", arr).map_err(pdf_err)?;
        form.fields = acro.get_dict("Fields").map_err(pdf_err)?;
    }
    let fields = form.fields.as_mut().expect("creado arriba");
    for mut root in roots {
        let original = match root.get_dict("T").map_err(pdf_err)? {
            Some(t) => t.as_string().unwrap_or_default(),
            None => String::new(),
        };
        let mut candidate = original.clone();
        let mut n = 1;
        while !form.used_names.insert(candidate.clone()) {
            n += 1;
            candidate = format!("{original}_{n}");
        }
        if candidate != original {
            root.dict_put("T", PdfObject::new_string(&candidate).map_err(pdf_err)?)
                .map_err(pdf_err)?;
            form.renamed += 1;
        }
        fields.array_push(root).map_err(pdf_err)?;
    }
    Ok(())
}
