//! Fuentes del sistema para MuPDF sin `font-kit`.
//!
//! La característica `system-fonts` de `mupdf` consulta `font-kit` en cada búsqueda de fuente y, en Windows,
//! enumera todas las fuentes instaladas cada vez (~1,3 s por documento). Aquí se sustituye por un índice
//! perezoso y único por proceso: se lee una vez la cabecera de cada archivo (solo las tablas `name`, `OS/2` y
//! `head`), se guardan nombre PostScript, familia, estilo, ruta e índice de colección, y las fuentes que se piden
//! se cargan desde una caché de bytes. Fuera de Windows no se registra nada.

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Once, OnceLock};

use mupdf::{CjkFontOrdering, Font, FontHints, FontLoader};

/// Tamaño máximo que se acepta para una tabla `name` (las reales miden decenas de KB).
const MAX_NAME_TABLE: usize = 1 << 20;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    /// Nombre PostScript normalizado (minúsculas y solo alfanuméricos).
    postscript: String,
    /// Familia normalizada.
    family: String,
    /// Nombre PostScript tal cual, para dar nombre a la fuente de MuPDF.
    display: String,
    bold: bool,
    italic: bool,
    path: PathBuf,
    /// Índice dentro de una colección (.ttc/.otc); 0 en el resto.
    index: u32,
}

fn norm(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn be16(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(b.get(at..at + 2)?.try_into().ok()?))
}

fn be32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// Cadena `id` de la tabla `name`, prefiriendo Windows (UTF-16BE), luego Unicode y por último Macintosh.
fn name_string(table: &[u8], id: u16) -> Option<String> {
    let count = be16(table, 2)? as usize;
    let base = be16(table, 4)? as usize;
    let mut best: Option<(u8, String)> = None;
    for i in 0..count {
        let r = 6 + i * 12;
        if be16(table, r + 6)? != id {
            continue;
        }
        let (len, off) = (be16(table, r + 8)? as usize, be16(table, r + 10)? as usize);
        let raw = table.get(base + off..base + off + len)?;
        let (rank, text) = match be16(table, r)? {
            3 | 0 => (
                if be16(table, r)? == 3 { 0 } else { 1 },
                String::from_utf16_lossy(
                    &raw.chunks_exact(2)
                        .map(|c| u16::from_be_bytes([c[0], c[1]]))
                        .collect::<Vec<_>>(),
                ),
            ),
            1 => (2, raw.iter().map(|&b| b as char).collect()),
            _ => continue,
        };
        if best.as_ref().is_none_or(|(r, _)| rank < *r) {
            best = Some((rank, text));
        }
    }
    best.map(|(_, s)| s).filter(|s| !s.is_empty())
}

fn read_at(f: &mut File, at: u64, len: usize) -> Option<Vec<u8>> {
    f.seek(SeekFrom::Start(at)).ok()?;
    let mut buf = vec![0u8; len];
    f.read_exact(&mut buf).ok()?;
    Some(buf)
}

/// Lee una fuente sfnt que empieza en `offset`. `None` si no es TrueType/OpenType o no tiene nombres.
fn read_face(f: &mut File, offset: u64, path: &Path, index: u32) -> Option<Entry> {
    let head = read_at(f, offset, 12)?;
    if !matches!(be32(&head, 0)?, 0x0001_0000 | 0x4F54_544F | 0x7472_7565) {
        return None; // ni TrueType, ni "OTTO", ni "true"
    }
    let n = be16(&head, 4)? as usize;
    let dir = read_at(f, offset + 12, n * 16)?;
    let table = |tag: &[u8; 4]| -> Option<(u64, usize)> {
        (0..n).find_map(|i| {
            let r = i * 16;
            (dir.get(r..r + 4)? == tag).then(|| Some((u64::from(be32(&dir, r + 8)?), be32(&dir, r + 12)? as usize)))?
        })
    };
    let (noff, nlen) = table(b"name")?;
    let names = read_at(f, noff, nlen.min(MAX_NAME_TABLE))?;
    let postscript = name_string(&names, 6)?;
    let family = name_string(&names, 16).or_else(|| name_string(&names, 1))?;

    let (mut bold, mut italic) = (false, false);
    if let Some(os2) = table(b"OS/2").and_then(|(o, l)| read_at(f, o, l.min(64))) {
        if let (Some(weight), Some(sel)) = (be16(&os2, 4), be16(&os2, 62)) {
            bold = weight >= 700 || sel & 0x20 != 0;
            italic = sel & 0x01 != 0;
        }
    } else if let Some(h) = table(b"head").and_then(|(o, _)| read_at(f, o, 46)) {
        let mac = be16(&h, 44).unwrap_or(0);
        (bold, italic) = (mac & 1 != 0, mac & 2 != 0);
    }
    Some(Entry {
        postscript: norm(&postscript),
        family: norm(&family),
        display: postscript,
        bold,
        italic,
        path: path.to_path_buf(),
        index,
    })
}

