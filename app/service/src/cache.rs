//! Caché de teselas con límite de bytes y expulsión del menos usado recientemente.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use crate::DocId;

/// Clave de caché: la revisión invalida las teselas anteriores a un cambio del documento.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TileKey {
    pub doc: DocId,
    pub revision: u64,
    pub page: u32,
    pub zoom_bits: u32,
    pub col: u32,
    pub row: u32,
}

#[derive(Debug)]
pub struct TileCache {
    max_bytes: usize,
    bytes: usize,
    map: HashMap<TileKey, Arc<Vec<u8>>>,
    order: VecDeque<TileKey>,
}

impl TileCache {
    pub fn new(max_bytes: usize) -> Self {
        Self {
            max_bytes,
            bytes: 0,
            map: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    pub fn get(&mut self, key: &TileKey) -> Option<Arc<Vec<u8>>> {
        let hit = self.map.get(key).cloned()?;
        self.order.retain(|k| k != key);
        self.order.push_back(*key);
        Some(hit)
    }

    pub fn insert(&mut self, key: TileKey, data: Arc<Vec<u8>>) {
        if data.len() > self.max_bytes {
            return;
        }
        if let Some(old) = self.map.insert(key, data.clone()) {
            self.bytes -= old.len();
            self.order.retain(|k| k != &key);
        }
        self.bytes += data.len();
        self.order.push_back(key);
        while self.bytes > self.max_bytes {
            let Some(oldest) = self.order.pop_front() else { break };
            if let Some(v) = self.map.remove(&oldest) {
                self.bytes -= v.len();
            }
        }
    }

    /// Elimina todas las teselas de un documento (al cerrarlo).
    pub fn drop_doc(&mut self, doc: DocId) {
        self.order.retain(|k| k.doc != doc);
        self.map.retain(|k, v| {
            let keep = k.doc != doc;
            if !keep {
                self.bytes -= v.len();
            }
            keep
        });
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(n: u32) -> TileKey {
        TileKey {
            doc: DocId(1),
            revision: 0,
            page: n,
            zoom_bits: 0,
            col: 0,
            row: 0,
        }
    }

    #[test]
    fn evicts_least_recently_used() {
        let mut c = TileCache::new(20);
        c.insert(key(1), Arc::new(vec![0; 10]));
        c.insert(key(2), Arc::new(vec![0; 10]));
        assert!(c.get(&key(1)).is_some()); // 2 pasa a ser el más antiguo
        c.insert(key(3), Arc::new(vec![0; 10]));
        assert!(c.get(&key(2)).is_none());
        assert!(c.get(&key(1)).is_some() && c.get(&key(3)).is_some());
    }

    #[test]
    fn drop_doc_clears_entries() {
        let mut c = TileCache::new(100);
        c.insert(key(1), Arc::new(vec![0; 10]));
        c.drop_doc(DocId(1));
        assert!(c.is_empty());
        c.insert(key(1), Arc::new(vec![0; 10]));
        assert_eq!(c.len(), 1);
    }
}
