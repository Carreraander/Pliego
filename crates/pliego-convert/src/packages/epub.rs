//! EPUB: un ZIP con un `container.xml` que apunta al `.opf`, cuyo `spine` ordena los capítulos (XHTML).
//!
//! Los capítulos se pasan al maquetador de HTML básico de Pliego: títulos, párrafos, listas, tablas, enlaces, énfasis y
//! las imágenes `<img>` que se pueden decodificar (JPG, PNG, GIF, WebP, BMP). No hay CSS, SVG, fuentes incrustadas,
//! fórmulas ni maquetación fija. Cada capítulo empieza en página nueva.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;
use std::rc::Rc;

use pliego_core::{Error, JobContext, Report, Result};
use pliego_pdf::native::Raster;

use super::zipio::{Package, resolve};
use crate::text::{TextToPdfOptions, page_spec};
use crate::typeset::{self, PAGE_BREAK_HR};
use crate::util::{ensure_not_same, write_atomic};

const MAX_XML_BYTES: u64 = 32 << 20;
const MAX_CHAPTER_BYTES: u64 = 64 << 20;
const MAX_IMAGE_BYTES: u64 = 64 << 20;
const MAX_IMAGES_TOTAL: u64 = 512 << 20;
const MAX_IMAGES: usize = 5_000;
const MAX_CHAPTERS: usize = 5_000;

