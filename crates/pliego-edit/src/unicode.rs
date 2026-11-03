//! Texto fuera de WinAnsi (CJK, cirílico, griego, árabe...): fuente del sistema o indicada, forma con
//! rustybuzz y escritura con códigos de glifo (CID Identity-H).

use std::path::{Path, PathBuf};

use pliego_core::{Error, Result};
use rustybuzz::{Face, UnicodeBuffer};

/// Fuente cargada para componer texto Unicode.
pub struct LoadedFont {
    pub data: Vec<u8>,
    /// Nombre legible para los avisos (archivo de la fuente).
    pub label: String,
}

/// Fuentes candidatas, por orden de preferencia. Primero las de Windows pensadas para CJK.
fn candidates() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = Vec::new();
    if let Some(p) = std::env::var_os("PLIEGO_FALLBACK_FONT") {
        v.push(PathBuf::from(p));
    }
    // Descarga opcional de Noto Sans CJK (OFL-1.1) en la carpeta de datos de Pliego.
    for var in ["LOCALAPPDATA", "XDG_DATA_HOME"] {
        if let Some(base) = std::env::var_os(var) {
            let dir = PathBuf::from(base).join("Pliego").join("fonts");
            if let Ok(rd) = std::fs::read_dir(&dir) {
                let mut files: Vec<_> = rd.flatten().map(|e| e.path()).collect();
                files.sort();
                v.extend(files);
            }
        }
    }
    let win = std::env::var_os("WINDIR").map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
    for f in [
        "msyh.ttc",
        "msyh.ttf",
        "YuGothR.ttc",
        "YuGothM.ttc",
        "malgun.ttf",
        "simsun.ttc",
        "segoeui.ttf",
        "arial.ttf",
    ] {
        v.push(win.join("Fonts").join(f));
    }
    for f in [
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
        "/etc/alternatives/fonts-japanese-gothic.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/freefont/FreeSans.ttf",
        "/Library/Fonts/Arial Unicode.ttf",
    ] {
        v.push(PathBuf::from(f));
    }
    v
}

fn covers(data: &[u8], text: &str) -> Vec<char> {
    let Some(face) = Face::from_slice(data, 0) else {
        return text.chars().filter(|c| !c.is_whitespace()).collect();
    };
    let mut missing: Vec<char> = Vec::new();
    for c in text.chars().filter(|c| !c.is_control() && *c != '\n') {
        if face.glyph_index(c).is_none() && !missing.contains(&c) {
            missing.push(c);
        }
    }
    missing
}

/// Busca una fuente que contenga todos los caracteres de `text`. `explicit` tiene prioridad.
pub fn find_font(text: &str, explicit: Option<&Path>) -> Result<LoadedFont> {
    let mut list: Vec<PathBuf> = explicit.map(Path::to_path_buf).into_iter().collect();
    let explicit_only = explicit.is_some();
    if !explicit_only {
        list.extend(candidates());
    }
    let mut best_missing: Vec<char> = text.chars().collect();
    for path in list {
        let Ok(data) = std::fs::read(&path) else {
            if explicit_only {
                return Err(Error::InvalidInput(format!(
                    "no se pudo leer la fuente {}",
                    path.display()
                )));
            }
            continue;
        };
        let missing = covers(&data, text);
        if missing.is_empty() {
            let label = path
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            return Ok(LoadedFont { data, label });
        }
        if missing.len() < best_missing.len() {
            best_missing = missing;
        }
    }
    let list: String = best_missing.iter().take(5).collect();
    Err(Error::Unsupported(format!(
        "no hay ninguna fuente instalada con los caracteres «{list}». \
         Indica una fuente con --font-file o instala Noto Sans CJK desde los ajustes de Pliego"
    )))
}

/// Texto formado: glifos (con el texto del que proceden) y avance total en puntos.
pub struct Shaped {
    pub glyphs: Vec<(u16, String)>,
    pub width: f32,
}

pub struct Shaper<'a> {
    face: Face<'a>,
}

impl<'a> Shaper<'a> {
    pub fn new(data: &'a [u8]) -> Result<Self> {
        Face::from_slice(data, 0)
            .map(|face| Self { face })
            .ok_or_else(|| Error::InvalidInput("la fuente no se puede leer".into()))
    }

    pub fn shape(&self, text: &str, size: f32) -> Shaped {
        let mut buf = UnicodeBuffer::new();
        buf.push_str(text);
        buf.guess_segment_properties();
        let out = rustybuzz::shape(&self.face, &[], buf);
        let upem = (self.face.units_per_em() as f32).max(1.0);
        let infos = out.glyph_infos();
        let mut clusters: Vec<u32> = infos.iter().map(|g| g.cluster).collect();
        clusters.sort_unstable();
        clusters.dedup();
        let glyphs = infos
            .iter()
            .map(|g| {
                // El texto de un glifo va desde su cluster hasta el siguiente cluster mayor (vale para RTL).
                let start = g.cluster as usize;
                let end = clusters
                    .iter()
                    .find(|c| **c as usize > start)
                    .map_or(text.len(), |c| *c as usize);
                let t = text.get(start..end).unwrap_or("").to_owned();
                (g.glyph_id as u16, t)
            })
            .collect();
        let advance: i32 = out.glyph_positions().iter().map(|p| p.x_advance).sum();
        Shaped {
            glyphs,
            width: advance as f32 * size / upem,
        }
    }

    pub fn width(&self, text: &str, size: f32) -> f32 {
        self.shape(text, size).width
    }

    pub fn ascender(&self) -> f32 {
        f32::from(self.face.ascender()) / (self.face.units_per_em() as f32).max(1.0)
    }
}

/// ¿Es un carácter tras el que se puede partir la línea aunque no haya espacio (ideogramas, kana, hangul)?
pub fn is_wide(c: char) -> bool {
    matches!(c as u32,
        0x2E80..=0x303F | 0x3040..=0x30FF | 0x3100..=0x312F | 0x3130..=0x318F | 0x31F0..=0x31FF
        | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xAC00..=0xD7AF | 0xF900..=0xFAFF | 0xFF00..=0xFFEF
        | 0x20000..=0x2FA1F)
}
