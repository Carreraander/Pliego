//! Recorrido seguro del árbol de estructura lógica (StructTreeRoot).

use std::collections::{HashMap, HashSet};

use mupdf::pdf::{PdfDocument, PdfObject};
use pliego_core::Result;

use crate::io::{name_of, pdf_err, xref_of};

/// Límites contra documentos hostiles.
const MAX_DEPTH: usize = 128;
const MAX_NODES: usize = 200_000;

/// Un elemento de estructura, con su tipo ya traducido a un tipo estándar mediante RoleMap.
#[derive(Debug, Clone)]
pub(crate) struct Elem {
    pub xref: i32,
    /// Tipo estándar (`P`, `H1`, `Figure`...). Si el tipo propio no está en RoleMap, es el propio.
    pub role: String,
    pub page: Option<usize>,
    pub has_alt: bool,
    /// Contenido marcado (MCID o MCR/OBJR) colgando directamente de este elemento.
    pub has_content: bool,
    pub children: Vec<usize>,
}

#[derive(Debug, Default)]
pub(crate) struct Tree {
    pub elems: Vec<Elem>,
    /// Hubo que dejar de recorrer por exceder los límites.
    pub truncated: bool,
}

fn nonempty_string(obj: &PdfObject, key: &str) -> bool {
    obj.get_dict(key)
        .ok()
        .flatten()
        .and_then(|v| v.as_string().ok())
        .is_some_and(|s| !s.trim().is_empty())
}

fn role_map(root: &PdfObject) -> HashMap<String, String> {
    let mut map = HashMap::new();
    if let Ok(Some(rm)) = root.get_dict("RoleMap")
        && let Ok(iter) = rm.dict_iter()
    {
        for entry in iter.flatten() {
            let (k, v) = entry;
            if let (Ok(k), Ok(v)) = (k.as_name(), v.as_name()) {
                map.insert(
                    String::from_utf8_lossy(&k).into_owned(),
                    String::from_utf8_lossy(&v).into_owned(),
                );
            }
        }
    }
    map
}

fn resolve_role(raw: &str, map: &HashMap<String, String>) -> String {
    let mut cur = raw.to_string();
    for _ in 0..16 {
        match map.get(&cur) {
            Some(next) if *next != cur => cur = next.clone(),
            _ => break,
        }
    }
    cur
}

struct Walker<'a> {
    doc: &'a PdfDocument,
    map: HashMap<String, String>,
    tree: Tree,
    seen: HashSet<i32>,
}

impl Walker<'_> {
    fn page_of(&self, pg: &PdfObject) -> Option<usize> {
        self.doc
            .lookup_page_number(pg)
            .ok()
            .and_then(|n| usize::try_from(n).ok())
    }

    /// Procesa un nodo hijo de `/K`: puede ser elemento, MCR, OBJR, entero (MCID) o matriz.
    fn visit(&mut self, node: &PdfObject, parent: Option<usize>, page: Option<usize>, depth: usize) -> Result<()> {
        if depth > MAX_DEPTH || self.tree.elems.len() > MAX_NODES {
            self.tree.truncated = true;
            return Ok(());
        }
        if node.is_int().unwrap_or(false) {
            if let Some(p) = parent {
                self.tree.elems[p].has_content = true;
            }
            return Ok(());
        }
        if node.is_array().unwrap_or(false) {
            for i in 0..node.len().map_err(pdf_err)? {
                if let Some(child) = node.get_array(i as i32).map_err(pdf_err)? {
                    self.visit(&child, parent, page, depth + 1)?;
                }
            }
            return Ok(());
        }
        if !node.is_dict().unwrap_or(false) {
            return Ok(());
        }
        // Referencias a contenido (MCR / OBJR) no son elementos de estructura.
        let ty = name_of(node, "Type");
        if matches!(ty.as_deref(), Some("MCR") | Some("OBJR")) {
            if let Some(p) = parent {
                self.tree.elems[p].has_content = true;
            }
            return Ok(());
        }
        let x = xref_of(node);
        if x != 0 && !self.seen.insert(x) {
            return Ok(()); // ciclo o nodo repetido
        }
        let Some(raw) = name_of(node, "S") else {
            return Ok(());
        };
        let own_page = node
            .get_dict("Pg")
            .ok()
            .flatten()
            .and_then(|pg| self.page_of(&pg))
            .or(page);
        let idx = self.tree.elems.len();
        self.tree.elems.push(Elem {
            xref: x,
            role: resolve_role(&raw, &self.map),
            page: own_page,
            has_alt: nonempty_string(node, "Alt") || nonempty_string(node, "ActualText"),
            has_content: false,
            children: Vec::new(),
        });
        if let Some(p) = parent {
            self.tree.elems[p].children.push(idx);
        }
        if let Some(k) = node.get_dict("K").map_err(pdf_err)? {
            self.visit(&k, Some(idx), own_page, depth + 1)?;
        }
        Ok(())
    }
}

/// Lee el árbol de estructura. `None` si el documento no tiene `StructTreeRoot`.
pub(crate) fn read_tree(doc: &PdfDocument) -> Result<Option<Tree>> {
    let catalog = doc.catalog().map_err(pdf_err)?;
    let Some(root) = catalog.get_dict("StructTreeRoot").map_err(pdf_err)? else {
        return Ok(None);
    };
    if !root.is_dict().unwrap_or(false) {
        return Ok(None);
    }
    let mut w = Walker {
        doc,
        map: role_map(&root),
        tree: Tree::default(),
        seen: HashSet::new(),
    };
    if let Some(k) = root.get_dict("K").map_err(pdf_err)? {
        w.visit(&k, None, None, 0)?;
    }
    Ok(Some(w.tree))
}
