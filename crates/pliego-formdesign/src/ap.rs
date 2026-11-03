//! Apariencias de campos de texto, listas y botones (MuPDF no las genera al crear campos nuevos).

use mupdf::Buffer;
use mupdf::pdf::{PdfDocument, PdfObject};
use pliego_core::{Error, Result};

use crate::objects::*;
use crate::pe;

/// Texto como cadena PDF en WinAnsi (lo que no cabe se sustituye por `?`).
fn winansi(s: &str) -> String {
    let mut out = String::from("(");
    for c in s.chars() {
        match c {
            '(' | ')' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            c if (' '..='~').contains(&c) => out.push(c),
            c if (c as u32) >= 0xA0 && (c as u32) <= 0xFF => out.push_str(&format!("\\{:03o}", c as u32)),
            _ => out.push('?'),
        }
    }
    out.push(')');
    out
}

/// Cuerpo de letra de un `/DA` (`/Helv 12 Tf 0 g`); 0 si es automático o no hay.
pub fn da_size(widget: &PdfObject) -> f32 {
    let Some(da) = widget
        .get_dict_inheritable("DA")
        .ok()
        .flatten()
        .and_then(|d| d.as_string().ok())
    else {
        return 0.0;
    };
    let toks: Vec<&str> = da.split_whitespace().collect();
    toks.iter()
        .position(|t| *t == "Tf")
        .and_then(|i| i.checked_sub(1))
        .and_then(|i| toks[i].parse().ok())
        .unwrap_or(0.0)
}

fn helv(doc: &PdfDocument) -> Result<Option<PdfObject>> {
    let af = doc.catalog().map_err(pe)?.get_dict("AcroForm").map_err(pe)?;
    Ok(af
        .and_then(|a| a.get_dict("DR").ok().flatten())
        .and_then(|d| d.get_dict("Font").ok().flatten())
        .and_then(|f| f.get_dict("Helv").ok().flatten()))
}

