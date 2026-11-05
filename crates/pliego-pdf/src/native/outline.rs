//! Marcadores (`/Outlines`) y destinos con nombre.

use std::collections::HashSet;

use lopdf::Object as L;
use lopdf::{Dictionary, ObjectId, StringFormat};
use pliego_core::Result;

use super::document::{PdfDocument, catalog_dict, catalog_id, decode_text, name_tree, set_dict_entry, text_string};
use super::page::{dest_array, dest_kind};
use super::{State, pdf_err};
use crate::links::{DestinationKind, LinkDestination};
use crate::options::Outline;

const MAX_DEPTH: usize = 64;
const MAX_NODES: usize = 100_000;

pub(crate) fn read(doc: &PdfDocument) -> Result<Vec<Outline>> {
    let pages = doc.page_ids();
    let st = doc.st.borrow();
    let first = catalog_dict(&st)
        .and_then(|c| c.get(b"Outlines").ok())
        .and_then(|o| st.dict_of(o))
        .and_then(|o| o.get(b"First").ok());
    let Some(first) = first else {
        return Ok(Vec::new());
    };
    let mut w = Walker {
        st: &st,
        pages: &pages,
        seen: HashSet::new(),
        nodes: 0,
    };
    Ok(w.siblings(first, 0))
}

struct Walker<'a> {
    st: &'a State,
    pages: &'a [ObjectId],
    seen: HashSet<ObjectId>,
    nodes: usize,
}

impl Walker<'_> {
    fn siblings(&mut self, node: &L, depth: usize) -> Vec<Outline> {
        let mut items = Vec::new();
        let mut cur = Some(node);
        while let Some(node) = cur.take() {
            if depth >= MAX_DEPTH || self.nodes >= MAX_NODES {
                break;
            }
            if let L::Reference(id) = node
                && !self.seen.insert(self.st.resolve_id(*id))
            {
                break;
            }
            let Some(d) = self.st.dict_of(node) else { break };
            self.nodes += 1;
            let title = d
                .get(b"Title")
                .ok()
                .and_then(|t| self.st.deref(t))
                .and_then(decode_text)
                .unwrap_or_default();
            let (dest, uri) = self.destination(d);
            let down = match d.get(b"First") {
                Ok(first) => self.siblings(first, depth + 1),
                Err(_) => Vec::new(),
            };
            items.push(Outline { title, uri, dest, down });
            cur = d.get(b"Next").ok();
        }
        items
    }

    fn destination(&self, node: &Dictionary) -> (Option<LinkDestination>, Option<String>) {
        if let Ok(dest) = node.get(b"Dest") {
            return (dest_of(self.st, self.pages, dest), None);
        }
        let Some(action) = node.get(b"A").ok().and_then(|a| self.st.dict_of(a)) else {
            return (None, None);
        };
        match action.get(b"S").ok().and_then(|s| s.as_name().ok()) {
            Some(b"GoTo") => (
                action.get(b"D").ok().and_then(|d| dest_of(self.st, self.pages, d)),
                None,
            ),
            Some(b"URI") => (
                None,
                action
                    .get(b"URI")
                    .ok()
                    .and_then(|u| self.st.deref(u))
                    .and_then(decode_text)
                    .filter(|u| !u.starts_with('#')),
            ),
            _ => (None, None),
        }
    }
}

/// Destino explícito, o con nombre (se busca en `/Dests` y en el árbol de nombres).
pub(crate) fn dest_of(st: &State, pages: &[ObjectId], dest: &L) -> Option<LinkDestination> {
    dest_depth(st, pages, dest, 0)
}

fn dest_depth(st: &State, pages: &[ObjectId], dest: &L, depth: usize) -> Option<LinkDestination> {
    if depth > 4 {
        return None;
    }
    let dest = st.deref(dest)?;
    match dest {
        L::Array(a) => {
            let page = match a.first()? {
                L::Reference(id) => {
                    let id = st.resolve_id(*id);
                    pages.iter().position(|p| *p == id)? as u32
                }
                // Destinos remotos usan el número de página; en el propio documento no deberían aparecer.
                L::Integer(n) => u32::try_from(*n).ok().filter(|n| (*n as usize) < pages.len())?,
                _ => return None,
            };
            Some(LinkDestination {
                page,
                kind: dest_kind(a),
            })
        }
        L::Name(n) => named(st, pages, n, depth),
        L::String(s, _) => named(st, pages, s, depth),
        L::Dictionary(d) => d.get(b"D").ok().and_then(|d| dest_depth(st, pages, d, depth + 1)),
        _ => None,
    }
}

