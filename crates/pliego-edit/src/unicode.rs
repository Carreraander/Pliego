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

/// Fuentes candidatas, por orden de preferencia. Primero las de Windows pensadas para CJK; las de símbolos y
/// emojis van después, para completar lo que las demás no tienen.
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
        // Símbolos y emojis (se dibujan en monocromo: el subconjunto no lleva la capa de color).
        "seguisym.ttf",
        "seguiemj.ttf",
        "seguihis.ttf",
        "l_10646.ttf",
    ] {
        v.push(win.join("Fonts").join(f));
    }
    for f in [
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
        "/etc/alternatives/fonts-japanese-gothic.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/freefont/FreeSans.ttf",
        "/usr/share/fonts/truetype/noto/NotoSansSymbols-Regular.ttf",
        "/usr/share/fonts/truetype/noto/NotoSansSymbols2-Regular.ttf",
        "/usr/share/fonts/truetype/noto/NotoEmoji-Regular.ttf",
        "/Library/Fonts/Arial Unicode.ttf",
        "/System/Library/Fonts/Apple Symbols.ttf",
    ] {
        v.push(PathBuf::from(f));
    }
    v
}

/// Caracteres que no necesitan glifo propio: controles, espacios, selectores de variación y uniones de ancho cero.
fn ignorable(c: char) -> bool {
    c.is_control()
        || c.is_whitespace()
        || matches!(c as u32, 0x200B..=0x200F | 0x2060 | 0xFE00..=0xFE0F | 0xFEFF | 0xE0100..=0xE01EF)
}

/// Caracteres distintos de `text` que necesitan glifo, por orden de aparición.
fn needed_chars(text: &str) -> Vec<char> {
    let mut v: Vec<char> = Vec::new();
    for c in text.chars().filter(|c| !ignorable(*c)) {
        if !v.contains(&c) {
            v.push(c);
        }
    }
    v
}

fn coverage(data: &[u8], needed: &[char]) -> Vec<bool> {
    match Face::from_slice(data, 0) {
        Some(face) => needed.iter().map(|c| face.glyph_index(*c).is_some()).collect(),
        None => vec![false; needed.len()],
    }
}

fn label_of(path: &Path) -> String {
    path.file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
}

/// Fuentes elegidas para componer un texto: la primera es la principal y las demás completan los caracteres
/// que esa no tiene (símbolos, emojis, otro alfabeto).
pub struct FontSet {
    pub fonts: Vec<LoadedFont>,
    /// Caracteres que ninguna fuente del equipo tiene: se dibujan con el recuadro vacío de la fuente principal.
    pub missing: Vec<char>,
}

/// Busca las fuentes con las que componer `text`. Si una sola tiene todos los caracteres, se usa esa; si no, se
/// combinan varias (primero la que cubre más). Nunca falla por faltar caracteres: los que ninguna fuente tiene se
/// devuelven en `missing`. `explicit` tiene prioridad y es la fuente principal.
pub fn find_fonts(text: &str, explicit: Option<&Path>) -> Result<FontSet> {
    let needed = needed_chars(text);
    let mut paths: Vec<PathBuf> = explicit.map(Path::to_path_buf).into_iter().collect();
    paths.extend(candidates());
    // Cobertura de cada fuente legible; los datos no se guardan para no retener decenas de MB.
    let mut cover: Vec<(PathBuf, Vec<bool>)> = Vec::new();
    for (i, path) in paths.into_iter().enumerate() {
        let Ok(data) = std::fs::read(&path) else {
            if i == 0 && explicit.is_some() {
                return Err(Error::InvalidInput(format!(
                    "no se pudo leer la fuente {}",
                    path.display()
                )));
            }
            continue;
        };
        let cov = coverage(&data, &needed);
        if cov.iter().all(|c| *c) {
            return Ok(FontSet {
                fonts: vec![LoadedFont {
                    label: label_of(&path),
                    data,
                }],
                missing: Vec::new(),
            });
        }
        cover.push((path, cov));
    }
    if cover.is_empty() {
        return Err(Error::Unsupported(
            "no hay ninguna fuente instalada para componer este texto. \
             Indica una fuente con --font-file o instala Noto Sans CJK desde los ajustes de Pliego"
                .into(),
        ));
    }
    let mut covered = vec![false; needed.len()];
    let mut chosen: Vec<usize> = Vec::new();
    if explicit.is_some() {
        chosen.push(0);
        for (k, c) in cover[0].1.iter().enumerate() {
            covered[k] |= *c;
        }
    }
    loop {
        let mut best: Option<(usize, usize)> = None;
        for (i, (_, cov)) in cover.iter().enumerate().filter(|(i, _)| !chosen.contains(i)) {
            let n = cov.iter().zip(&covered).filter(|(c, done)| **c && !**done).count();
            if n > 0 && best.is_none_or(|(_, b)| n > b) {
                best = Some((i, n));
            }
        }
        let Some((i, _)) = best else { break };
        for (k, c) in cover[i].1.iter().enumerate() {
            covered[k] |= *c;
        }
        chosen.push(i);
    }
    if chosen.is_empty() {
        // Ninguna fuente tiene ninguno de los caracteres: se usa la primera para dibujar el recuadro vacío.
        chosen.push(0);
    }
    let fonts = chosen
        .iter()
        .map(|i| {
            let path = &cover[*i].0;
            std::fs::read(path)
                .map(|data| LoadedFont {
                    data,
                    label: label_of(path),
                })
                .map_err(|e| Error::InvalidInput(format!("no se pudo leer la fuente {}: {e}", path.display())))
        })
        .collect::<Result<Vec<_>>>()?;
    let missing = needed
        .iter()
        .zip(&covered)
        .filter(|(_, done)| !**done)
        .map(|(c, _)| *c)
        .collect();
    Ok(FontSet { fonts, missing })
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

    /// ¿La fuente tiene glifo para `c`?
    pub fn covers(&self, c: char) -> bool {
        self.face.glyph_index(c).is_some()
    }

    pub fn ascender(&self) -> f32 {
        f32::from(self.face.ascender()) / (self.face.units_per_em() as f32).max(1.0)
    }
}

