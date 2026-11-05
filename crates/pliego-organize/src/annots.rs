//! Traspaso de anotaciones al combinar: enlaces, comentarios y campos de formulario se llevan aparte, con un
//! mapa de injerto común para que los campos que comparten padre sigan compartiéndolo y con nombres únicos.
//! `insert_pdf` del modelo propio ya copia anotaciones y registra sus campos en `/AcroForm /Fields`; aquí se
//! deshace ese registro (véase [`fields_len`] y [`trim_fields`]) para que lo rehaga este módulo con renombrado.

use std::collections::{HashMap, HashSet};

use pliego_core::Result;
use pliego_pdf::{Object, PdfDocument};

/// Estado del formulario del documento de destino mientras se combinan archivos.
#[derive(Default)]
pub(crate) struct FormState {
    used_names: HashSet<String>,
    fields: Option<Object>,
    renamed: usize,
}

impl FormState {
    /// Campos que se han renombrado para evitar colisiones.
    pub(crate) fn renamed(&self) -> usize {
        self.renamed
    }
}

/// Número de campos raíz que tiene registrados el destino (0 si no hay formulario).
pub(crate) fn fields_len(dest: &PdfDocument) -> Result<usize> {
    let Some(acro) = dest.catalog()?.get_dict("AcroForm")? else {
        return Ok(0);
    };
    match acro.get_dict("Fields")? {
        Some(f) if f.is_array()? => Ok(f.len()?),
        _ => Ok(0),
    }
}

/// Quita de `/AcroForm /Fields` los campos añadidos desde que había `keep`.
pub(crate) fn trim_fields(dest: &PdfDocument, keep: usize) -> Result<()> {
    let Some(acro) = dest.catalog()?.get_dict("AcroForm")? else {
        return Ok(());
    };
    let Some(mut fields) = acro.get_dict("Fields")? else {
        return Ok(());
    };
    if !fields.is_array()? {
        return Ok(());
    }
    for i in (keep..fields.len()?).rev() {
        fields.array_delete(i as i32)?;
    }
    Ok(())
}

fn subtype(obj: &Object) -> Result<String> {
    Ok(match obj.get_dict("Subtype")? {
        Some(s) => String::from_utf8_lossy(&s.as_name().unwrap_or_default()).into_owned(),
        None => String::new(),
    })
}

/// Sustituye en el array de destino la referencia a página por el índice de la página de resultado (entero
/// directo), para que el injerto no arrastre la página de origen entera. `false` si esa página no se copia.
fn detach_dest(arr: &mut Object, page_of: &HashMap<i32, u32>) -> Result<bool> {
    let Some(first) = arr.get_array(0)? else {
        return Ok(true);
    };
    let Ok(xref) = first.as_indirect() else { return Ok(true) };
    let Some(&idx) = page_of.get(&xref) else {
        return Ok(false);
    };
    arr.array_put(0, Object::new_int(idx as i32)?)?;
    Ok(true)
}

/// Prepara un enlace del origen: los destinos a páginas que no se copian se eliminan (el enlace queda inerte).
fn detach_link(annot: &mut Object, page_of: &HashMap<i32, u32>) -> Result<()> {
    if let Some(mut d) = annot.get_dict("Dest")?
        && d.is_array()?
        && !detach_dest(&mut d, page_of)?
    {
        annot.dict_delete("Dest")?;
    }
    if let Some(action) = annot.get_dict("A")?
        && let Some(mut d) = action.get_dict("D")?
        && d.is_array()?
        && !detach_dest(&mut d, page_of)?
    {
        annot.dict_delete("A")?;
    }
    Ok(())
}

/// Arrays de destino de un enlace: `/Dest` y `/A /D`.
fn link_dests(annot: &Object) -> Result<Vec<Object>> {
    let mut found = Vec::new();
    let mut candidates = vec![annot.get_dict("Dest")?];
    if let Some(action) = annot.get_dict("A")? {
        candidates.push(action.get_dict("D")?);
    }
    for d in candidates.into_iter().flatten() {
        if d.is_array()? {
            found.push(d);
        }
    }
    Ok(found)
}

