//! Recorrido seguro de los marcadores (/Outlines) de un PDF.
//!
//! Tolera marcadores sin /Title, ciclos (conjunto de visitados), profundidad y número de nodos
//! excesivos, destinos explícitos, con nombre y acciones GoTo/URI.

use std::collections::HashSet;

use pliego_core::Result;

use crate::document::PdfDocument;
use crate::links::{DestinationKind, LinkDestination};
use crate::object::Object;
use crate::options::Outline;

const MAX_DEPTH: usize = 64;
const MAX_NODES: usize = 100_000;

impl PdfDocument {
    /// Marcadores del documento. A diferencia de [`crate::Document::outlines`] (que en MuPDF 0.8 aborta con un
    /// marcador sin `/Title`), recorre el árbol a mano y tolera ciclos y destinos rotos. Los destinos explícitos
    /// solo conservan la página (`DestinationKind::Fit`); los destinos con nombre traen la vista que resuelva el motor.
    pub fn outlines(&self) -> Result<Vec<Outline>> {
        let root = self.trailer().ok().and_then(|t| t.get_dict("Root").ok().flatten());
        let first = root
            .and_then(|r| r.get_dict("Outlines").ok().flatten())
            .and_then(|o| o.get_dict("First").ok().flatten());
        let Some(first) = first else {
            return Ok(Vec::new());
        };
        let mut walker = Walker {
            pdf: self,
            seen: HashSet::new(),
            nodes: 0,
        };
        Ok(walker.siblings(first, 0))
    }
}

struct Walker<'a> {
    pdf: &'a PdfDocument,
    seen: HashSet<i32>,
    nodes: usize,
}

impl Walker<'_> {
    /// Recorre una lista enlazada de hermanos empezando por `node` (con sus hijos).
    fn siblings(&mut self, node: Object, depth: usize) -> Vec<Outline> {
        let mut items = Vec::new();
        let mut cur = Some(node);
        while let Some(node) = cur.take() {
            if depth >= MAX_DEPTH || self.nodes >= MAX_NODES || !node.is_dict().unwrap_or(false) {
                break;
            }
            // Un objeto ya visitado (por referencia indirecta) indica un ciclo.
            let num = node.as_indirect().unwrap_or(0);
            if num != 0 && !self.seen.insert(num) {
                break;
            }
            self.nodes += 1;
            let title = node
                .get_dict("Title")
                .ok()
                .flatten()
                .and_then(|t| t.as_string().ok())
                .unwrap_or_default();
            let (dest, uri) = self.destination(&node);
            let down = match node.get_dict("First").ok().flatten() {
                Some(first) => self.siblings(first, depth + 1),
                None => Vec::new(),
            };
            items.push(Outline { title, uri, dest, down });
            cur = node.get_dict("Next").ok().flatten();
        }
        items
    }

    fn destination(&self, node: &Object) -> (Option<LinkDestination>, Option<String>) {
        if let Some(dest) = node.get_dict("Dest").ok().flatten() {
            return (self.dest_page(&dest), None);
        }
        let Some(action) = node.get_dict("A").ok().flatten() else {
            return (None, None);
        };
        let kind = action
            .get_dict("S")
            .ok()
            .flatten()
            .and_then(|s| s.as_name().ok())
            .unwrap_or_default();
        match kind.as_slice() {
            b"GoTo" => (
                action.get_dict("D").ok().flatten().and_then(|d| self.dest_page(&d)),
                None,
            ),
            b"URI" => (
                None,
                action
                    .get_dict("URI")
                    .ok()
                    .flatten()
                    .and_then(|u| u.as_string().ok())
                    .filter(|u| !u.starts_with('#')),
            ),
            _ => (None, None),
        }
    }

    /// Destino: array explícito `[página /Fit ...]` o nombre/cadena de destino con nombre.
    fn dest_page(&self, dest: &Object) -> Option<LinkDestination> {
        if dest.is_array().unwrap_or(false) {
            let page = dest.get_array(0).ok().flatten()?;
            let n = self.pdf.lookup_page_number(&page).ok()?;
            return Some(LinkDestination {
                page: u32::try_from(n).ok()?,
                kind: DestinationKind::Fit,
            });
        }
        let name = match dest.as_name() {
            Ok(n) => String::from_utf8_lossy(&n).into_owned(),
            Err(_) => dest.as_string().ok()?,
        };
        let uri = format!("#nameddest={}", percent_encode(&name));
        self.pdf.resolve_link(&uri).ok().flatten()
    }
}

fn percent_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}