/// Pila de fuentes: cada carácter se dibuja con la primera que lo tiene, prefiriendo seguir con la actual.
pub struct Stack<'a> {
    pub shapers: Vec<Shaper<'a>>,
}

impl Stack<'_> {
    /// Parte `text` en trozos seguidos, cada uno con el índice de la fuente que lo dibuja. Los espacios y los
    /// caracteres sin glifo propio (selector de variación, unión de ancho cero) siguen con la fuente anterior.
    pub fn runs<'t>(&self, text: &'t str) -> Vec<(usize, &'t str)> {
        let first_with = |c: char| self.shapers.iter().position(|s| s.covers(c));
        let mut out: Vec<(usize, &str)> = Vec::new();
        let (mut cur, mut start): (Option<usize>, usize) = (None, 0);
        for (i, c) in text.char_indices() {
            let want = match cur {
                Some(f) if self.shapers[f].covers(c) || (ignorable(c) && !c.is_whitespace()) => f,
                _ => first_with(c).or(cur).unwrap_or(0),
            };
            if cur != Some(want) {
                if let Some(f) = cur {
                    out.push((f, &text[start..i]));
                }
                cur = Some(want);
                start = i;
            }
        }
        if let Some(f) = cur {
            out.push((f, &text[start..]));
        }
        out
    }
}

/// ¿Es un carácter tras el que se puede partir la línea aunque no haya espacio (ideogramas, kana, hangul)?
pub fn is_wide(c: char) -> bool {
    matches!(c as u32,
        0x2E80..=0x303F | 0x3040..=0x30FF | 0x3100..=0x312F | 0x3130..=0x318F | 0x31F0..=0x31FF
        | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xAC00..=0xD7AF | 0xF900..=0xFAFF | 0xFF00..=0xFFEF
        | 0x20000..=0x2FA1F)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignorable_chars_do_not_need_a_glyph() {
        assert_eq!(needed_chars("a b\u{FE0F}\u{200D}\nc"), vec!['a', 'b', 'c']);
    }

    #[test]
    fn explicit_unreadable_font_is_an_error() {
        let e = find_fonts("hola", Some(Path::new("no-existe.ttf"))).err().unwrap();
        assert!(e.to_string().contains("no-existe.ttf"), "{e}");
    }

    /// Si hay una fuente del sistema con emojis, el texto mezclado se reparte en trozos por fuente.
    #[test]
    fn mixed_text_is_split_by_font() {
        let Ok(set) = find_fonts("Hola ✅ mundo", None) else {
            return;
        };
        let stack = Stack {
            shapers: set.fonts.iter().map(|f| Shaper::new(&f.data).unwrap()).collect(),
        };
        let runs = stack.runs("Hola ✅ mundo");
        let joined: String = runs.iter().map(|(_, t)| *t).collect();
        assert_eq!(joined, "Hola ✅ mundo");
        assert!(runs.iter().all(|(f, _)| *f < set.fonts.len()));
    }
}
