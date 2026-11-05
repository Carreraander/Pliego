//! Árbol de páginas (`/Pages`): lista plana, inserción, borrado y reordenación.

use std::collections::HashSet;

use lopdf::Object as L;
use lopdf::{Dictionary, ObjectId};
use pliego_core::Result;

use super::{State, pdf_err};

const MAX_DEPTH: usize = 64;
const INHERITABLE: [&str; 4] = ["Resources", "MediaBox", "CropBox", "Rotate"];

/// ¿Es un nodo del árbol de páginas o el catálogo? (Un cambio en ellos invalida la lista de páginas.)
pub(crate) fn is_tree_node(o: Option<&L>) -> bool {
    let Some(L::Dictionary(d)) = o else { return false };
    d.has(b"Kids") || matches!(d.get(b"Type").and_then(L::as_name), Ok(b"Pages" | b"Page" | b"Catalog"))
}

pub(crate) fn catalog_id(st: &State) -> Option<ObjectId> {
    match st.doc.trailer.get(b"Root") {
        Ok(L::Reference(id)) => Some(st.resolve_id(*id)),
        _ => None,
    }
}

pub(crate) fn pages_root_id(st: &State) -> Option<ObjectId> {
    let cat = st.get(catalog_id(st)?)?;
    match st.dict_of(cat)?.get(b"Pages") {
        Ok(L::Reference(id)) => Some(st.resolve_id(*id)),
        _ => None,
    }
}

fn collect(st: &State, node: ObjectId, out: &mut Vec<ObjectId>, seen: &mut HashSet<ObjectId>, depth: usize) {
    if depth > MAX_DEPTH || !seen.insert(node) {
        return;
    }
    let Some(obj) = st.doc.objects.get(&node) else { return };
    let Some(d) = st.dict_of(obj) else { return };
    let kids = d
        .get(b"Kids")
        .ok()
        .and_then(|k| st.deref(k))
        .and_then(|k| k.as_array().ok());
    match kids {
        Some(kids) => {
            for k in kids {
                if let L::Reference(id) = k {
                    collect(st, st.resolve_id(*id), out, seen, depth + 1);
                }
            }
        }
        None => out.push(node),
    }
}

/// Páginas en orden (ids de objeto). Se guarda en caché hasta que cambie el árbol.
pub(crate) fn page_list(st: &mut State) -> Vec<ObjectId> {
    if let Some(p) = &st.pages_cache {
        return p.clone();
    }
    let mut out = Vec::new();
    if let Some(root) = pages_root_id(st) {
        collect(st, root, &mut out, &mut HashSet::new(), 0);
    }
    st.pages_cache = Some(out.clone());
    out
}

fn dict_mut(st: &mut State, id: ObjectId) -> Option<&mut Dictionary> {
    match st.doc.objects.get_mut(&id)? {
        L::Dictionary(d) => Some(d),
        L::Stream(s) => Some(&mut s.dict),
        _ => None,
    }
}

fn parent_of(st: &State, id: ObjectId) -> Option<ObjectId> {
    match st.dict_of(st.doc.objects.get(&id)?)?.get(b"Parent") {
        Ok(L::Reference(p)) => Some(st.resolve_id(*p)),
        _ => None,
    }
}

/// Suma `delta` a `/Count` de `node` y de todos sus ascendientes.
fn bump_count(st: &mut State, mut node: ObjectId, delta: i64) {
    let mut guard = 0;
    loop {
        let parent = parent_of(st, node);
        if let Some(d) = dict_mut(st, node) {
            let n = d.get(b"Count").and_then(L::as_i64).unwrap_or(0);
            d.set("Count", (n + delta).max(0));
            st.touch(node);
        }
        guard += 1;
        match parent {
            Some(p) if guard < MAX_DEPTH => node = p,
            _ => break,
        }
    }
}

fn kids_vec(st: &State, node: ObjectId) -> Vec<L> {
    st.doc
        .objects
        .get(&node)
        .and_then(|o| st.dict_of(o))
        .and_then(|d| d.get(b"Kids").ok())
        .and_then(|k| st.deref(k))
        .and_then(|k| k.as_array().ok())
        .cloned()
        .unwrap_or_default()
}

/// Escribe `kids` en `node`. Si `/Kids` es una referencia a una matriz, la modifica en su sitio.
fn set_kids(st: &mut State, node: ObjectId, kids: Vec<L>) {
    let target = match st
        .doc
        .objects
        .get(&node)
        .and_then(|o| st.dict_of(o))
        .and_then(|d| d.get(b"Kids").ok())
    {
        Some(L::Reference(id)) => Some(st.resolve_id(*id)),
        _ => None,
    };
    if let Some(arr_id) = target
        && let Some(L::Array(a)) = st.doc.objects.get_mut(&arr_id)
    {
        *a = kids;
        st.touch(arr_id);
        return;
    }
    if let Some(d) = dict_mut(st, node) {
        d.set("Kids", L::Array(kids));
        st.touch(node);
    }
}

