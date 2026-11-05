use pliego_core::{Error, Result};
use pliego_pdf::PdfDocument;

use crate::objects::xref_of;
use crate::read::page_widgets;

/// Nombres de los campos de la página en su orden de tabulación actual (el de `/Annots`).
pub fn tab_order(doc: &PdfDocument, page: usize) -> Result<Vec<String>> {
    let n = doc.page_count()?;
    if page as i32 >= n {
        return Err(Error::InvalidInput(format!("la página {} no existe", page + 1)));
    }
    let widgets = page_widgets(doc, page as i32)?;
    let mut out: Vec<String> = Vec::new();
    for (name, _) in widgets {
        if !out.contains(&name) {
            out.push(name);
        }
    }
    Ok(out)
}

/// Fija el orden de tabulación de los campos de una página: los nombres indicados pasan a recorrerse en ese orden
/// (los demás elementos de la página no se mueven). Elimina `/Tabs` para que mande el orden de `/Annots`.
pub fn set_tab_order(doc: &mut PdfDocument, page: usize, order: &[String]) -> Result<()> {
    let n = doc.page_count()?;
    if page as i32 >= n {
        return Err(Error::InvalidInput(format!("la página {} no existe", page + 1)));
    }
    let widgets = page_widgets(doc, page as i32)?;
    // xref de cada widget agrupado por nombre.
    let mut by_name: Vec<(String, Vec<i32>)> = Vec::new();
    for (name, w) in &widgets {
        let x = w.xref()?;
        match by_name.iter_mut().find(|(n, _)| n == name) {
            Some((_, v)) => v.push(x),
            None => by_name.push((name.clone(), vec![x])),
        }
    }
    let mut seq: Vec<i32> = Vec::new();
    for name in order {
        let Some((_, xs)) = by_name.iter().find(|(n, _)| n == name) else {
            return Err(Error::InvalidInput(format!(
                "el campo «{name}» no está en la página {}",
                page + 1
            )));
        };
        if seq.iter().any(|x| xs.contains(x)) {
            return Err(Error::InvalidInput(format!(
                "el campo «{name}» aparece repetido en el orden"
            )));
        }
        seq.extend(xs);
    }
    let mut page_obj = doc.find_page(page as i32)?;
    let mut annots = page_obj
        .get_dict("Annots")?
        .ok_or_else(|| Error::InvalidInput("la página no tiene campos".into()))?;
    let len = annots.len()?;
    let mut slots: Vec<i32> = Vec::new();
    let mut items = Vec::new();
    for i in 0..len {
        let item = annots.get_array(i as i32)?;
        if let Some(it) = &item
            && seq.contains(&xref_of(it))
        {
            slots.push(i as i32);
        }
        items.push(item);
    }
    for (slot, x) in slots.iter().zip(&seq) {
        let item = items
            .iter()
            .flatten()
            .find(|it| xref_of(it) == *x)
            .ok_or_else(|| Error::Pdf("no se encontró el campo en /Annots".into()))?
            .try_clone()?;
        annots.array_put(*slot, item)?;
    }
    page_obj.dict_delete("Tabs")?;
    Ok(())
}