/// Tras el injerto, vuelve a poner la página de destino real donde había un índice.
fn attach_dest(dest: &PdfDocument, arr: &mut Object) -> Result<()> {
    let Some(first) = arr.get_array(0)? else {
        return Ok(());
    };
    if !first.is_int()? {
        return Ok(());
    }
    let idx = first.as_int()?.max(0) as u32;
    let page = dest.find_page(idx as i32)?;
    arr.array_put(0, page)
}

/// Raíz de la jerarquía de un campo (sigue `/Parent` hasta el final).
fn field_root(widget: &Object) -> Result<Object> {
    let mut cur = widget.try_clone()?;
    for _ in 0..64 {
        match cur.get_dict("Parent")? {
            Some(p) if p.is_dict()? => cur = p,
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
        let page = src.find_page(from as i32)?;
        if let Ok(x) = page.as_indirect() {
            page_of.entry(x).or_insert(to);
        }
        src_pages.push((page, to));
    }
    let mut map = dest.new_graft_map()?;
    let mut roots: Vec<Object> = Vec::new();
    let mut root_seen: HashSet<i32> = HashSet::new();
    let mut src_acro: Option<Object> = None;

    let mut done: HashSet<i32> = HashSet::new();
    for (src_page, to) in &src_pages {
        if src_page.as_indirect().is_ok_and(|x| !done.insert(x)) {
            continue;
        }
        let Some(annots) = src_page.get_dict("Annots")? else {
            continue;
        };
        if !annots.is_array()? {
            continue;
        }
        let mut copied = dest.new_array()?;
        for k in 0..annots.len()? {
            let Some(mut annot) = annots.get_array(k as i32)? else {
                continue;
            };
            if !annot.is_dict()? {
                continue;
            }
            let kind = subtype(&annot)?;
            // Se corta el vínculo a la página de origen para que el injerto no la copie entera.
            let _ = annot.dict_delete("P");
            if kind == "Link" {
                detach_link(&mut annot, &page_of)?;
            }
            let g = map.graft_object(&annot)?;
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
            copied.array_push(g)?;
        }
        if copied.len()? > 0 {
            let mut page = dest.find_page(*to as i32)?;
            page.dict_put("Annots", copied)?;
        }
    }

    if roots.is_empty() {
        return Ok(());
    }
    if src_acro.is_none() {
        src_acro = src.catalog()?.get_dict("AcroForm")?;
    }
    let mut catalog = dest.catalog()?;
    let mut acro = match catalog.get_dict("AcroForm")? {
        Some(a) if a.is_dict()? => a,
        _ => {
            let d = dest.new_dict()?;
            let mut d = dest.add_object(&d)?;
            d.dict_put("NeedAppearances", Object::new_bool(true))?;
            catalog.dict_put("AcroForm", d.try_clone()?)?;
            d
        }
    };
    if let Some(sa) = &src_acro {
        for key in ["DA", "DR"] {
            if acro.get_dict(key)?.is_some() {
                continue;
            }
            if let Some(v) = sa.get_dict(key)? {
                let g = map.graft_object(&v)?;
                acro.dict_put(key, g)?;
            }
        }
    }
    if form.fields.is_none() {
        let arr = dest.new_array()?;
        acro.dict_put("Fields", arr)?;
        form.fields = acro.get_dict("Fields")?;
    }
    let fields = form.fields.as_mut().expect("creado arriba");
    for mut root in roots {
        let original = match root.get_dict("T")? {
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
            root.dict_put("T", Object::new_string(&candidate)?)?;
            form.renamed += 1;
        }
        fields.array_push(root)?;
    }
    Ok(())
}
