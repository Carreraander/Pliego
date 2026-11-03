//! Caché LRU de teselas con límite de memoria.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};

use pliego_core::RgbaImage;

/// Clave de tesela: (documento, revisión, página, zoom, columna, fila). El zoom va como bits de `f32`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TileKey {
    pub doc: u64,
    pub revision: u64,
    pub page: u32,
    pub zoom_bits: u32,
    pub col: u32,
    pub row: u32,
}

impl TileKey {
    pub fn new(doc: u64, revision: u64, page: u32, zoom: f32, col: u32, row: u32) -> Self {
        Self {
            doc,
            revision,
            page,
            zoom_bits: zoom.to_bits(),
            col,
            row,
        }
    }
}

#[derive(Default)]
struct Inner {
    map: HashMap<TileKey, (Arc<RgbaImage>, u64)>,
    /// Orden de uso: marca → clave. La marca más baja es la menos usada recientemente.
    order: BTreeMap<u64, TileKey>,
    tick: u64,
    bytes: usize,
}

/// Caché compartible entre hilos y documentos.
pub struct TileCache {
    max_bytes: usize,
    inner: Mutex<Inner>,
}

impl TileCache {
    pub fn new(max_bytes: usize) -> Self {
        Self {
            max_bytes,
            inner: Mutex::new(Inner::default()),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub fn get(&self, key: &TileKey) -> Option<Arc<RgbaImage>> {
        let mut g = self.lock();
        let g = &mut *g;
        let (img, old) = g.map.get_mut(key)?;
        g.tick += 1;
        g.order.remove(old);
        *old = g.tick;
        g.order.insert(g.tick, *key);
        Some(img.clone())
    }

    /// Inserta una tesela y expulsa las menos usadas hasta cumplir el límite.
    /// Una tesela mayor que el límite entero no se guarda.
    pub fn insert(&self, key: TileKey, img: Arc<RgbaImage>) {
        let size = img.data.len();
        if size > self.max_bytes {
            return;
        }
        let mut g = self.lock();
        if let Some((old_img, old_tick)) = g.map.remove(&key) {
            g.order.remove(&old_tick);
            g.bytes -= old_img.data.len();
        }
        while g.bytes + size > self.max_bytes {
            let Some((_, victim)) = g.order.pop_first() else { break };
            if let Some((v, _)) = g.map.remove(&victim) {
                g.bytes -= v.data.len();
            }
        }
        g.tick += 1;
        let tick = g.tick;
        g.order.insert(tick, key);
        g.map.insert(key, (img, tick));
        g.bytes += size;
    }

    /// Descarta las teselas de `doc` con revisión distinta de `keep_revision`.
    pub fn drop_stale(&self, doc: u64, keep_revision: u64) {
        self.retain(|k| !(k.doc == doc && k.revision != keep_revision));
    }

    /// Descarta todas las teselas de `doc`.
    pub fn drop_doc(&self, doc: u64) {
        self.retain(|k| k.doc != doc);
    }

    fn retain(&self, keep: impl Fn(&TileKey) -> bool) {
        let mut g = self.lock();
        let g = &mut *g;
        let (map, order, bytes) = (&mut g.map, &mut g.order, &mut g.bytes);
        map.retain(|k, (img, tick)| {
            let ok = keep(k);
            if !ok {
                order.remove(tick);
                *bytes -= img.data.len();
            }
            ok
        });
    }

    pub fn len(&self) -> usize {
        self.lock().map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Memoria ocupada por los píxeles cacheados.
    pub fn bytes(&self) -> usize {
        self.lock().bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn img(n: usize) -> Arc<RgbaImage> {
        Arc::new(RgbaImage {
            width: 1,
            height: 1,
            data: vec![0; n],
        })
    }

    fn key(col: u32) -> TileKey {
        TileKey::new(1, 0, 0, 1.0, col, 0)
    }

    #[test]
    fn evicts_least_recently_used() {
        let c = TileCache::new(300);
        for i in 0..3 {
            c.insert(key(i), img(100));
        }
        assert!(c.get(&key(0)).is_some()); // 0 pasa a ser la más reciente; 1 es la más antigua
        c.insert(key(3), img(100));
        assert!(c.get(&key(1)).is_none());
        assert!(c.get(&key(0)).is_some() && c.get(&key(2)).is_some() && c.get(&key(3)).is_some());
        assert_eq!(c.bytes(), 300);
    }

    #[test]
    fn respects_limit_and_oversized() {
        let c = TileCache::new(250);
        c.insert(key(0), img(1000));
        assert!(c.is_empty());
        c.insert(key(1), img(200));
        c.insert(key(1), img(100));
        assert_eq!((c.len(), c.bytes()), (1, 100));
    }

    #[test]
    fn invalidates_by_revision_and_doc() {
        let c = TileCache::new(10_000);
        c.insert(TileKey::new(1, 0, 0, 1.0, 0, 0), img(10));
        c.insert(TileKey::new(1, 1, 0, 1.0, 0, 0), img(10));
        c.insert(TileKey::new(2, 0, 0, 1.0, 0, 0), img(10));
        c.drop_stale(1, 1);
        assert_eq!(c.len(), 2);
        c.drop_doc(1);
        assert_eq!((c.len(), c.bytes()), (1, 10));
    }
}
