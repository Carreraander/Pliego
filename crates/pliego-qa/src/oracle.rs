//! Oráculo de redacción: dado un PDF redactado y los datos que debían desaparecer, comprueba por varias vías
//! independientes que no queda rastro. Es la verificación obligatoria de cualquier motor que redacte (W38).
//!
//! Vías: texto extraído y búsqueda del motor, `pdftotext` (extractor ajeno, si está instalado), bytes del archivo
//! (ASCII, UTF-16BE y cadenas hexadecimales), flujos descomprimidos (incluidos los de objetos comprimidos) y
//! literales de texto de los flujos de contenido, aunque el texto vaya partido por ajustes de interletraje
//! (`[(Hol)-20(a)] TJ`). Opcionalmente, que una región de la página renderizada sea de un solo color.

use std::io::Read;
use std::path::Path;
use std::process::Command;

use flate2::read::ZlibDecoder;
use pliego_core::Result;
use serde::{Deserialize, Serialize};

use crate::engines::DynEngine;

/// Zona de página en puntos PDF con origen arriba a la izquierda (como el render a 72 ppp).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Region {
    pub page: u32,
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

#[derive(Debug, Clone, Default)]
pub struct OracleConfig {
    /// Cadenas que no deben quedar en ninguna parte.
    pub needles: Vec<String>,
    /// Zonas que deben quedar de un solo color tras redactar.
    pub regions: Vec<Region>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    pub check: String,
    pub detail: String,
}

fn norm(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect()
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && hay.windows(needle.len()).any(|w| w == needle)
}

fn utf16be(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_be_bytes).collect()
}

fn hex(bytes: &[u8], upper: bool) -> Vec<u8> {
    bytes
        .iter()
        .flat_map(|b| {
            let s = if upper { format!("{b:02X}") } else { format!("{b:02x}") };
            s.into_bytes()
        })
        .collect()
}

/// Representaciones en bytes de una cadena tal como podría aparecer en un archivo PDF.
fn byte_forms(needle: &str) -> Vec<(&'static str, Vec<u8>)> {
    let mut v = vec![
        ("ASCII/UTF-8", needle.as_bytes().to_vec()),
        ("UTF-16BE", utf16be(needle)),
        ("hex", hex(needle.as_bytes(), true)),
        ("hex", hex(needle.as_bytes(), false)),
        ("hex UTF-16BE", hex(&utf16be(needle), true)),
        ("hex UTF-16BE", hex(&utf16be(needle), false)),
    ];
    // Los espacios dentro del dato suelen ir como ajuste de interletraje; se busca también la forma compacta.
    let compact: String = needle.split_whitespace().collect();
    if compact != needle {
        v.push(("compacta", compact.into_bytes()));
    }
    v
}

/// Segmentos `stream ... endstream` del archivo, descomprimidos si son Flate; si no, tal cual.
pub fn streams(data: &[u8]) -> Vec<Vec<u8>> {
    const START: &[u8] = b"stream";
    const END: &[u8] = b"endstream";
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(p) = find(data, START, i) {
        i = p + START.len();
        if p >= 3 && &data[p - 3..p] == b"end" {
            continue;
        }
        let mut s = p + START.len();
        if data.get(s) == Some(&b'\r') {
            s += 1;
        }
        if data.get(s) == Some(&b'\n') {
            s += 1;
        }
        let Some(e) = find(data, END, s) else { break };
        let raw = &data[s..e];
        let mut dec = Vec::new();
        // Los datos pueden traer basura al final: lo decodificado antes del error también cuenta.
        let _ = ZlibDecoder::new(raw).read_to_end(&mut dec);
        out.push(if dec.is_empty() { raw.to_vec() } else { dec });
        i = e + END.len();
    }
    out
}

fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    hay.get(from..)?
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

/// Concatena el contenido de todos los literales `( )` y cadenas hexadecimales `< >` de un flujo de contenido,
/// para encontrar texto partido por `TJ`. Los literales hexadecimales de 2 bytes por carácter se devuelven aparte.
pub fn text_literals(content: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let (mut lit, mut hexs) = (Vec::new(), Vec::new());
    let mut i = 0;
    while i < content.len() {
        match content[i] {
            b'(' => {
                let mut depth = 1;
                i += 1;
                while i < content.len() && depth > 0 {
                    match content[i] {
                        b'\\' => {
                            i += 1;
                            if let Some(&c) = content.get(i) {
                                lit.push(match c {
                                    b'n' => b'\n',
                                    b'r' => b'\r',
                                    b't' => b'\t',
                                    other => other,
                                });
                            }
                        }
                        b'(' => {
                            depth += 1;
                            lit.push(b'(');
                        }
                        b')' => {
                            depth -= 1;
                            if depth > 0 {
                                lit.push(b')');
                            }
                        }
                        c => lit.push(c),
                    }
                    i += 1;
                }
            }
            b'<' if content.get(i + 1) != Some(&b'<') => {
                let mut j = i + 1;
                let mut digits = Vec::new();
                while j < content.len() && content[j] != b'>' {
                    if content[j].is_ascii_hexdigit() {
                        digits.push(content[j]);
                    }
                    j += 1;
                }
                if digits.len() % 2 == 1 {
                    digits.push(b'0');
                }
                for p in digits.chunks(2) {
                    if let Ok(b) = u8::from_str_radix(std::str::from_utf8(p).unwrap_or("00"), 16) {
                        hexs.push(b);
                    }
                }
                i = j + 1;
            }
            b'<' => i += 2,
            _ => i += 1,
        }
    }
    (lit, hexs)
}

