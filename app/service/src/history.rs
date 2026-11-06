//! Historial de deshacer del [`crate::annot::Editor`] con presupuesto de memoria.
//!
//! Cada paso es el contenido **anterior** de un PDF (el archivo entero). Guardarlos todos en memoria no escala: con un
//! PDF de 1 GB y 30 pasos serían 30 GB. Por eso:
//!
//! - Los pasos pequeños (hasta [`Limits::mem_step`]) se guardan en memoria mientras todos juntos quepan en
//!   [`Limits::mem_total`].
//! - El resto se escribe en un temporal **junto al documento** (en su misma unidad: ahí ya hay que tener sitio para la
//!   copia de seguridad y las versiones nuevas) y no ocupa memoria. Se borra al descartar el paso o al cerrar la app.
//! - Todos los temporales juntos caben en [`Limits::disk_total`]; al pasarse se descartan los pasos más antiguos de
//!   cualquier documento. Un paso que no cabe ni solo, o que no se puede escribir, vacía el historial de ese
//!   documento: es mejor no poder deshacer que deshacer saltándose una versión.

use std::collections::{HashMap, VecDeque};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use pliego_core::{Error, Result};

/// Límites del historial.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Pasos que se pueden deshacer por documento.
    pub max_steps: usize,
    /// Mayor paso que se guarda en memoria.
    pub mem_step: usize,
    /// Memoria total de los pasos guardados en memoria (todos los documentos).
    pub mem_total: usize,
    /// Disco total de los temporales (todos los documentos).
    pub disk_total: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_steps: 30,
            mem_step: 4 * 1024 * 1024,
            mem_total: 24 * 1024 * 1024,
            disk_total: 4 * 1024 * 1024 * 1024,
        }
    }
}

enum Store {
    Mem(Vec<u8>),
    Disk(tempfile::TempPath),
    /// El cambio solo añadió bytes al final: el contenido anterior es el principio del archivo (sin copia).
    Tail {
        before: u64,
        after: u64,
    },
}

/// Contenido anterior de un archivo, listo para apilar ([`History::capture`]).
pub enum Previous {
    Mem(Vec<u8>),
    /// Copia ya hecha en un temporal junto al documento.
    Disk(tempfile::TempPath, u64),
    /// El cambio solo añade al final del archivo (guardado incremental): el contenido anterior son sus primeros
    /// `before` bytes y el archivo mide `after` tras el cambio. No ocupa memoria ni disco.
    Tail {
        before: u64,
        after: u64,
    },
    /// No se pudo conservar: al apilarlo se vacía el historial del archivo.
    Lost,
}

/// Lo que devuelve [`History::pop`]: en memoria, o un temporal que el llamador copia sobre el archivo.
pub enum Popped {
    Mem(Vec<u8>),
    Disk(tempfile::TempPath),
    /// Basta con cortar el archivo en `before` bytes (ahora debe medir `after`).
    Tail {
        before: u64,
        after: u64,
    },
}

struct Step {
    /// Orden de creación en todo el historial (para descartar el más antiguo).
    seq: u64,
    size: u64,
    store: Store,
}

#[derive(Default)]
struct Inner {
    /// Pasos por archivo, el más antiguo delante. La clave existe aunque la pila esté vacía.
    stacks: HashMap<PathBuf, VecDeque<Step>>,
    seq: u64,
    mem: usize,
    disk: u64,
}

impl Inner {
    fn forget(&mut self, step: &Step) {
        match step.store {
            Store::Mem(_) => self.mem -= step.size as usize,
            Store::Disk(_) => self.disk -= step.size,
            Store::Tail { .. } => {}
        }
    }

    /// Descarta el paso más antiguo de todos (cualquier documento). `false` si no queda ninguno.
    fn evict_oldest(&mut self) -> bool {
        let oldest = self
            .stacks
            .iter()
            .filter_map(|(p, s)| s.front().map(|st| (st.seq, p.clone())))
            .min_by_key(|(seq, _)| *seq);
        let Some((_, path)) = oldest else { return false };
        if let Some(step) = self.stacks.get_mut(&path).and_then(VecDeque::pop_front) {
            self.forget(&step);
        }
        true
    }

