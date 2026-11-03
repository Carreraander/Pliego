use mupdf::pdf::PdfDocument;
use pliego_core::{Error, Result};
use serde::{Deserialize, Serialize};

use crate::pdf_err;

/// Marcador (índice) con su página de destino desde 0.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutlineNode {
    pub title: String,
    pub page: Option<u32>,
    pub children: Vec<OutlineNode>,
}

/// Lee los marcadores de un documento.
pub fn read_outline(doc: &PdfDocument) -> Result<Vec<OutlineNode>> {
    fn conv(items: Vec<mupdf::Outline>) -> Vec<OutlineNode> {
        items
            .into_iter()
            .map(|o| OutlineNode {
                title: o.title,
                page: o.dest.map(|d| d.loc.page_number),
                children: conv(o.down),
            })
            .collect()
    }
    Ok(conv(doc.outlines().map_err(pdf_err)?))
}

/// Sustituye los marcadores del documento. Los nodos con página fuera de rango se escriben sin destino.
pub fn write_outline(doc: &mut PdfDocument, nodes: &[OutlineNode]) -> Result<()> {
    let mut catalog = doc.catalog().map_err(pdf_err)?;
    if nodes.is_empty() {
        let _ = catalog.dict_delete("Outlines");
        return Ok(());
    }
    let page_count = doc.page_count().map_err(pdf_err)?.max(0) as u32;
    let root = doc.new_dict().map_err(pdf_err)?;
    let mut root = doc.add_object(&root).map_err(pdf_err)?;
    root.dict_put("Type", mupdf::pdf::PdfObject::new_name("Outlines").map_err(pdf_err)?)
        .map_err(pdf_err)?;
    let total = build_level(doc, nodes, &root, page_count)?;
    root.dict_put("Count", mupdf::pdf::PdfObject::new_int(total).map_err(pdf_err)?)
        .map_err(pdf_err)?;
    catalog.dict_put("Outlines", root).map_err(pdf_err)?;
    Ok(())
}

/// Crea los hijos de `parent`; devuelve cuántos elementos visibles cuelgan de él.
fn build_level(
    doc: &mut PdfDocument,
    nodes: &[OutlineNode],
    parent: &mupdf::pdf::PdfObject,
    page_count: u32,
) -> Result<i32> {
    use mupdf::pdf::PdfObject;
    let mut refs: Vec<PdfObject> = Vec::new();
    let mut count = 0;
    for node in nodes {
        let dict = doc.new_dict().map_err(pdf_err)?;
        let mut item = doc.add_object(&dict).map_err(pdf_err)?;
        item.dict_put("Title", PdfObject::new_string(&node.title).map_err(pdf_err)?)
            .map_err(pdf_err)?;
        item.dict_put("Parent", parent.try_clone().map_err(pdf_err)?)
            .map_err(pdf_err)?;
        if let Some(p) = node.page.filter(|&p| p < page_count) {
            let page = doc.find_page(p as i32).map_err(pdf_err)?;
            let mut dest = doc.new_array().map_err(pdf_err)?;
            dest.array_push(page).map_err(pdf_err)?;
            dest.array_push(PdfObject::new_name("Fit").map_err(pdf_err)?)
                .map_err(pdf_err)?;
            item.dict_put("Dest", dest).map_err(pdf_err)?;
        }
        let below = build_level(doc, &node.children, &item, page_count)?;
        if below > 0 {
            item.dict_put("Count", PdfObject::new_int(below).map_err(pdf_err)?)
                .map_err(pdf_err)?;
        }
        count += 1 + below;
        refs.push(item);
    }
    for (i, item) in refs.iter().enumerate() {
        let mut item = item.try_clone().map_err(pdf_err)?;
        if i > 0 {
            item.dict_put("Prev", refs[i - 1].try_clone().map_err(pdf_err)?)
                .map_err(pdf_err)?;
        }
        if let Some(next) = refs.get(i + 1) {
            item.dict_put("Next", next.try_clone().map_err(pdf_err)?)
                .map_err(pdf_err)?;
        }
    }
    if let (Some(first), Some(last)) = (refs.first(), refs.last()) {
        let mut parent = parent.try_clone().map_err(pdf_err)?;
        parent
            .dict_put("First", first.try_clone().map_err(pdf_err)?)
            .map_err(pdf_err)?;
        parent
            .dict_put("Last", last.try_clone().map_err(pdf_err)?)
            .map_err(pdf_err)?;
    }
    if refs.len() as i64 != nodes.len() as i64 {
        return Err(Error::Other("marcadores incoherentes".into()));
    }
    Ok(count)
}

/// Reubica los marcadores con `map` (página antigua → página nueva). Los nodos cuya página desaparece se
/// descartan y sus hijos válidos suben de nivel; los nodos sin destino se conservan.
pub(crate) fn retarget(nodes: &[OutlineNode], map: &dyn Fn(u32) -> Option<u32>) -> Vec<OutlineNode> {
    let mut out = Vec::new();
    for n in nodes {
        let kids = retarget(&n.children, map);
        match n.page {
            Some(p) => match map(p) {
                Some(q) => out.push(OutlineNode {
                    title: n.title.clone(),
                    page: Some(q),
                    children: kids,
                }),
                None => out.extend(kids),
            },
            None => out.push(OutlineNode {
                title: n.title.clone(),
                page: None,
                children: kids,
            }),
        }
    }
    out
}