/// Convierte un EPUB en un PDF (texto de todos los capítulos del `spine` y sus imágenes).
pub fn epub_to_pdf(input: &Path, output: &Path, options: &TextToPdfOptions, ctx: &JobContext) -> Result<Report> {
    ensure_not_same(input, output)?;
    let spec = page_spec(options)?;
    let mut package = Package::open(input)?;
    let bad = |what: &str| Error::InvalidInput(format!("{}: EPUB no válido ({what})", input.display()));

    // 1. container.xml → ruta del .opf
    let container = package.require("META-INF/container.xml", MAX_XML_BYTES)?;
    let container = String::from_utf8_lossy(&container).into_owned();
    let container = roxmltree::Document::parse(&container).map_err(|_| bad("container.xml"))?;
    let opf_path = container
        .descendants()
        .find(|n| n.tag_name().name() == "rootfile")
        .and_then(|n| n.attribute("full-path"))
        .map(super::zipio::normalize)
        .ok_or_else(|| bad("no indica el archivo .opf"))?;

    // 2. .opf → manifiesto y spine
    let opf = package.require(&opf_path, MAX_XML_BYTES)?;
    let opf = String::from_utf8_lossy(&opf).into_owned();
    let opf = roxmltree::Document::parse(&opf).map_err(|_| bad("el archivo .opf"))?;
    let mut manifest: HashMap<&str, (&str, bool)> = HashMap::new();
    for item in opf.descendants().filter(|n| n.tag_name().name() == "item") {
        if let (Some(id), Some(href)) = (item.attribute("id"), item.attribute("href")) {
            let is_nav = item
                .attribute("properties")
                .is_some_and(|p| p.split_whitespace().any(|w| w == "nav"));
            manifest.insert(id, (href, is_nav));
        }
    }
    let chapters: Vec<String> = opf
        .descendants()
        .filter(|n| n.tag_name().name() == "itemref")
        .filter_map(|n| n.attribute("idref"))
        .filter_map(|id| manifest.get(id))
        .filter(|(_, is_nav)| !is_nav)
        .filter_map(|(href, _)| resolve(&opf_path, href))
        .take(MAX_CHAPTERS)
        .collect();
    if chapters.is_empty() {
        return Err(bad("el spine no tiene capítulos"));
    }

    // 3. capítulos → un solo HTML con saltos de página, y las imágenes que citan
    let mut html = String::new();
    let mut wanted: BTreeSet<String> = BTreeSet::new();
    let mut read_chapters = 0usize;
    for (n, chapter) in chapters.iter().enumerate() {
        ctx.step(n as u64, chapters.len() as u64 + 1, "leyendo capítulos")?;
        let Some(bytes) = package.read(chapter, MAX_CHAPTER_BYTES)? else {
            continue;
        };
        let text = decode_text(&bytes);
        let body = body_of(&text);
        if read_chapters > 0 {
            html.push_str(PAGE_BREAK_HR);
            html.push('\n');
        }
        html.push_str(&rewrite_img_src(body, chapter, &mut wanted));
        html.push('\n');
        read_chapters += 1;
    }
    if read_chapters == 0 {
        return Err(bad("no se pudo leer ningún capítulo"));
    }

    // 4. imágenes
    let mut images: HashMap<String, Rc<Raster>> = HashMap::new();
    let mut total = 0u64;
    let mut skipped = 0usize;
    for path in wanted.iter().take(MAX_IMAGES) {
        let decoded = package
            .read(path, MAX_IMAGE_BYTES)
            .ok()
            .flatten()
            .filter(|b| {
                total += b.len() as u64;
                total <= MAX_IMAGES_TOTAL
            })
            .and_then(|b| Raster::decode(&b).ok())
            .and_then(|mut frames| (!frames.is_empty()).then(|| frames.swap_remove(0)));
        match decoded {
            Some(r) => {
                images.insert(path.clone(), Rc::new(r));
            }
            None => skipped += 1,
        }
    }
    ctx.step(chapters.len() as u64, chapters.len() as u64 + 1, "maquetando")?;
    let typeset = typeset::html_with_images(&html, &spec, images)?;
    let bytes = typeset::save_bytes(&typeset.pdf)?;
    write_atomic(output, &bytes)?;
    ctx.step(chapters.len() as u64 + 1, chapters.len() as u64 + 1, "listo")?;

    let mut report = Report::with_output(
        output,
        format!("{} página(s) creadas desde {read_chapters} capítulo(s)", typeset.pages),
    );
    if typeset.substituted > 0 {
        report.warnings.push(format!(
            "{} carácter(es) fuera de Windows-1252 (cirílico, griego, CJK, emojis…) se sustituyeron por «?»",
            typeset.substituted
        ));
    }
    if skipped > 0 {
        report.warnings.push(format!(
            "{skipped} imagen(es) no se pudieron incluir (formato no admitido, como SVG, o dañadas)"
        ));
    }
    report
        .warnings
        .push("El EPUB se maqueta sin su CSS ni sus fuentes: es una vista para leer, no una copia fiel.".into());
    Ok(report)
}

/// Texto del capítulo: UTF-8 (con o sin BOM) o UTF-16 con BOM.
fn decode_text(bytes: &[u8]) -> String {
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        let units: Vec<u16> = rest.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).collect();
        return String::from_utf16_lossy(&units);
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        let units: Vec<u16> = rest.as_chunks::<2>().0.iter().map(|c| u16::from_be_bytes(*c)).collect();
        return String::from_utf16_lossy(&units);
    }
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    String::from_utf8_lossy(bytes).into_owned()
}

/// Contenido de `<body>…</body>` (o todo el texto si no lo tiene).
fn body_of(html: &str) -> &str {
    let lower = html.to_ascii_lowercase();
    let Some(start) = lower.find("<body") else { return html };
    let Some(open_end) = lower[start..].find('>') else {
        return html;
    };
    let from = start + open_end + 1;
    let to = lower.rfind("</body").filter(|&t| t >= from).unwrap_or(html.len());
    &html[from..to]
}

