use pliego_core::{Error, Result};
use pliego_pdf::PdfDocument;
use serde::{Deserialize, Serialize};

/// Marcador (índice) con su página de destino desde 0.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct OutlineNode {
    pub title: String,
    pub page: Option<u32>,
    pub children: Vec<OutlineNode>,
    /// Altura de destino en la página (coordenadas del PDF, origen abajo), si el marcador lleva a un punto concreto y
    /// no a la página entera. Se conserva al reescribir los marcadores.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top: Option<f32>,
    /// Dirección web, para los marcadores que abren un enlace en vez de ir a una página.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
}

/// Altura del destino si el marcador apunta a un punto de la página.
fn dest_top(kind: &pliego_pdf::DestinationKind) -> Option<f32> {
    use pliego_pdf::DestinationKind as K;
    match kind {
        K::XYZ { top, .. } | K::FitH { top } | K::FitBH { top } => *top,
        K::FitR { top, .. } => Some(*top),
        _ => None,
    }
}

/// Lee los marcadores de un documento.
pub fn read_outline(doc: &PdfDocument) -> Result<Vec<OutlineNode>> {
    fn conv(items: Vec<pliego_pdf::Outline>) -> Vec<OutlineNode> {
        items
            .into_iter()
            .map(|o| OutlineNode {
                title: o.title,
                page: o.dest.as_ref().map(|d| d.page),
                top: o.dest.as_ref().and_then(|d| dest_top(&d.kind)),
                uri: o.uri,
                children: conv(o.down),
            })
            .collect()
    }
    Ok(conv(doc.outlines()?))
}

/// Sustituye los marcadores del documento. Los nodos con página fuera de rango se escriben sin destino.
pub fn write_outline(doc: &mut PdfDocument, nodes: &[OutlineNode]) -> Result<()> {
    let mut catalog = doc.catalog()?;
    if nodes.is_empty() {
        let _ = catalog.dict_delete("Outlines");
        return Ok(());
    }
    let page_count = doc.page_count()?.max(0) as u32;
    let root = doc.new_dict()?;
    let mut root = doc.add_object(&root)?;
    root.dict_put("Type", pliego_pdf::Object::new_name("Outlines")?)?;
    let total = build_level(doc, nodes, &root, page_count)?;
    root.dict_put("Count", pliego_pdf::Object::new_int(total)?)?;
    catalog.dict_put("Outlines", root)?;
    Ok(())
}

/// Crea los hijos de `parent`; devuelve cuántos elementos visibles cuelgan de él.
fn build_level(
    doc: &mut PdfDocument,
    nodes: &[OutlineNode],
    parent: &pliego_pdf::Object,
    page_count: u32,
) -> Result<i32> {
    use pliego_pdf::Object;
    let mut refs: Vec<Object> = Vec::new();
    let mut count = 0;
    for node in nodes {
        let dict = doc.new_dict()?;
        let mut item = doc.add_object(&dict)?;
        item.dict_put("Title", doc.new_text_string(&node.title)?)?;
        item.dict_put("Parent", parent.try_clone()?)?;
        if let Some(p) = node.page.filter(|&p| p < page_count) {
            let page = doc.find_page(p as i32)?;
            let mut dest = doc.new_array()?;
            dest.array_push(page)?;
            match node.top {
                // [página /XYZ null alto null]: va a esa altura sin cambiar el zoom.
                Some(top) => {
                    dest.array_push(Object::new_name("XYZ")?)?;
                    dest.array_push(Object::new_null())?;
                    dest.array_push(Object::new_real(top)?)?;
                    dest.array_push(Object::new_null())?;
                }
                None => dest.array_push(Object::new_name("Fit")?)?,
            }
            item.dict_put("Dest", dest)?;
        } else if let Some(uri) = node.uri.as_deref().filter(|u| !u.is_empty()) {
            let mut action = doc.new_dict()?;
            action.dict_put("S", Object::new_name("URI")?)?;
            action.dict_put("URI", Object::new_string(uri)?)?;
            item.dict_put("A", action)?;
        }
        let below = build_level(doc, &node.children, &item, page_count)?;
        if below > 0 {
            item.dict_put("Count", Object::new_int(below)?)?;
        }
        count += 1 + below;
        refs.push(item);
    }
    for (i, item) in refs.iter().enumerate() {
        let mut item = item.try_clone()?;
        if i > 0 {
            item.dict_put("Prev", refs[i - 1].try_clone()?)?;
        }
        if let Some(next) = refs.get(i + 1) {
            item.dict_put("Next", next.try_clone()?)?;
        }
    }
    if let (Some(first), Some(last)) = (refs.first(), refs.last()) {
        let mut parent = parent.try_clone()?;
        parent.dict_put("First", first.try_clone()?)?;
        parent.dict_put("Last", last.try_clone()?)?;
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
                    page: Some(q),
                    children: kids,
                    ..n.clone()
                }),
                None => out.extend(kids),
            },
            None => out.push(OutlineNode {
                children: kids,
                ..n.clone()
            }),
        }
    }
    out
}
