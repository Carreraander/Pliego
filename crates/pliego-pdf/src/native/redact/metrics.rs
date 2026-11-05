//! Métricas de fuentes para medir dónde cae cada glifo: partir una cadena en códigos y dar el avance de cada uno.
//!
//! Regla de seguridad: si no se puede medir con certeza (CMap predefinido sin tablas, escritura vertical, fuente
//! estándar sin anchos conocidos…), la fuente es *opaca* y el redactor trata la operación de texto entera como
//! sospechosa (la quita si su caja posible toca la zona). Nunca se adivina un ancho.

use std::collections::HashMap;

use lopdf::{Dictionary, Object as L};

use super::super::State;
use super::std14;

/// Un código de la cadena mostrada.
#[derive(Debug, Clone, Copy)]
pub(super) struct Glyph {
    pub(super) start: usize,
    pub(super) len: usize,
    /// Avance en em (1.0 = tamaño de la fuente). `None` = desconocido.
    pub(super) width: Option<f64>,
    /// Código de un byte igual a 32 (recibe el espaciado entre palabras `Tw`).
    pub(super) space: bool,
}

#[derive(Debug)]
enum Kind {
    Simple {
        widths: Box<[Option<f64>; 256]>,
    },
    Cid {
        code_len: usize,
        /// `None` = identidad (CID = código).
        map: Option<CidMap>,
        dw: f64,
        w: HashMap<u32, f64>,
    },
    Opaque,
}

#[derive(Debug, Default)]
struct CidMap {
    /// (primero, último, cid del primero)
    ranges: Vec<(u32, u32, u32)>,
}

impl CidMap {
    fn cid(&self, code: u32) -> u32 {
        self.ranges
            .iter()
            .find(|(a, b, _)| (*a..=*b).contains(&code))
            .map_or(0, |(a, _, c)| c + (code - a))
    }
}

#[derive(Debug)]
pub(super) struct FontInfo {
    kind: Kind,
    pub(super) ascent: f64,
    pub(super) descent: f64,
}

fn deref<'a>(st: &'a State, o: &'a L) -> Option<&'a L> {
    st.deref(o)
}

fn number(st: &State, o: &L) -> Option<f64> {
    match deref(st, o)? {
        L::Integer(i) => Some(*i as f64),
        L::Real(r) => Some(f64::from(*r)),
        _ => None,
    }
}

fn dict_get<'a>(st: &'a State, d: &'a Dictionary, key: &[u8]) -> Option<&'a L> {
    d.get(key).ok().and_then(|o| deref(st, o))
}

fn name_of(o: &L) -> Option<&[u8]> {
    match o {
        L::Name(n) => Some(n),
        _ => None,
    }
}

impl FontInfo {
    /// Fuente que no se sabe medir.
    pub(super) fn opaque() -> FontInfo {
        FontInfo {
            kind: Kind::Opaque,
            ascent: 0.9,
            descent: -0.25,
        }
    }

    pub(super) fn is_opaque(&self) -> bool {
        matches!(self.kind, Kind::Opaque)
    }

    pub(super) fn load(st: &State, font: &L) -> FontInfo {
        let Some(d) = st.dict_of(font) else {
            return FontInfo::opaque();
        };
        let subtype = dict_get(st, d, b"Subtype").and_then(name_of).unwrap_or(b"");
        let (mut ascent, mut descent) = (0.9, -0.25);
        fn desc_of<'s>(st: &'s State, dd: &'s Dictionary) -> Option<&'s Dictionary> {
            dict_get(st, dd, b"FontDescriptor").and_then(|o| st.dict_of(o))
        }
        let mut read_ad = |dd: &Dictionary| {
            if let Some(fd) = desc_of(st, dd) {
                if let Some(a) = fd.get(b"Ascent").ok().and_then(|o| number(st, o))
                    && a > 0.0
                {
                    ascent = (a / 1000.0).clamp(0.7, 1.2);
                }
                if let Some(de) = fd.get(b"Descent").ok().and_then(|o| number(st, o))
                    && de < 0.0
                {
                    descent = (de / 1000.0).clamp(-0.5, -0.15);
                }
            }
        };
        read_ad(d);
        if subtype == b"Type0" {
            let desc = dict_get(st, d, b"DescendantFonts")
                .and_then(|o| match o {
                    L::Array(a) => a.first(),
                    _ => None,
                })
                .and_then(|o| st.dict_of(o));
            let Some(desc) = desc else {
                return FontInfo::opaque();
            };
            read_ad(desc);
            let kind = Self::cid_kind(st, d, desc);
            return FontInfo { kind, ascent, descent };
        }
        let kind = Self::simple_kind(st, d, subtype);
        FontInfo { kind, ascent, descent }
    }