fn without_nul(b: &[u8]) -> Vec<u8> {
    b.iter().copied().filter(|&c| c != 0).collect()
}

fn pdftotext(path: &Path) -> Option<String> {
    let out = Command::new("pdftotext")
        .arg("-layout")
        .arg(path)
        .arg("-")
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Palabras (separadas por espacios) de un texto, ordenadas: se comparan como multiconjunto, así que no depende
/// del orden de lectura (que cambia en páginas giradas).
fn sorted_words(text: &str) -> Vec<String> {
    let mut w: Vec<String> = text.split_whitespace().map(str::to_owned).collect();
    w.sort();
    w
}

/// Quita de `words` (ordenadas) una copia de cada palabra de `remove`; las que no estén se ignoran.
fn remove_words(words: &mut Vec<String>, remove: &[String]) {
    for r in remove {
        if let Ok(i) = words.binary_search(r) {
            words.remove(i);
        }
    }
}

/// Palabras que sobran y que faltan en `got` respecto a `expected` (ambas ordenadas).
fn word_diff(expected: &[String], got: &[String]) -> (Vec<String>, Vec<String>) {
    let (mut extra, mut missing) = (Vec::new(), Vec::new());
    let (mut i, mut j) = (0, 0);
    while i < expected.len() || j < got.len() {
        match (expected.get(i), got.get(j)) {
            (Some(e), Some(g)) if e == g => {
                i += 1;
                j += 1;
            }
            (Some(e), Some(g)) if e < g => {
                missing.push(e.clone());
                i += 1;
            }
            (Some(_), Some(g)) => {
                extra.push(g.clone());
                j += 1;
            }
            (Some(e), None) => {
                missing.push(e.clone());
                i += 1;
            }
            (None, Some(g)) => {
                extra.push(g.clone());
                j += 1;
            }
            (None, None) => break,
        }
    }
    (extra, missing)
}

/// Compara el texto extraíble del redactado con el del original, página a página, palabra a palabra: el resultado
/// tiene que ser exactamente el original menos las palabras de cada dato (una vez por cada aparición del dato).
///
/// Detecta lo que la búsqueda de la cadena entera no ve: **restos parciales** (un trozo del dato que sobrevive, como
/// «8Z» de «12345678Z» cuando la zona de borrado quedó corta o desplazada) y **texto vecino perdido** (palabras que no
/// eran del dato y han desaparecido porque la zona se pasó). Es la comprobación que faltaba cuando la redacción en
/// páginas giradas 90°/270° dejaba trozos y se llevaba vecinos sin que el oráculo lo notara.
pub fn check_against_original(
    engine: &dyn DynEngine,
    original: &Path,
    redacted: &Path,
    needles: &[String],
) -> Result<Vec<Finding>> {
    let mut f = Vec::new();
    let before = engine.open(original)?;
    let after = engine.open(redacted)?;
    let pages = before.page_count()?;
    if after.page_count()? != pages {
        f.push(Finding {
            check: "páginas".into(),
            detail: format!(
                "el original tiene {pages} páginas y el redactado {}",
                after.page_count()?
            ),
        });
        return Ok(f);
    }
    for p in 0..pages {
        let text = before.page_text(p)?;
        let squashed = norm(&text);
        let mut expected = sorted_words(&text);
        for needle in needles {
            let nn = norm(needle);
            if nn.is_empty() {
                continue;
            }
            let hits = squashed.matches(&nn).count();
            let words: Vec<String> = needle.split_whitespace().map(str::to_owned).collect();
            for _ in 0..hits {
                remove_words(&mut expected, &words);
            }
        }
        let got = sorted_words(&after.page_text(p)?);
        let (extra, missing) = word_diff(&expected, &got);
        if !extra.is_empty() {
            f.push(Finding {
                check: "resto parcial".into(),
                detail: format!(
                    "página {}: quedan palabras que no deberían estar ({}): puede ser un trozo de un dato",
                    p + 1,
                    extra.join(", ")
                ),
            });
        }
        if !missing.is_empty() {
            f.push(Finding {
                check: "texto vecino perdido".into(),
                detail: format!(
                    "página {}: faltan palabras que no eran del dato ({})",
                    p + 1,
                    missing.join(", ")
                ),
            });
        }
    }
    Ok(f)
}

/// Ejecuta todas las comprobaciones. Devuelve la lista de hallazgos: vacía = el archivo pasa el oráculo.
pub fn check_redacted(engine: &dyn DynEngine, path: &Path, cfg: &OracleConfig) -> Result<Vec<Finding>> {
    let mut f = Vec::new();
    let mut add = |check: &str, detail: String| {
        f.push(Finding {
            check: check.into(),
            detail,
        })
    };
    let doc = engine.open(path)?;
    let pages = doc.page_count()?;
    for needle in &cfg.needles {
        let nn = norm(needle);
        for p in 0..pages {
            if norm(&doc.page_text(p)?).contains(&nn) {
                add(
                    "texto extraíble",
                    format!("«{needle}» se extrae en la página {}", p + 1),
                );
            }
            if doc.search_page(p, needle, 1).is_ok_and(|h| !h.quads.is_empty()) {
                add("búsqueda", format!("«{needle}» se encuentra en la página {}", p + 1));
            }
        }
    }
    if let Some(text) = pdftotext(path) {
        for needle in &cfg.needles {
            if norm(&text).contains(&norm(needle)) {
                add("pdftotext", format!("«{needle}» se extrae con pdftotext"));
            }
        }
    }
    let data = std::fs::read(path).map_err(|e| pliego_core::Error::io_at(path, e))?;
    let decoded = streams(&data);
    for needle in &cfg.needles {
        for (kind, b) in byte_forms(needle) {
            if contains(&data, &b) {
                add(
                    "bytes del archivo",
                    format!("«{needle}» aparece en el archivo ({kind})"),
                );
            }
            if decoded.iter().any(|s| contains(s, &b)) {
                add(
                    "flujo descomprimido",
                    format!("«{needle}» aparece en un flujo ({kind})"),
                );
            }
        }
        let nn = norm(needle).into_bytes();
        for s in &decoded {
            let (lit, hx) = text_literals(s);
            let hay = [
                norm(&String::from_utf8_lossy(&lit)).into_bytes(),
                norm(&String::from_utf8_lossy(&without_nul(&hx))).into_bytes(),
            ];
            if hay.iter().any(|h| contains(h, &nn)) {
                add(
                    "literales de texto",
                    format!("«{needle}» está en los literales de un flujo de contenido"),
                );
            }
        }
    }
    for r in &cfg.regions {
        let img = doc.render_page(r.page, 150.0 / 72.0)?;
        let k = 150.0 / 72.0;
        let (x0, x1) = ((r.x0 * k) as u32, ((r.x1 * k) as u32).min(img.width));
        let (y0, y1) = ((r.y0 * k) as u32, ((r.y1 * k) as u32).min(img.height));
        let mut counts = std::collections::HashMap::<[u8; 3], u32>::new();
        for y in y0..y1 {
            for x in x0..x1 {
                let i = ((y * img.width + x) * 4) as usize;
                *counts
                    .entry([img.data[i], img.data[i + 1], img.data[i + 2]])
                    .or_insert(0) += 1;
            }
        }
        let total: u32 = counts.values().sum();
        let top = counts.values().max().copied().unwrap_or(0);
        if total > 0 && f64::from(top) / f64::from(total) < 0.995 {
            add(
                "región no uniforme",
                format!(
                    "página {}: la zona ({}, {})-({}, {}) conserva contenido visible",
                    r.page + 1,
                    r.x0,
                    r.y0,
                    r.x1,
                    r.y1
                ),
            );
        }
    }
    Ok(f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literals_join_split_text() {
        let (lit, _) = text_literals(b"BT [(Hol)-20(a \\(x\\))] TJ ET");
        assert_eq!(lit, b"Hola (x)");
    }

    fn w(s: &str) -> Vec<String> {
        sorted_words(s)
    }

    #[test]
    fn word_diff_reports_leftovers_and_lost_neighbours() {
        // Trozo del dato que sobrevive («8Z») y vecino perdido («Correo»).
        let (extra, missing) = word_diff(&w("DNI Correo texto"), &w("DNI 8Z texto"));
        assert_eq!(extra, ["8Z"]);
        assert_eq!(missing, ["Correo"]);
        let (extra, missing) = word_diff(&w("a b c"), &w("c b a"));
        assert!(extra.is_empty() && missing.is_empty());
    }

    #[test]
    fn remove_words_takes_one_copy_each() {
        let mut v = w("12 12 345");
        remove_words(&mut v, &w("12 999"));
        assert_eq!(v, w("12 345"));
    }

    #[test]
    fn hex_strings_decode() {
        let (_, hx) = text_literals(b"<486f6c61> Tj << /A 1 >>");
        assert_eq!(hx, b"Hola");
    }

    #[test]
    fn streams_inflate_flate_and_keep_raw() {
        use flate2::{Compression, write::ZlibEncoder};
        use std::io::Write;
        let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
        e.write_all(b"secreto 123").unwrap();
        let z = e.finish().unwrap();
        let mut pdf = b"1 0 obj\n<< >>\nstream\n".to_vec();
        pdf.extend(&z);
        pdf.extend(b"\nendstream\nendobj\n2 0 obj\n<< >>\nstream\nplano 456\nendstream\n");
        let s = streams(&pdf);
        assert!(s.iter().any(|x| contains(x, b"secreto 123")));
        assert!(s.iter().any(|x| contains(x, b"plano 456")));
    }
}