    fn clear(&mut self, path: &Path) {
        if let Some(stack) = self.stacks.get_mut(path) {
            let steps: Vec<Step> = stack.drain(..).collect();
            for s in &steps {
                self.forget(s);
            }
        }
    }
}

/// Pilas de deshacer por archivo, con [`Limits`].
#[derive(Default)]
pub struct History {
    limits: Limits,
    inner: Mutex<Inner>,
}

impl History {
    pub fn new(limits: Limits) -> Self {
        Self {
            limits,
            inner: Mutex::default(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// `true` si ya se registró algún cambio de `path` (aunque ya no quede nada que deshacer).
    pub fn tracks(&self, path: &Path) -> bool {
        self.lock().stacks.contains_key(path)
    }

    /// Pasos que se pueden deshacer.
    pub fn depth(&self, path: &Path) -> usize {
        self.lock().stacks.get(path).map_or(0, VecDeque::len)
    }

    /// Toma el contenido actual de `path` **antes** de cambiarlo: lo lee si es pequeño y, si no, lo copia de archivo a
    /// archivo a un temporal junto al documento (sin pasarlo por memoria). Se apila después con [`History::push`].
    pub fn capture(&self, path: &Path) -> Result<Previous> {
        let size = std::fs::metadata(path).map_err(|e| Error::io_at(path, e))?.len();
        if size <= self.limits.mem_step as u64 {
            return std::fs::read(path)
                .map(Previous::Mem)
                .map_err(|e| Error::io_at(path, e));
        }
        // Sin sitio para el temporal no se cancela la edición: simplemente no habrá paso de deshacer (véase `push`).
        Ok(spill_copy(path).map_or(Previous::Lost, |t| Previous::Disk(t, size)))
    }

    /// Anota `previous` (el contenido de `path` antes del cambio). Nunca falla: si no se puede conservar, se vacía el
    /// historial de `path` para que deshacer no se salte versiones.
    pub fn push(&self, path: &Path, previous: Previous) {
        let size = match &previous {
            Previous::Mem(b) => b.len() as u64,
            Previous::Disk(_, n) => *n,
            Previous::Tail { .. } | Previous::Lost => 0,
        };
        let mut g = self.lock();
        g.stacks.entry(path.to_path_buf()).or_default();
        if matches!(previous, Previous::Lost) {
            g.clear(path);
            return;
        }
        let store = match previous {
            Previous::Tail { before, after } => Store::Tail { before, after },
            Previous::Mem(bytes)
                if bytes.len() <= self.limits.mem_step && g.mem + bytes.len() <= self.limits.mem_total =>
            {
                Store::Mem(bytes)
            }
            other => {
                if size > self.limits.disk_total {
                    g.clear(path);
                    return;
                }
                // Hueco en el disco: se descarta lo más antiguo hasta que quepa.
                while g.disk + size > self.limits.disk_total {
                    if !g.evict_oldest() {
                        break;
                    }
                }
                match other {
                    Previous::Disk(tmp, _) => Store::Disk(tmp),
                    Previous::Mem(bytes) => match spill(path, &bytes) {
                        Ok(t) => Store::Disk(t),
                        Err(_) => {
                            g.clear(path);
                            return;
                        }
                    },
                    Previous::Lost | Previous::Tail { .. } => unreachable!("tratado arriba"),
                }
            }
        };
        g.seq += 1;
        let step = Step {
            seq: g.seq,
            size,
            store,
        };
        match step.store {
            Store::Mem(_) => g.mem += size as usize,
            Store::Disk(_) => g.disk += size,
            Store::Tail { .. } => {}
        }
        let stack = g.stacks.get_mut(path).expect("la pila existe");
        stack.push_back(step);
        let mut over = Vec::new();
        while stack.len() > self.limits.max_steps {
            over.extend(stack.pop_front());
        }
        for s in &over {
            g.forget(s);
        }
    }

    /// Quita y devuelve el último contenido anotado de `path`.
    pub fn pop(&self, path: &Path) -> Result<Option<Popped>> {
        let mut g = self.lock();
        let Some(step) = g.stacks.get_mut(path).and_then(VecDeque::pop_back) else {
            return Ok(None);
        };
        g.forget(&step);
        Ok(Some(match step.store {
            Store::Mem(bytes) => Popped::Mem(bytes),
            Store::Disk(tmp) => Popped::Disk(tmp),
            Store::Tail { before, after } => Popped::Tail { before, after },
        }))
    }

    /// Memoria y disco que ocupa el historial: `(bytes en memoria, bytes en temporales)`.
    pub fn usage(&self) -> (usize, u64) {
        let g = self.lock();
        (g.mem, g.disk)
    }
}

/// Copia `from` a un temporal oculto junto a él, de archivo a archivo.
fn spill_copy(from: &Path) -> std::io::Result<tempfile::TempPath> {
    let tmp = tempfile::Builder::new()
        .prefix(".pliego-deshacer-")
        .suffix(".tmp")
        .tempfile_in(
            from.parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new(".")),
        )?
        .into_temp_path();
    std::fs::copy(from, &tmp)?;
    Ok(tmp)
}

/// Escribe `bytes` en un temporal oculto junto a `doc` (o en el temporal del sistema si esa carpeta no admite escritura).
fn spill(doc: &Path, bytes: &[u8]) -> std::io::Result<tempfile::TempPath> {
    let write = |dir: Option<&Path>| -> std::io::Result<tempfile::TempPath> {
        let mut b = tempfile::Builder::new();
        b.prefix(".pliego-deshacer-").suffix(".tmp");
        let mut f = match dir {
            Some(d) => b.tempfile_in(d)?,
            None => b.tempfile()?,
        };
        f.write_all(bytes)?;
        f.flush()?;
        Ok(f.into_temp_path())
    };
    let beside = doc.parent().filter(|p| !p.as_os_str().is_empty());
    write(beside).or_else(|_| write(None))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes_of(p: Popped) -> Vec<u8> {
        match p {
            Popped::Mem(b) => b,
            Popped::Disk(t) => std::fs::read(&t).unwrap(),
            Popped::Tail { .. } => unreachable!("estas pruebas no apilan cortes"),
        }
    }

    fn limits() -> Limits {
        Limits {
            max_steps: 3,
            mem_step: 10,
            mem_total: 25,
            disk_total: 100,
        }
    }

    fn doc(dir: &tempfile::TempDir) -> PathBuf {
        dir.path().join("a.pdf")
    }

    #[test]
    fn small_steps_stay_in_memory_and_come_back_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let (h, p) = (History::new(limits()), doc(&dir));
        assert!(!h.tracks(&p));
        h.push(&p, Previous::Mem(vec![1; 5]));
        h.push(&p, Previous::Mem(vec![2; 5]));
        assert_eq!((h.depth(&p), h.usage()), (2, (10, 0)));
        assert_eq!(bytes_of(h.pop(&p).unwrap().unwrap()), vec![2; 5]);
        assert_eq!(bytes_of(h.pop(&p).unwrap().unwrap()), vec![1; 5]);
        assert!(h.pop(&p).unwrap().is_none());
        assert!(h.tracks(&p), "la clave queda aunque la pila se vacíe");
        assert_eq!(h.usage(), (0, 0));
    }

    #[test]
    fn big_steps_go_to_disk_beside_the_document_and_are_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let (h, p) = (History::new(limits()), doc(&dir));
        h.push(&p, Previous::Mem(vec![7; 40]));
        assert_eq!(h.usage(), (0, 40));
        let spilled = std::fs::read_dir(dir.path()).unwrap().count();
        assert_eq!(spilled, 1, "un temporal junto al documento");
        assert_eq!(bytes_of(h.pop(&p).unwrap().unwrap()), vec![7; 40]);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
        assert_eq!(h.usage(), (0, 0));
    }