    fn cid_kind(st: &State, font: &Dictionary, desc: &Dictionary) -> Kind {
        let enc = dict_get(st, font, b"Encoding");
        let (code_len, map) = match enc {
            Some(L::Name(n)) if n == b"Identity-H" => (2, None),
            Some(L::Stream(s)) => {
                let Ok(data) = super::super::object::decode_stream(s) else {
                    return Kind::Opaque;
                };
                match parse_cmap(&data) {
                    Some((len, map)) => (len, Some(map)),
                    None => return Kind::Opaque,
                }
            }
            _ => return Kind::Opaque,
        };
        let dw = desc.get(b"DW").ok().and_then(|o| number(st, o)).unwrap_or(1000.0);
        let mut w = HashMap::new();
        if let Some(L::Array(arr)) = dict_get(st, desc, b"W") {
            let items: Vec<&L> = arr.iter().filter_map(|o| deref(st, o)).collect();
            let mut i = 0;
            while i < items.len() {
                let Some(c) = number(st, items[i]) else { break };
                match items.get(i + 1) {
                    Some(L::Array(ws)) => {
                        for (k, wv) in ws.iter().enumerate() {
                            if let Some(wv) = number(st, wv) {
                                w.insert(c as u32 + k as u32, wv / 1000.0);
                            }
                        }
                        i += 2;
                    }
                    Some(last) => {
                        let (Some(last), Some(wv)) = (number(st, last), items.get(i + 2).and_then(|o| number(st, o)))
                        else {
                            break;
                        };
                        let (a, b) = (c as u32, last as u32);
                        if b >= a && b - a < 70_000 {
                            for cid in a..=b {
                                w.insert(cid, wv / 1000.0);
                            }
                        }
                        i += 3;
                    }
                    None => break,
                }
            }
        }
        Kind::Cid {
            code_len,
            map,
            dw: dw / 1000.0,
            w,
        }
    }