/// Inserta la página `page` para que quede en la posición `index` (0..=n).
pub(crate) fn insert_page(st: &mut State, index: usize, page: ObjectId) -> Result<()> {
    let pages = page_list(st);
    let root = pages_root_id(st).ok_or_else(|| pdf_err("el documento no tiene árbol de páginas"))?;
    let (parent, pos) = if index >= pages.len() {
        // Al final: en el último nodo `Pages` que contiene páginas, o en la raíz.
        match pages.last().and_then(|&last| parent_of(st, last)) {
            Some(p)
                if kids_vec(st, p)
                    .iter()
                    .any(|k| matches!(k, L::Reference(r) if st.resolve_id(*r) == *pages.last().unwrap())) =>
            {
                let n = kids_vec(st, p).len();
                (p, n)
            }
            _ => {
                let n = kids_vec(st, root).len();
                (root, n)
            }
        }
    } else {
        let target = pages[index];
        let found = parent_of(st, target).and_then(|p| {
            kids_vec(st, p)
                .iter()
                .position(|k| matches!(k, L::Reference(r) if st.resolve_id(*r) == target))
                .map(|pos| (p, pos))
        });
        match found {
            Some(f) => f,
            None => {
                // `/Parent` roto: se busca el nodo que contiene la página.
                find_parent(st, root, target).ok_or_else(|| pdf_err("no se encuentra el nodo padre de la página"))?
            }
        }
    };
    let mut kids = kids_vec(st, parent);
    kids.insert(pos.min(kids.len()), L::Reference(page));
    set_kids(st, parent, kids);
    if let Some(d) = dict_mut(st, page) {
        d.set("Parent", L::Reference(parent));
        st.touch(page);
    }
    bump_count(st, parent, 1);
    st.invalidate_pages();
    Ok(())
}

fn find_parent(st: &State, node: ObjectId, target: ObjectId) -> Option<(ObjectId, usize)> {
    fn go(
        st: &State,
        node: ObjectId,
        target: ObjectId,
        depth: usize,
        seen: &mut HashSet<ObjectId>,
    ) -> Option<(ObjectId, usize)> {
        if depth > MAX_DEPTH || !seen.insert(node) {
            return None;
        }
        let kids = kids_vec(st, node);
        for (i, k) in kids.iter().enumerate() {
            if let L::Reference(r) = k {
                let r = st.resolve_id(*r);
                if r == target {
                    return Some((node, i));
                }
                if let Some(f) = go(st, r, target, depth + 1, seen) {
                    return Some(f);
                }
            }
        }
        None
    }
    go(st, node, target, 0, &mut HashSet::new())
}

/// Quita la página `index` del árbol (el objeto queda huérfano hasta la limpieza).
pub(crate) fn remove_page(st: &mut State, index: usize) -> Result<()> {
    let pages = page_list(st);
    let target = *pages
        .get(index)
        .ok_or_else(|| pdf_err(format!("la página {} no existe", index + 1)))?;
    let root = pages_root_id(st).ok_or_else(|| pdf_err("el documento no tiene árbol de páginas"))?;
    let (parent, pos) = find_parent(st, root, target).ok_or_else(|| pdf_err("la página no cuelga del árbol"))?;
    let mut kids = kids_vec(st, parent);
    kids.remove(pos);
    set_kids(st, parent, kids);
    bump_count(st, parent, -1);
    st.invalidate_pages();
    Ok(())
}

/// Copia a la página los atributos heredables que solo tiene en un ascendiente.
pub(crate) fn flatten_inheritable(st: &mut State, page: ObjectId) {
    let mut found: Vec<(&str, L)> = Vec::new();
    for key in INHERITABLE {
        let has = st
            .doc
            .objects
            .get(&page)
            .and_then(|o| st.dict_of(o))
            .is_some_and(|d| d.has(key.as_bytes()));
        if has {
            continue;
        }
        let mut cur = parent_of(st, page);
        for _ in 0..MAX_DEPTH {
            let Some(p) = cur else { break };
            let v = st
                .doc
                .objects
                .get(&p)
                .and_then(|o| st.dict_of(o))
                .and_then(|d| d.get(key.as_bytes()).ok())
                .cloned();
            if let Some(v) = v {
                found.push((key, v));
                break;
            }
            cur = parent_of(st, p);
        }
    }
    if found.is_empty() {
        return;
    }
    if let Some(d) = dict_mut(st, page) {
        for (k, v) in found {
            d.set(k, v);
        }
        st.touch(page);
    }
}

/// Deja el árbol plano con las páginas `order` (índices de la lista actual; un repetido se copia).
pub(crate) fn rearrange(st: &mut State, order: &[usize]) -> Result<()> {
    let pages = page_list(st);
    let root = pages_root_id(st).ok_or_else(|| pdf_err("el documento no tiene árbol de páginas"))?;
    for &i in order {
        if i >= pages.len() {
            return Err(pdf_err(format!("la página {} no existe", i + 1)));
        }
    }
    let mut used = HashSet::new();
    let mut kids = Vec::with_capacity(order.len());
    for &i in order {
        let id = pages[i];
        flatten_inheritable(st, id);
        let page_id = if used.insert(id) {
            id
        } else {
            let copy = st
                .doc
                .objects
                .get(&id)
                .cloned()
                .ok_or_else(|| pdf_err("página sin objeto"))?;
            st.add(copy)
        };
        if let Some(d) = dict_mut(st, page_id) {
            d.set("Parent", L::Reference(root));
            st.touch(page_id);
        }
        kids.push(L::Reference(page_id));
    }
    let n = kids.len() as i64;
    if let Some(d) = dict_mut(st, root) {
        d.set("Kids", L::Array(kids));
        d.set("Count", n);
        st.touch(root);
    }
    st.invalidate_pages();
    Ok(())
}