/// Sustituye el `src` de cada `<img>` por la ruta absoluta dentro del paquete (la clave de las imágenes cargadas) y
/// anota las rutas que hay que cargar. Las direcciones externas o `data:` se dejan como están.
fn rewrite_img_src(body: &str, chapter: &str, wanted: &mut BTreeSet<String>) -> String {
    let lower = body.to_ascii_lowercase();
    let mut out = String::with_capacity(body.len());
    let mut at = 0;
    while let Some(rel) = lower[at..].find("<img") {
        let tag_start = at + rel;
        // Debe ser exactamente la etiqueta `img` (no `imgx`).
        let after = lower.as_bytes().get(tag_start + 4).copied().unwrap_or(b'>');
        let tag_end = lower[tag_start..].find('>').map_or(body.len(), |e| tag_start + e);
        if !(after.is_ascii_whitespace() || after == b'/' || after == b'>') {
            out.push_str(&body[at..tag_start + 4]);
            at = tag_start + 4;
            continue;
        }
        let tag = &body[tag_start..tag_end];
        let tag_lower = &lower[tag_start..tag_end];
        out.push_str(&body[at..tag_start]);
        match find_src(tag_lower) {
            Some((vs, ve)) => {
                out.push_str(&tag[..vs]);
                let raw = &tag[vs..ve];
                match resolve(chapter, &decode_entities(raw)) {
                    Some(path) => {
                        out.push_str(&path);
                        wanted.insert(path);
                    }
                    None => out.push_str(raw),
                }
                out.push_str(&tag[ve..]);
            }
            None => out.push_str(tag),
        }
        at = tag_end;
    }
    out.push_str(&body[at..]);
    out
}

/// Posición (inicio, fin) del valor entre comillas de `src=` dentro de una etiqueta (en minúsculas).
fn find_src(tag: &str) -> Option<(usize, usize)> {
    let b = tag.as_bytes();
    let mut from = 0;
    while let Some(rel) = tag[from..].find("src") {
        let i = from + rel;
        from = i + 3;
        if i == 0 || !b[i - 1].is_ascii_whitespace() {
            continue;
        }
        let mut j = i + 3;
        while j < b.len() && b[j].is_ascii_whitespace() {
            j += 1;
        }
        if b.get(j) != Some(&b'=') {
            continue;
        }
        j += 1;
        while j < b.len() && b[j].is_ascii_whitespace() {
            j += 1;
        }
        let q = *b.get(j)?;
        if q != b'"' && q != b'\'' {
            continue;
        }
        let vs = j + 1;
        let ve = tag[vs..].find(q as char)? + vs;
        return Some((vs, ve));
    }
    None
}

fn decode_entities(s: &str) -> String {
    s.replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_is_extracted() {
        assert_eq!(
            body_of("<html><head><title>x</title></head><BODY class=a><p>hola</p></BODY></html>"),
            "<p>hola</p>"
        );
        assert_eq!(body_of("<p>sin cuerpo</p>"), "<p>sin cuerpo</p>");
    }

    #[test]
    fn img_sources_become_package_paths() {
        let mut wanted = BTreeSet::new();
        let html = r#"<p>a <img alt="x" src="../img/Foto%201.png"/> b <img src='c.jpg'> <imgx src="no"> <img src="http://x.org/a.png"></p>"#;
        let out = rewrite_img_src(html, "OEBPS/text/c1.xhtml", &mut wanted);
        assert!(out.contains(r#"src="OEBPS/img/Foto 1.png""#), "{out}");
        assert!(out.contains("src='OEBPS/text/c.jpg'"), "{out}");
        assert!(out.contains(r#"<imgx src="no">"#), "{out}");
        assert!(out.contains(r#"src="http://x.org/a.png""#), "{out}");
        assert_eq!(
            wanted.into_iter().collect::<Vec<_>>(),
            ["OEBPS/img/Foto 1.png", "OEBPS/text/c.jpg"]
        );
    }

    #[test]
    fn utf16_chapters_are_read() {
        let mut bytes = vec![0xFF, 0xFE];
        for u in "<p>hola</p>".encode_utf16() {
            bytes.extend_from_slice(&u.to_le_bytes());
        }
        assert_eq!(decode_text(&bytes), "<p>hola</p>");
    }
}
