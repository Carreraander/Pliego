//! Documentos recientes: lista con los más nuevos primero, fijados arriba, sin duplicados y con un máximo.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub const MAX_RECENTS: usize = 50;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentEntry {
    pub path: String,
    /// Segundos desde 1970 de la última apertura.
    pub opened_at: u64,
    #[serde(default)]
    pub pinned: bool,
}

/// Lo que ve la interfaz: la entrada y el estado actual del archivo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecentView {
    #[serde(flatten)]
    pub entry: RecentEntry,
    pub exists: bool,
    pub size: Option<u64>,
    /// Carpeta que contiene el archivo.
    pub folder: String,
    pub name: String,
}

pub struct RecentStore {
    path: PathBuf,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Pone los fijados primero y, dentro de cada grupo, lo más reciente antes.
fn sort(list: &mut [RecentEntry]) {
    list.sort_by(|a, b| b.pinned.cmp(&a.pinned).then(b.opened_at.cmp(&a.opened_at)));
}

impl RecentStore {
    pub fn new(dir: &Path) -> Self {
        Self {
            path: dir.join("recents.json"),
        }
    }

    fn read(&self) -> Vec<RecentEntry> {
        std::fs::read_to_string(&self.path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    fn write(&self, list: &[RecentEntry]) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(list).map_err(|e| e.to_string())?;
        crate::write_atomic(&self.path, &bytes).map_err(|e| format!("No se pudo guardar la lista de recientes: {e}"))
    }

    /// Anota una apertura (sube el documento al principio y conserva si estaba fijado).
    pub fn add(&self, path: &str) -> Result<(), String> {
        self.add_at(path, now())
    }

    fn add_at(&self, path: &str, at: u64) -> Result<(), String> {
        let mut list = self.read();
        let pinned = list.iter().find(|e| e.path == path).is_some_and(|e| e.pinned);
        list.retain(|e| e.path != path);
        list.push(RecentEntry {
            path: path.to_owned(),
            opened_at: at,
            pinned,
        });
        sort(&mut list);
        // El máximo se aplica solo a los no fijados.
        let mut unpinned = 0;
        list.retain(|e| {
            e.pinned || {
                unpinned += 1;
                unpinned <= MAX_RECENTS
            }
        });
        self.write(&list)
    }

    pub fn remove(&self, path: &str) -> Result<(), String> {
        let mut list = self.read();
        list.retain(|e| e.path != path);
        self.write(&list)
    }

    pub fn set_pinned(&self, path: &str, pinned: bool) -> Result<(), String> {
        let mut list = self.read();
        if let Some(e) = list.iter_mut().find(|e| e.path == path) {
            e.pinned = pinned;
        }
        sort(&mut list);
        self.write(&list)
    }

    /// La lista para mostrar, con el estado de cada archivo (los desaparecidos siguen, atenuados, hasta quitarlos).
    pub fn list(&self) -> Vec<RecentView> {
        let mut list = self.read();
        sort(&mut list);
        list.into_iter()
            .map(|entry| {
                let p = Path::new(&entry.path);
                let meta = std::fs::metadata(p).ok();
                RecentView {
                    exists: meta.as_ref().is_some_and(|m| m.is_file()),
                    size: meta.map(|m| m.len()),
                    folder: p.parent().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default(),
                    name: p
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| entry.path.clone()),
                    entry,
                }
            })
            .collect()
    }
}

/// Orden para mostrar un archivo en el explorador del sistema (`explorer.exe /select,<ruta>` en Windows).
pub fn reveal_command(path: &str) -> (&'static str, Vec<String>) {
    if cfg!(windows) {
        ("explorer.exe", vec![format!("/select,{path}")])
    } else if cfg!(target_os = "macos") {
        ("open", vec!["-R".into(), path.into()])
    } else {
        let folder = Path::new(path)
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        ("xdg-open", vec![folder])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reveal_selects_the_file() {
        let (program, args) = reveal_command("C:\\Docs\\a.pdf");
        assert!(!program.is_empty() && !args.is_empty());
        if cfg!(windows) {
            assert_eq!(args[0], "/select,C:\\Docs\\a.pdf");
        }
    }

    #[test]
    fn newest_first_without_duplicates_and_pins_on_top() {
        let dir = tempfile::tempdir().unwrap();
        let s = RecentStore::new(dir.path());
        s.add_at("a.pdf", 1).unwrap();
        s.add_at("b.pdf", 2).unwrap();
        s.add_at("a.pdf", 3).unwrap();
        let names: Vec<_> = s.list().into_iter().map(|r| r.entry.path).collect();
        assert_eq!(names, ["a.pdf", "b.pdf"]);
        s.set_pinned("b.pdf", true).unwrap();
        s.add_at("a.pdf", 4).unwrap();
        let names: Vec<_> = s.list().into_iter().map(|r| r.entry.path).collect();
        assert_eq!(names, ["b.pdf", "a.pdf"]);
        s.add_at("b.pdf", 5).unwrap();
        assert!(s.list()[0].entry.pinned, "reabrir conserva el estado de fijado");
        s.remove("b.pdf").unwrap();
        assert_eq!(s.list().len(), 1);
    }

    #[test]
    fn missing_files_are_reported_and_limit_spares_pinned() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real.pdf");
        std::fs::write(&real, b"%PDF").unwrap();
        let s = RecentStore::new(dir.path());
        s.add_at(real.to_str().unwrap(), 1).unwrap();
        s.add_at("no-existe.pdf", 2).unwrap();
        let view = s.list();
        assert!(!view[0].exists && view[1].exists);
        assert_eq!(view[1].name, "real.pdf");
        assert_eq!(view[1].size, Some(4));
        s.set_pinned("no-existe.pdf", true).unwrap();
        for i in 0..(MAX_RECENTS as u64 + 5) {
            s.add_at(&format!("f{i}.pdf"), 10 + i).unwrap();
        }
        let list = s.list();
        assert_eq!(list.len(), MAX_RECENTS + 1);
        assert!(list[0].entry.pinned);
    }

    #[test]
    fn damaged_file_reads_as_empty() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("recents.json"), "basura").unwrap();
        assert!(RecentStore::new(dir.path()).list().is_empty());
    }
}