/// Regenera `/AP /N` de un widget de texto, combo, lista o botón de acción a partir de sus propiedades.
pub fn rebuild_ap(doc: &mut PdfDocument, widget: &mut PdfObject) -> Result<()> {
    let field = field_of(widget)?;
    let ft = field
        .get_dict_inheritable("FT")
        .map_err(pe)?
        .and_then(|t| t.as_name().ok())
        .unwrap_or_default();
    let ff = flags_of(&field);
    let r = raw_rect(widget);
    let (w, h) = ((r[2] - r[0]).abs(), (r[3] - r[1]).abs());
    let text_of = |k: &str| {
        field.get_dict_inheritable(k).ok().flatten().and_then(|v| {
            v.as_string()
                .ok()
                .or_else(|| v.as_name().ok().map(|n| String::from_utf8_lossy(&n).into_owned()))
        })
    };
    let is_push = ft == b"Btn" && ff & F_PUSHBUTTON != 0;
    if !(ft == b"Tx" || ft == b"Ch" || is_push) {
        return Ok(());
    }
    let size_cfg = da_size(widget);
    let mut c = String::new();
    // Fondo y borde.
    if is_push {
        c.push_str(&format!("0.85 g 0 0 {w} {h} re f "));
    } else {
        c.push_str(&format!("1 g 0 0 {w} {h} re f "));
    }
    c.push_str(&format!("0 G 1 w 0.5 0.5 {} {} re S ", w - 1.0, h - 1.0));
    let font = |s: f32| format!("/Helv {s} Tf 0 g");
    if is_push {
        let cap = widget
            .get_dict("MK")
            .ok()
            .flatten()
            .and_then(|m| m.get_dict("CA").ok().flatten())
            .and_then(|v| v.as_string().ok())
            .unwrap_or_default();
        if !cap.is_empty() {
            let s = if size_cfg > 0.0 {
                size_cfg
            } else {
                (h - 6.0).clamp(6.0, 12.0)
            };
            let tw = cap.chars().count() as f32 * s * 0.5;
            c.push_str(&format!(
                "q BT {} {} {} Td {} Tj ET Q",
                font(s),
                ((w - tw) / 2.0).max(2.0),
                (h - s) / 2.0 + s * 0.22,
                winansi(&cap)
            ));
        }
    } else if ft == b"Tx" {
        let v = text_of("V").unwrap_or_default();
        let multiline = ff & F_MULTILINE != 0;
        let s = if size_cfg > 0.0 {
            size_cfg
        } else if multiline {
            11.0
        } else {
            (h - 6.0).clamp(6.0, 12.0)
        };
        let first = if ff & F_PASSWORD != 0 {
            "*".repeat(v.chars().count())
        } else {
            v.lines().next().unwrap_or("").to_string()
        };
        let y = if multiline {
            h - 2.0 - s
        } else {
            (h - s) / 2.0 + s * 0.22
        };
        if !first.is_empty() {
            c.push_str(&format!(
                "/Tx BMC q 2 2 {} {} re W n BT {} 2 {y} Td {} Tj ET Q EMC",
                w - 4.0,
                h - 4.0,
                font(s),
                winansi(&first)
            ));
        }
    } else if ff & F_COMBO != 0 {
        let v = text_of("V").unwrap_or_default();
        let s = if size_cfg > 0.0 {
            size_cfg
        } else {
            (h - 6.0).clamp(6.0, 12.0)
        };
        if !v.is_empty() {
            c.push_str(&format!(
                "/Tx BMC q 2 2 {} {} re W n BT {} 2 {} Td {} Tj ET Q EMC",
                w - 4.0 - 14.0_f32.min(w / 2.0),
                h - 4.0,
                font(s),
                (h - s) / 2.0 + s * 0.22,
                winansi(&v)
            ));
        }
        // Flecha del desplegable.
        let ax = w - 12.0;
        if ax > 4.0 {
            c.push_str(&format!(
                " 0 g {ax} {} m {} {} l {} {} l f",
                h / 2.0 + 2.0,
                ax + 8.0,
                h / 2.0 + 2.0,
                ax + 4.0,
                h / 2.0 - 2.0
            ));
        }
    } else {
        // Lista: una línea por opción; la elegida, resaltada.
        let sel = text_of("V").unwrap_or_default();
        let s = if size_cfg > 0.0 { size_cfg } else { 11.0 };
        let line = s * 1.2;
        let opts = crate::read::options_of(&field);
        c.push_str(&format!("/Tx BMC q 2 2 {} {} re W n ", w - 4.0, h - 4.0));
        for (i, o) in opts.iter().enumerate() {
            let top = h - 2.0 - line * i as f32;
            if top < 0.0 {
                break;
            }
            if *o == sel {
                c.push_str(&format!("0.6 0.75 0.95 rg 2 {} {} {line} re f ", top - line, w - 4.0));
            }
            c.push_str(&format!("BT {} 3 {} Td {} Tj ET ", font(s), top - s, winansi(o)));
        }
        c.push_str("Q EMC");
    }

    let Some(helv) = helv(doc)? else {
        return Err(Error::Pdf("el formulario no tiene fuente por defecto".into()));
    };
    let res = obj(doc, "<< /Font << >> >>")?;
    let mut fonts = res
        .get_dict("Font")
        .map_err(pe)?
        .ok_or_else(|| Error::Pdf("no se pudo crear /Font".into()))?;
    put(&mut fonts, "Helv", helv)?;
    let mut dict = obj(doc, &format!("<< /Type /XObject /Subtype /Form /BBox [0 0 {w} {h}] >>"))?;
    put(&mut dict, "Resources", res)?;
    let buf = Buffer::from_bytes(c.as_bytes()).map_err(pe)?;
    let n = doc.add_stream(&buf, Some(&dict), false).map_err(pe)?;
    let mut ap = obj(doc, "<< >>")?;
    put(&mut ap, "N", n)?;
    put(widget, "AP", ap)
}
