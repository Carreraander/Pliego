//! Incrustación de un subconjunto de fuente TrueType/OpenType como fuente CID Identity-H.
//!
//! Solo se guardan los glifos usados (con `subsetter`), con `CIDToGIDMap` identidad, anchos (`W`) y un
//! `ToUnicode` para que el texto siga siendo seleccionable y buscable.

use std::collections::HashMap;
use std::fmt::Write as _;

use pliego_core::{Error, Result};
use pliego_pdf::{PdfDocument, PdfPage};
use rustybuzz::Face;
use subsetter::GlyphRemapper;

use crate::unicode::Shaper;

/// Fuente incrustada: nombre del recurso y paso de glifo original a código del subconjunto.
pub struct Embedded {
    pub resource: String,
    pub codes: HashMap<u16, u16>,
}

fn utf16_hex(s: &str) -> String {
    s.encode_utf16().map(|u| format!("{u:04X}")).collect()
}

fn to_unicode_cmap(entries: &[(u16, String)]) -> String {
    let mut s = String::from(
        "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n\
         /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n\
         /CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n\
         1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
    );
    for chunk in entries.chunks(100) {
        let _ = writeln!(s, "{} beginbfchar", chunk.len());
        for (code, text) in chunk {
            let _ = writeln!(s, "<{code:04X}> <{}>", utf16_hex(text));
        }
        s.push_str("endbfchar\n");
    }
    s.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    s
}

/// Etiqueta de subconjunto: seis mayúsculas distintas para cada combinación de fuente y glifos (como pide la norma).
fn font_tag(label: &str, glyphs: &[u16]) -> String {
    let base: String = label
        .split('.')
        .next()
        .unwrap_or("Fuente")
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect();
    // FNV-1a sobre la fuente y sus glifos.
    let mut h: u64 = 0xcbf29ce484222325;
    for b in label.bytes().chain(glyphs.iter().flat_map(|g| g.to_le_bytes())) {
        h = (h ^ u64::from(b)).wrapping_mul(0x100000001b3);
    }
    let prefix: String = (0..6).map(|i| char::from(b'A' + ((h >> (i * 5)) % 26) as u8)).collect();
    format!("{prefix}+{}", if base.is_empty() { "Fuente".into() } else { base })
}

/// Crea el subconjunto con los glifos de `lines` y lo registra en los recursos de la página.
pub fn embed_subset(
    doc: &mut PdfDocument,
    page: &PdfPage,
    data: &[u8],
    label: &str,
    lines: &[String],
) -> Result<Embedded> {
    let shaper = Shaper::new(data)?;
    let face = Face::from_slice(data, 0).ok_or_else(|| Error::InvalidInput("la fuente no se puede leer".into()))?;
    let upem = (face.units_per_em() as f32).max(1.0);
    let to1000 = |v: f32| (v * 1000.0 / upem).round() as i32;

    // Glifos usados y texto de cada uno.
    let mut remapper = GlyphRemapper::new();
    remapper.remap(0);
    let mut unicode: Vec<(u16, String)> = Vec::new();
    let mut codes: HashMap<u16, u16> = HashMap::new();
    codes.insert(0, 0);
    for line in lines {
        for (gid, text) in shaper.shape(line, 1.0).glyphs {
            let new = remapper.remap(gid);
            if codes.insert(gid, new).is_none() && !text.is_empty() {
                unicode.push((new, text));
            }
        }
    }
    let bytes = subsetter::subset(data, 0, &remapper)
        .map_err(|e| Error::Pdf(format!("no se pudo reducir la fuente: {e:?}")))?;
    let is_cff = face.tables().cff.is_some();

    // Anchos de los glifos del subconjunto, en milésimas.
    let mut widths: Vec<(u16, i32)> = codes
        .iter()
        .map(|(old, new)| {
            let adv = face
                .glyph_hor_advance(rustybuzz::ttf_parser::GlyphId(*old))
                .unwrap_or(0);
            (*new, to1000(f32::from(adv)))
        })
        .collect();
    widths.sort_unstable();
    let mut w = String::new();
    for (g, v) in &widths {
        let _ = write!(w, "{g} [{v}] ");
    }

    let mut used: Vec<u16> = codes.keys().copied().collect();
    used.sort_unstable();
    let tag = font_tag(label, &used);
    let file_dict = doc.new_object_from_str(&format!("<< /Length1 {} >>", bytes.len()))?;
    let file_dict = if is_cff {
        doc.new_object_from_str("<< /Subtype /OpenType >>")?
    } else {
        file_dict
    };
    let file = doc.add_stream(&bytes, Some(&file_dict), false)?;
    let file_ref = file.as_indirect()?;
    let bb = face.global_bounding_box();
    let ascent = to1000(f32::from(face.ascender()));
    let descent = to1000(f32::from(face.descender()));
    let cap = face.capital_height().map_or(ascent, |c| to1000(f32::from(c)));
    let key = if is_cff { "FontFile3" } else { "FontFile2" };
    let desc = doc.new_object_from_str(&format!(
        "<< /Type /FontDescriptor /FontName /{tag} /Flags 4 /FontBBox [{} {} {} {}] \
             /ItalicAngle {} /Ascent {ascent} /Descent {descent} /CapHeight {cap} /StemV 80 /{key} {file_ref} 0 R >>",
        to1000(f32::from(bb.x_min)),
        to1000(f32::from(bb.y_min)),
        to1000(f32::from(bb.x_max)),
        to1000(f32::from(bb.y_max)),
        face.italic_angle().round() as i32,
    ))?;
    let desc_ref = doc.add_object(&desc)?.as_indirect()?;
    let (subtype, map_entry) = if is_cff {
        ("CIDFontType0", "")
    } else {
        ("CIDFontType2", "/CIDToGIDMap /Identity ")
    };
    let cid = doc.new_object_from_str(&format!(
        "<< /Type /Font /Subtype /{subtype} /BaseFont /{tag} \
             /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> \
             /FontDescriptor {desc_ref} 0 R /DW 1000 /W [{w}] {map_entry}>>"
    ))?;
    let cid_ref = doc.add_object(&cid)?.as_indirect()?;

    unicode.sort_unstable_by_key(|(c, _)| *c);
    let tu = to_unicode_cmap(&unicode);
    let tu_ref = doc.add_stream(tu.as_bytes(), None, false)?.as_indirect()?;
    let type0 = doc.new_object_from_str(&format!(
        "<< /Type /Font /Subtype /Type0 /BaseFont /{tag} /Encoding /Identity-H \
             /DescendantFonts [{cid_ref} 0 R] /ToUnicode {tu_ref} 0 R >>"
    ))?;
    let type0 = doc.add_object(&type0)?;

    // Registrar en /Resources /Font con un nombre libre.
    let mut res = page.resources()?;
    let mut fonts = match res.get_dict("Font")? {
        Some(f) if f.is_dict()? => f,
        _ => {
            let d = doc.new_dict()?;
            res.dict_put("Font", d.clone())?;
            d
        }
    };
    let mut n = 0;
    let name = loop {
        let cand = format!("PlUni{n}");
        if fonts.get_dict(cand.as_str())?.is_none() {
            break cand;
        }
        n += 1;
    };
    fonts.dict_put(name.as_str(), type0)?;
    Ok(Embedded {
        resource: format!("/{name}"),
        codes,
    })
}