    fn simple_kind(st: &State, d: &Dictionary, subtype: &[u8]) -> Kind {
        let mut widths: Box<[Option<f64>; 256]> = Box::new([None; 256]);
        let first = d
            .get(b"FirstChar")
            .ok()
            .and_then(|o| number(st, o))
            .map_or(0, |v| v as i64);
        let scale = if subtype == b"Type3" {
            match dict_get(st, d, b"FontMatrix") {
                Some(L::Array(m)) => m.first().and_then(|o| number(st, o)).unwrap_or(0.001),
                _ => 0.001,
            }
        } else {
            0.001
        };
        let fd = dict_get(st, d, b"FontDescriptor").and_then(|o| st.dict_of(o));
        if let Some(L::Array(ws)) = dict_get(st, d, b"Widths") {
            let missing = fd
                .and_then(|f| f.get(b"MissingWidth").ok())
                .and_then(|o| number(st, o))
                .unwrap_or(0.0);
            for c in 0..256i64 {
                let idx = c - first;
                widths[c as usize] = if idx >= 0 && (idx as usize) < ws.len() {
                    number(st, &ws[idx as usize]).map(|w| w * scale)
                } else {
                    Some(missing * scale)
                };
            }
            return Kind::Simple { widths };
        }
        if subtype == b"Type3" {
            return Kind::Opaque;
        }
        // Sin /Widths: solo los 14 fuentes estándar.
        let base = dict_get(st, d, b"BaseFont").and_then(name_of).unwrap_or(b"");
        let base = String::from_utf8_lossy(base).replace(' ', "");
        let table = standard_table(&base);
        let Some(table) = table else {
            return Kind::Opaque;
        };
        // Qué códigos tienen significado seguro con la codificación declarada.
        let enc = dict_get(st, d, b"Encoding");
        let mut winansi = false;
        let mut differences: Vec<u8> = Vec::new();
        match enc {
            Some(L::Name(n)) => winansi = n == b"WinAnsiEncoding",
            Some(L::Dictionary(ed)) => {
                winansi = dict_get(st, ed, b"BaseEncoding")
                    .and_then(name_of)
                    .is_some_and(|n| n == b"WinAnsiEncoding");
                if let Some(L::Array(diff)) = dict_get(st, ed, b"Differences") {
                    let mut code: i64 = 0;
                    for item in diff {
                        match deref(st, item) {
                            Some(L::Integer(i)) => code = *i,
                            Some(L::Name(_)) => {
                                if (0..256).contains(&code) {
                                    differences.push(code as u8);
                                }
                                code += 1;
                            }
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
        for c in 0..256usize {
            if differences.contains(&(c as u8)) {
                continue;
            }
            let w = match table {
                Table::Fixed(w) => {
                    if (32..127).contains(&c) || (winansi && c >= 160) {
                        Some(w)
                    } else {
                        None
                    }
                }
                Table::Widths(t) => {
                    if c < 32 || c == 127 {
                        None
                    } else if (32..127).contains(&c) || (winansi && c >= 128) {
                        let v = t[c - 32];
                        (v != std14::UNKNOWN).then(|| f64::from(v) / 1000.0)
                    } else {
                        None
                    }
                }
            };
            widths[c] = w;
        }
        Kind::Simple { widths }
    }

    /// Parte `s` en códigos con su ancho. `None` si la fuente es opaca.
    pub(super) fn decode(&self, s: &[u8]) -> Option<Vec<Glyph>> {
        match &self.kind {
            Kind::Opaque => None,
            Kind::Simple { widths } => Some(
                s.iter()
                    .enumerate()
                    .map(|(i, &b)| Glyph {
                        start: i,
                        len: 1,
                        width: widths[b as usize],
                        space: b == 32,
                    })
                    .collect(),
            ),
            Kind::Cid { code_len, map, dw, w } => {
                let mut out = Vec::new();
                let mut i = 0;
                while i < s.len() {
                    let len = (*code_len).min(s.len() - i);
                    let code = s[i..i + len].iter().fold(0u32, |a, b| (a << 8) | u32::from(*b));
                    let cid = map.as_ref().map_or(code, |m| m.cid(code));
                    out.push(Glyph {
                        start: i,
                        len,
                        width: Some(w.get(&cid).copied().unwrap_or(*dw)),
                        space: *code_len == 1 && code == 32,
                    });
                    i += len;
                }
                Some(out)
            }
        }
    }
}

enum Table {
    Fixed(f64),
    Widths(&'static [u16; 224]),
}

fn standard_table(base: &str) -> Option<Table> {
    let b = base.trim_start_matches('/');
    let lower = b.to_ascii_lowercase();
    let bold = lower.contains("bold");
    let italic = lower.contains("italic") || lower.contains("oblique");
    if lower.starts_with("courier") {
        return Some(Table::Fixed(0.6));
    }
    if lower.starts_with("helvetica") || lower == "arial" || lower.starts_with("arial,") || lower.starts_with("arial-")
    {
        return Some(Table::Widths(if bold {
            &std14::HELVETICA_BOLD
        } else {
            &std14::HELVETICA
        }));
    }
    if lower.starts_with("times") {
        return Some(Table::Widths(match (bold, italic) {
            (false, false) => &std14::TIMES_ROMAN,
            (true, false) => &std14::TIMES_BOLD,
            (false, true) => &std14::TIMES_ITALIC,
            (true, true) => &std14::TIMES_BOLD_ITALIC,
        }));
    }
    None
}

/// Lee un CMap incrustado: longitud única de código y rangos código → CID. `None` si usa otro CMap (`usecmap`),
/// mezcla longitudes o es de escritura vertical.
fn parse_cmap(data: &[u8]) -> Option<(usize, CidMap)> {
    let text = String::from_utf8_lossy(data);
    if text.contains("usecmap") || text.contains("/WMode 1") {
        return None;
    }
    let hex = |t: &str| -> Option<(u32, usize)> {
        let t = t.trim().strip_prefix('<')?.strip_suffix('>')?;
        let t: String = t.chars().filter(|c| !c.is_whitespace()).collect();
        Some((u32::from_str_radix(&t, 16).ok()?, t.len().div_ceil(2)))
    };
    let mut lens: Vec<usize> = Vec::new();
    let mut map = CidMap::default();
    let mut section = "";
    let mut toks: Vec<&str> = Vec::new();
    for tok in text.split_whitespace() {
        match tok {
            "begincodespacerange" | "begincidrange" | "begincidchar" => {
                section = tok;
                toks.clear();
            }
            "endcodespacerange" | "endcidrange" | "endcidchar" => {
                match section {
                    "begincodespacerange" => {
                        for pair in toks.chunks(2) {
                            if let Some((_, l)) = hex(pair[0]) {
                                lens.push(l);
                            }
                        }
                    }
                    "begincidrange" => {
                        for t in toks.chunks(3) {
                            if t.len() == 3
                                && let (Some((a, _)), Some((b, _)), Ok(c)) = (hex(t[0]), hex(t[1]), t[2].parse::<u32>())
                            {
                                map.ranges.push((a, b, c));
                            }
                        }
                    }
                    "begincidchar" => {
                        for t in toks.chunks(2) {
                            if t.len() == 2
                                && let (Some((a, _)), Ok(c)) = (hex(t[0]), t[1].parse::<u32>())
                            {
                                map.ranges.push((a, a, c));
                            }
                        }
                    }
                    _ => {}
                }
                section = "";
            }
            _ if !section.is_empty() => toks.push(tok),
            _ => {}
        }
    }
    let len = *lens.first()?;
    if lens.iter().any(|l| *l != len) || map.ranges.is_empty() {
        return None;
    }
    Some((len, map))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cmap_ranges() {
        let cm = b"/CIDInit begincmap 1 begincodespacerange <0000> <FFFF> endcodespacerange \
            2 begincidrange <0000> <00FF> 0 <0100> <01FF> 500 endcidrange 1 begincidchar <0300> 7 endcidchar";
        let (len, map) = parse_cmap(cm).unwrap();
        assert_eq!(len, 2);
        assert_eq!(map.cid(0x41), 0x41);
        assert_eq!(map.cid(0x105), 505);
        assert_eq!(map.cid(0x300), 7);
        assert_eq!(map.cid(0x400), 0);
    }

    #[test]
    fn standard_widths() {
        match standard_table("Helvetica").unwrap() {
            Table::Widths(t) => assert_eq!(t[(b'H' - 32) as usize], 722),
            Table::Fixed(_) => panic!(),
        }
        assert!(standard_table("Symbol").is_none());
    }
}