fn read_file(path: &Path, out: &mut Vec<Entry>) {
    let Ok(mut f) = File::open(path) else { return };
    let Some(magic) = read_at(&mut f, 0, 12) else { return };
    if be32(&magic, 0) == Some(0x7474_6366) {
        // "ttcf": colección con `numFonts` desplazamientos.
        let count = be32(&magic, 8).unwrap_or(0).min(256);
        let Some(offsets) = read_at(&mut f, 12, count as usize * 4) else {
            return;
        };
        for i in 0..count {
            if let Some(off) = be32(&offsets, i as usize * 4)
                && let Some(e) = read_face(&mut f, u64::from(off), path, i)
            {
                out.push(e);
            }
        }
    } else if let Some(e) = read_face(&mut f, 0, path, 0) {
        out.push(e);
    }
}

fn scan(dir: &Path, depth: u32, out: &mut Vec<Entry>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut paths: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
    paths.sort();
    for p in paths {
        if p.is_dir() {
            if depth < 4 {
                scan(&p, depth + 1, out);
            }
        } else if p
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "ttf" | "otf" | "ttc" | "otc"))
        {
            read_file(&p, out);
        }
    }
}

/// Carga fuentes por nombre desde un conjunto de carpetas. Indexa la primera vez que se le pide una fuente.
pub struct SystemFonts {
    dirs: Vec<PathBuf>,
    index: OnceLock<Vec<Entry>>,
    bytes: Mutex<HashMap<PathBuf, Arc<Vec<u8>>>>,
}

impl SystemFonts {
    pub fn new(dirs: Vec<PathBuf>) -> Self {
        Self {
            dirs,
            index: OnceLock::new(),
            bytes: Mutex::new(HashMap::new()),
        }
    }

    fn entries(&self) -> &[Entry] {
        self.index.get_or_init(|| {
            let mut out = Vec::new();
            for d in &self.dirs {
                scan(d, 0, &mut out);
            }
            out
        })
    }

    fn by_postscript(&self, name: &str) -> Option<&Entry> {
        let key = norm(name);
        self.entries().iter().find(|e| e.postscript == key)
    }

    /// La mejor entrada de la familia `name` para el estilo pedido. Con `exact`, el estilo debe coincidir.
    fn by_family(&self, name: &str, hints: FontHints, exact: bool) -> Option<&Entry> {
        let key = norm(name);
        let score = |e: &Entry| u8::from(e.bold == hints.bold) + u8::from(e.italic == hints.italic);
        let best = self
            .entries()
            .iter()
            .filter(|e| e.family == key)
            .max_by_key(|e| score(e))?;
        (!exact || score(best) == 2).then_some(best)
    }

    fn make(&self, e: &Entry) -> Option<Font> {
        let data = {
            let mut cache = self.bytes.lock().ok()?;
            match cache.get(&e.path) {
                Some(d) => Arc::clone(d),
                None => {
                    let d = Arc::new(std::fs::read(&e.path).ok()?);
                    cache.insert(e.path.clone(), Arc::clone(&d));
                    d
                }
            }
        };
        Font::from_bytes_with_index(&e.display, e.index as i32, &data).ok()
    }

    fn family_first(&self, families: &[&str], hints: FontHints) -> Option<Font> {
        families.iter().find_map(|f| {
            let e = self.by_family(f, hints, false)?;
            self.make(e)
        })
    }
}

impl FontLoader for SystemFonts {
    fn load_font(&self, name: &str, hints: FontHints) -> Option<Font> {
        let e = match self.by_postscript(name) {
            Some(e) => e,
            None => {
                let mut base = name;
                for suffix in ["MT", "PS", "IdentityH"] {
                    base = base.strip_suffix(suffix).unwrap_or(base);
                }
                self.by_family(base, hints, false)?
            }
        };
        if hints.needs_exact_metrics && ((hints.bold && !e.bold) || (hints.italic && !e.italic)) {
            return None;
        }
        self.make(e)
    }