    #[test]
    fn memory_budget_overflows_to_disk() {
        let dir = tempfile::tempdir().unwrap();
        let (h, p) = (History::new(limits()), doc(&dir));
        for i in 0..3 {
            h.push(&p, Previous::Mem(vec![i; 10]));
        }
        // 10 + 10 caben (20 ≤ 25); el tercero se pasa de 25 y va a disco.
        assert_eq!(h.usage(), (20, 10));
        assert_eq!(bytes_of(h.pop(&p).unwrap().unwrap()), vec![2; 10]);
    }

    #[test]
    fn max_steps_drops_the_oldest_and_frees_its_budget() {
        let dir = tempfile::tempdir().unwrap();
        let (h, p) = (History::new(limits()), doc(&dir));
        for i in 0..5 {
            h.push(&p, Previous::Mem(vec![i; 4]));
        }
        assert_eq!(h.depth(&p), 3);
        assert_eq!(h.usage(), (12, 0));
        assert_eq!(bytes_of(h.pop(&p).unwrap().unwrap()), vec![4; 4]);
        assert_eq!(bytes_of(h.pop(&p).unwrap().unwrap()), vec![3; 4]);
        assert_eq!(bytes_of(h.pop(&p).unwrap().unwrap()), vec![2; 4]);
    }

