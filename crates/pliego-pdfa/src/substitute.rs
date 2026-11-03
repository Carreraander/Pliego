//! Sustitución de las 14 fuentes estándar no incrustadas por una fuente Type3 vectorial equivalente.
//!
//! El motor trae esas fuentes, pero no expone sus datos para incrustarlas tal cual; en su lugar se
//! dibuja cada glifo como un procedimiento Type3 (que PDF/A admite sin incrustar nada más), con las
//! mismas anchuras y una correspondencia a Unicode para poder buscar y copiar el texto.

use std::fmt::Write as _;

use mupdf::pdf::{PdfDocument, PdfObject};
use mupdf::{Buffer, Font, Matrix, PathWalker};
use pliego_core::Result;

use crate::io::pdf_err;

/// Unicode de los códigos 0x80..=0x9F de WinAnsi (cp1252); 0 = sin definir.
const CP1252_HIGH: [u16; 32] = [
    0x20AC, 0, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039, 0x0152, 0, 0x017D, 0, 0,
    0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014, 0x02DC, 0x2122, 0x0161, 0x203A, 0x0153, 0, 0x017E, 0x0178,
];

/// Unicode que representa cada código WinAnsi (32..=255), si lo tiene.
fn winansi_unicode(code: u8) -> Option<u32> {
    match code {
        0x20..=0x7E | 0xA0..=0xFF => Some(code as u32),
        0x80..=0x9F => Some(CP1252_HIGH[(code - 0x80) as usize] as u32).filter(|u| *u != 0),
        _ => None,
    }
}

#[derive(Default)]
struct Outline {
    ops: String,
    min: (f32, f32),
    max: (f32, f32),
    any: bool,
}

impl Outline {
    fn point(&mut self, x: f32, y: f32) {
        if self.any {
            self.min = (self.min.0.min(x), self.min.1.min(y));
            self.max = (self.max.0.max(x), self.max.1.max(y));
        } else {
            self.min = (x, y);
            self.max = (x, y);
            self.any = true;
        }
    }
}

impl PathWalker for &mut Outline {
    fn move_to(&mut self, x: f32, y: f32) {
        self.point(x, y);
        let _ = writeln!(self.ops, "{x:.1} {y:.1} m");
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.point(x, y);
        let _ = writeln!(self.ops, "{x:.1} {y:.1} l");
    }
    fn curve_to(&mut self, cx1: f32, cy1: f32, cx2: f32, cy2: f32, ex: f32, ey: f32) {
        for (x, y) in [(cx1, cy1), (cx2, cy2), (ex, ey)] {
            self.point(x, y);
        }
        let _ = writeln!(self.ops, "{cx1:.1} {cy1:.1} {cx2:.1} {cy2:.1} {ex:.1} {ey:.1} c");
    }
    fn close(&mut self) {
        self.ops.push_str("h\n");
    }
}

fn buffer(data: &[u8]) -> Result<Buffer> {
    let mut b = Buffer::with_capacity(data.len());
    std::io::Write::write_all(&mut b, data)?;
    Ok(b)
}

fn stream(doc: &mut PdfDocument, data: &[u8]) -> Result<PdfObject> {
    doc.add_stream(&buffer(data)?, None, false).map_err(pdf_err)
}

fn to_unicode_cmap(map: &[(u8, u32)]) -> String {
    let mut s = String::from(
        "/CIDInit /ProcSet findresource begin 12 dict begin begincmap\n\
         /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n\
         /CMapName /Adobe-Identity-UCS def /CMapType 2 def\n1 begincodespacerange <00> <FF> endcodespacerange\n",
    );
    for chunk in map.chunks(100) {
        let _ = writeln!(s, "{} beginbfchar", chunk.len());
        for (code, uni) in chunk {
            let _ = writeln!(s, "<{code:02X}> <{uni:04X}>");
        }
        s.push_str("endbfchar\n");
    }
    s.push_str("endcmap CMapName currentdict /CMap defineresource pop end end\n");
    s
}