    fn load_cjk_font(&self, _name: &str, ordering: CjkFontOrdering, serif: bool) -> Option<Font> {
        let names: &[&str] = match (serif, ordering) {
            (true, CjkFontOrdering::AdobeCns) => &["MingLiU"],
            (true, CjkFontOrdering::AdobeGb) => &["SimSun"],
            (true, CjkFontOrdering::AdobeJapan) => &["MS-Mincho"],
            (true, CjkFontOrdering::AdobeKorea) => &["Batang"],
            (false, CjkFontOrdering::AdobeCns) => &["DFKaiShu-SB-Estd-BF"],
            (false, CjkFontOrdering::AdobeGb) => &["KaiTi", "KaiTi_GB2312"],
            (false, CjkFontOrdering::AdobeJapan) => &["MS-Gothic"],
            (false, CjkFontOrdering::AdobeKorea) => &["Gulim"],
        };
        names.iter().find_map(|n| self.load_font(n, FontHints::default()))
    }

    fn load_fallback_font(&self, script: u32, _language: u32, hints: FontHints) -> Option<Font> {
        // Valores de `UCDN_SCRIPT_*` (mupdf/ucdn.h).
        let families: &[&str] = match script {
            35 => &["Microsoft YaHei", "Yu Gothic", "SimSun"], // Han
            32..=34 => &["Yu Gothic", "MS Gothic"],            // Hiragana, Katakana, Bopomofo
            24 => &["Malgun Gothic"],                          // Hangul
            19 | 20 | 30 => &["Leelawadee UI"],                // Thai, Lao, Khmer
            9..=18 | 21 => &["Nirmala UI"],                    // Escrituras índicas y Tibetano
            2..=8 | 23 | 25 => &["Segoe UI"],                  // Griego, cirílico, árabe, hebreo…
            _ => &["Segoe UI Symbol", "Segoe UI Emoji", "Segoe UI"],
        };
        self.family_first(families, hints)
    }
}

/// Carpetas de fuentes de Windows: las del sistema y las instaladas por el usuario.
fn windows_font_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let windir = std::env::var_os("WINDIR").or_else(|| std::env::var_os("SystemRoot"));
    dirs.push(PathBuf::from(windir.unwrap_or_else(|| "C:\\Windows".into())).join("Fonts"));
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        dirs.push(PathBuf::from(local).join("Microsoft").join("Windows").join("Fonts"));
    }
    dirs
}

/// Registra el cargador de fuentes del sistema. Es idempotente y barato; llamarlo antes de abrir documentos.
/// Fuera de Windows no hace nada.
pub fn init_fonts() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        if cfg!(windows) {
            mupdf::set_font_loader(SystemFonts::new(windows_font_dirs()));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn system_dirs() -> Vec<PathBuf> {
        ["/usr/share/fonts", "/usr/local/share/fonts"]
            .iter()
            .map(PathBuf::from)
            .collect()
    }

    #[test]
    fn norm_ignores_case_and_separators() {
        assert_eq!(norm("Times New Roman-Bold"), "timesnewromanbold");
    }

    #[test]
    fn name_table_prefers_windows_names() {
        // Formato 0, dos registros: Mac (1,0,0) "Mac" y Windows (3,1,0x409) "Win" en UTF-16BE, ambos nameID 6.
        let mut t = vec![0, 0, 0, 2, 0, 30];
        for (pid, eid, lang, len, off) in [(1u16, 0u16, 0u16, 3u16, 0u16), (3, 1, 0x409, 6, 3)] {
            for v in [pid, eid, lang, 6, len, off] {
                t.extend(v.to_be_bytes());
            }
        }
        t.extend(b"Mac");
        t.extend("Win".encode_utf16().flat_map(u16::to_be_bytes));
        assert_eq!(name_string(&t, 6).as_deref(), Some("Win"));
        assert_eq!(name_string(&t, 1), None);
    }

    #[test]
    fn indexes_and_loads_system_fonts_if_present() {
        let fonts = SystemFonts::new(system_dirs());
        let entries = fonts.entries();
        if entries.is_empty() {
            eprintln!("sin fuentes del sistema en este equipo: prueba omitida");
            return;
        }
        let e = &entries[0];
        let f = fonts.load_font(&e.display, FontHints::default());
        assert!(f.is_some(), "no se cargó {}", e.display);
        // Pedir una fuente inexistente no falla y es rápido porque el índice ya existe.
        let t = std::time::Instant::now();
        for _ in 0..1000 {
            assert!(fonts.load_font("NoExisteEstaFuente", FontHints::default()).is_none());
        }
        assert!(t.elapsed().as_millis() < 500, "{:?}", t.elapsed());
    }
}