fn named(st: &State, pages: &[ObjectId], name: &[u8], depth: usize) -> Option<LinkDestination> {
    let cat = catalog_dict(st)?;
    if let Some(dests) = cat.get(b"Dests").ok().and_then(|d| st.dict_of(d))
        && let Ok(v) = dests.get(name)
        && let Some(r) = dest_depth(st, pages, v, depth + 1)
    {
        return Some(r);
    }
    let tree = cat
        .get(b"Names")
        .ok()
        .and_then(|n| st.dict_of(n))
        .and_then(|n| n.get(b"Dests").ok())?;
    name_tree(st, tree)
        .into_iter()
        .find(|(k, _)| k.as_slice() == name)
        .and_then(|(_, v)| dest_depth(st, pages, &v, depth + 1))
}

pub(crate) fn resolve_link(doc: &PdfDocument, uri: &str) -> Result<Option<LinkDestination>> {
    let pages = doc.page_ids();
    let st = doc.st.borrow();
    let Some(frag) = uri.strip_prefix('#') else {
        return Ok(None);
    };
    if let Some(rest) = frag.strip_prefix("page=") {
        let n: usize = rest.split(',').next().unwrap_or("").trim().parse().unwrap_or(0);
        return Ok((n >= 1 && n <= pages.len()).then(|| LinkDestination {
            page: (n - 1) as u32,
            kind: DestinationKind::Fit,
        }));
    }
    let name = frag.strip_prefix("nameddest=").unwrap_or(frag);
    let name = percent_decode(name);
    Ok(named(&st, &pages, &name, 0))
}

fn percent_decode(s: &str) -> Vec<u8> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Some(v) = std::str::from_utf8(&b[i + 1..i + 3])
                .ok()
                .and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(v);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    out
}

pub(crate) fn write(doc: &mut PdfDocument, toc: &[Outline]) -> Result<()> {
    let pages = doc.page_ids();
    let mut st = doc.st.borrow_mut();
    let cat = catalog_id(&st)?;
    if toc.is_empty() {
        if let Some(d) = super::document::dict_of_mut(&mut st, cat) {
            d.remove(b"Outlines");
            st.touch(cat);
        }
        return Ok(());
    }
    let root = st.add(L::Null);
    let (ids, total) = build(&mut st, toc, root, &pages)?;
    let mut d = Dictionary::new();
    d.set("Type", L::Name(b"Outlines".to_vec()));
    d.set("First", L::Reference(ids[0]));
    d.set("Last", L::Reference(*ids.last().expect("no vacío")));
    d.set("Count", total);
    st.doc.objects.insert(root, L::Dictionary(d));
    st.touch(root);
    set_dict_entry(&mut st, cat, "Outlines", L::Reference(root))
}

/// Crea los nodos de un nivel. Devuelve sus ids y el total de nodos visibles (ellos y sus descendientes).
fn build(st: &mut State, items: &[Outline], parent: ObjectId, pages: &[ObjectId]) -> Result<(Vec<ObjectId>, i64)> {
    let ids: Vec<ObjectId> = items.iter().map(|_| st.add(L::Null)).collect();
    let mut total = items.len() as i64;
    for (i, item) in items.iter().enumerate() {
        let mut d = Dictionary::new();
        d.set("Title", text_string(&item.title));
        d.set("Parent", L::Reference(parent));
        if i > 0 {
            d.set("Prev", L::Reference(ids[i - 1]));
        }
        if i + 1 < ids.len() {
            d.set("Next", L::Reference(ids[i + 1]));
        }
        if let Some(dest) = item.dest {
            let pid = *pages.get(dest.page as usize).ok_or_else(|| {
                pdf_err(format!(
                    "el marcador apunta a la página {}, que no existe",
                    dest.page + 1
                ))
            })?;
            d.set("Dest", L::Array(dest_array(pid, dest.kind)));
        } else if let Some(uri) = &item.uri {
            let mut a = Dictionary::new();
            a.set("S", L::Name(b"URI".to_vec()));
            a.set("URI", L::String(uri.as_bytes().to_vec(), StringFormat::Literal));
            d.set("A", L::Dictionary(a));
        }
        if !item.down.is_empty() {
            let (kids, n) = build(st, &item.down, ids[i], pages)?;
            d.set("First", L::Reference(kids[0]));
            d.set("Last", L::Reference(*kids.last().expect("no vacío")));
            d.set("Count", n);
            total += n;
        }
        st.doc.objects.insert(ids[i], L::Dictionary(d));
        st.touch(ids[i]);
    }
    Ok((ids, total))
}