/// Convierte `font` (diccionario de una fuente estándar de 14) en una fuente Type3 equivalente.
pub(crate) fn make_type3(doc: &mut PdfDocument, font: &mut PdfObject, base: &str) -> Result<()> {
    let builtin = Font::new(base).map_err(pdf_err)?;
    let scale = Matrix::new(1000.0, 0.0, 0.0, 1000.0, 0.0, 0.0);
    let mut char_procs = doc.new_dict().map_err(pdf_err)?;
    let mut differences = doc.new_array().map_err(pdf_err)?;
    let mut widths = vec![0i32; 256];
    let mut map = Vec::new();
    let (mut first, mut last) = (255u8, 0u8);
    let mut bbox = Outline::default();
    let mut prev: Option<u8> = None;

    for code in 32u8..=255 {
        let Some(uni) = winansi_unicode(code) else { continue };
        let gid = builtin.encode_character(uni as i32).map_err(pdf_err)?;
        if gid <= 0 && uni != 0x20 && uni != 0xA0 {
            continue;
        }
        let advance = if gid > 0 {
            builtin.advance_glyph(gid).map_err(pdf_err)?
        } else {
            0.278
        };
        let w = (advance * 1000.0).round() as i32;
        let mut outline = Outline::default();
        if gid > 0
            && let Some(path) = builtin.outline_glyph_with_ctm(gid, &scale).map_err(pdf_err)?
        {
            path.walk(&mut outline).map_err(pdf_err)?;
        }
        let (llx, lly, urx, ury) = if outline.any {
            bbox.point(outline.min.0, outline.min.1);
            bbox.point(outline.max.0, outline.max.1);
            (outline.min.0, outline.min.1, outline.max.0, outline.max.1)
        } else {
            (0.0, 0.0, 0.0, 0.0)
        };
        let fill = if outline.any { "f" } else { "" };
        let content = format!("{w} 0 {llx:.0} {lly:.0} {urx:.0} {ury:.0} d1\n{}{fill}\n", outline.ops);
        let proc_ref = stream(doc, content.as_bytes())?;
        let name = format!("uni{uni:04X}");
        char_procs.dict_put(name.as_str(), proc_ref).map_err(pdf_err)?;
        if prev.is_none_or(|p| p + 1 != code) {
            differences
                .array_push(PdfObject::new_int(code as i32).map_err(pdf_err)?)
                .map_err(pdf_err)?;
        }
        differences
            .array_push(PdfObject::new_name(&name).map_err(pdf_err)?)
            .map_err(pdf_err)?;
        prev = Some(code);
        widths[code as usize] = w;
        map.push((code, uni));
        first = first.min(code);
        last = last.max(code);
    }

    let mut ws = doc.new_array().map_err(pdf_err)?;
    for c in first..=last {
        ws.array_push(PdfObject::new_int(widths[c as usize]).map_err(pdf_err)?)
            .map_err(pdf_err)?;
    }
    let mut enc = doc.new_dict().map_err(pdf_err)?;
    enc.dict_put("Type", PdfObject::new_name("Encoding").map_err(pdf_err)?)
        .map_err(pdf_err)?;
    enc.dict_put("Differences", differences).map_err(pdf_err)?;
    let tounicode = stream(doc, to_unicode_cmap(&map).as_bytes())?;

    let mut bb = doc.new_array().map_err(pdf_err)?;
    let (b0, b1, b2, b3) = if bbox.any {
        (bbox.min.0, bbox.min.1, bbox.max.0, bbox.max.1)
    } else {
        (0.0, 0.0, 1000.0, 1000.0)
    };
    for v in [b0, b1, b2, b3] {
        bb.array_push(PdfObject::new_int(v.round() as i32).map_err(pdf_err)?)
            .map_err(pdf_err)?;
    }
    let mut fm = doc.new_array().map_err(pdf_err)?;
    for v in [0.001f32, 0.0, 0.0, 0.001, 0.0, 0.0] {
        fm.array_push(PdfObject::new_real(v).map_err(pdf_err)?)
            .map_err(pdf_err)?;
    }

    for key in ["FontDescriptor", "BaseFont"] {
        if font.get_dict(key).map_err(pdf_err)?.is_some() {
            font.dict_delete(key).map_err(pdf_err)?;
        }
    }
    font.dict_put("Subtype", PdfObject::new_name("Type3").map_err(pdf_err)?)
        .map_err(pdf_err)?;
    font.dict_put("Name", PdfObject::new_name(base).map_err(pdf_err)?)
        .map_err(pdf_err)?;
    font.dict_put("FontBBox", bb).map_err(pdf_err)?;
    font.dict_put("FontMatrix", fm).map_err(pdf_err)?;
    font.dict_put("CharProcs", char_procs).map_err(pdf_err)?;
    font.dict_put("Encoding", enc).map_err(pdf_err)?;
    font.dict_put("FirstChar", PdfObject::new_int(first as i32).map_err(pdf_err)?)
        .map_err(pdf_err)?;
    font.dict_put("LastChar", PdfObject::new_int(last as i32).map_err(pdf_err)?)
        .map_err(pdf_err)?;
    font.dict_put("Widths", ws).map_err(pdf_err)?;
    font.dict_put("ToUnicode", tounicode).map_err(pdf_err)?;
    let res = doc.new_dict().map_err(pdf_err)?;
    font.dict_put("Resources", res).map_err(pdf_err)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn winansi_table() {
        assert_eq!(winansi_unicode(b'A'), Some(0x41));
        assert_eq!(winansi_unicode(0x80), Some(0x20AC));
        assert_eq!(winansi_unicode(0x81), None);
        assert_eq!(winansi_unicode(0xE9), Some(0xE9));
        assert_eq!(winansi_unicode(0x1F), None);
    }
}