    #[test]
    fn disk_budget_evicts_the_oldest_step_of_any_document() {
        let dir = tempfile::tempdir().unwrap();
        let (h, a, b) = (
            History::new(limits()),
            dir.path().join("a.pdf"),
            dir.path().join("b.pdf"),
        );
        h.push(&a, Previous::Mem(vec![1; 60]));
        h.push(&b, Previous::Mem(vec![2; 30]));
        // 60 + 30 + 50 > 100: se descarta lo más antiguo (el de `a`).
        h.push(&b, Previous::Mem(vec![3; 50]));
        assert_eq!((h.depth(&a), h.depth(&b), h.usage().1), (0, 2, 80));
        assert_eq!(bytes_of(h.pop(&b).unwrap().unwrap()), vec![3; 50]);
    }

    #[test]
    fn capture_copies_big_files_on_disk_and_reads_small_ones() {
        let dir = tempfile::tempdir().unwrap();
        let (h, p) = (History::new(limits()), doc(&dir));
        std::fs::write(&p, vec![5u8; 8]).unwrap();
        assert!(matches!(h.capture(&p).unwrap(), Previous::Mem(b) if b == vec![5u8; 8]));
        std::fs::write(&p, vec![6u8; 50]).unwrap();
        let big = h.capture(&p).unwrap();
        assert!(matches!(big, Previous::Disk(_, 50)));
        h.push(&p, big);
        // El original puede cambiar después: el paso conserva lo de antes.
        std::fs::write(&p, b"nuevo").unwrap();
        assert_eq!(bytes_of(h.pop(&p).unwrap().unwrap()), vec![6u8; 50]);
        assert_eq!(
            std::fs::read_dir(dir.path()).unwrap().count(),
            1,
            "solo queda el documento"
        );
    }

    #[test]
    fn a_lost_step_clears_the_history_instead_of_skipping_a_version() {
        let dir = tempfile::tempdir().unwrap();
        let (h, p) = (History::new(limits()), doc(&dir));
        h.push(&p, Previous::Mem(vec![1; 5]));
        h.push(&p, Previous::Lost);
        assert_eq!((h.depth(&p), h.tracks(&p)), (0, true));
    }

    #[test]
    fn a_step_that_cannot_fit_clears_that_documents_history() {
        let dir = tempfile::tempdir().unwrap();
        let (h, p) = (History::new(limits()), doc(&dir));
        h.push(&p, Previous::Mem(vec![1; 5]));
        h.push(&p, Previous::Mem(vec![2; 500])); // más que todo el presupuesto de disco
        assert_eq!(h.depth(&p), 0, "no se puede deshacer saltándose una versión");
        assert!(h.tracks(&p));
        assert_eq!(h.usage(), (0, 0));
    }

    #[test]
    fn tail_steps_cost_nothing_and_come_back_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let (h, p) = (History::new(limits()), doc(&dir));
        h.push(&p, Previous::Tail { before: 10, after: 15 });
        h.push(&p, Previous::Tail { before: 15, after: 21 });
        assert_eq!((h.depth(&p), h.usage()), (2, (0, 0)));
        assert!(matches!(
            h.pop(&p).unwrap(),
            Some(Popped::Tail { before: 15, after: 21 })
        ));
        assert!(matches!(
            h.pop(&p).unwrap(),
            Some(Popped::Tail { before: 10, after: 15 })
        ));
        assert!(h.pop(&p).unwrap().is_none());
    }
}
