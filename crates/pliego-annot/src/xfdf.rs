//! Intercambio de comentarios en XFDF. Las coordenadas de XFDF son las del espacio PDF (origen abajo a la
//! izquierda); se convierten desde/hacia el espacio de página de MuPDF suponiendo páginas sin rotación.

use std::fmt::Write;

use pliego_core::{Error, Result};
use pliego_pdf::PdfDocument;

use crate::ops::{add, list};
use crate::types::*;
use crate::xml::{self, escape};

fn page_heights(doc: &PdfDocument) -> Result<Vec<f32>> {
    let n = doc.page_count()?;
    (0..n)
        .map(|p| {
            let b = doc.load_pdf_page(p)?.bounds()?;
            Ok(b.y1 - b.y0)
        })
        .collect()
}

fn num(v: f32) -> String {
    let s = format!("{v:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn hex(c: Rgb) -> String {
    let b = |x: f32| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02X}{:02X}{:02X}", b(c[0]), b(c[1]), b(c[2]))
}

fn parse_hex(s: &str) -> Option<Rgb> {
    let s = s.strip_prefix('#')?;
    if s.len() != 6 {
        return None;
    }
    let c = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).ok().map(|v| f32::from(v) / 255.0);
    Some([c(0)?, c(2)?, c(4)?])
}

fn tag(k: AnnotKind) -> Option<&'static str> {
    Some(match k {
        AnnotKind::Note => "text",
        AnnotKind::FreeText => "freetext",
        AnnotKind::Highlight => "highlight",
        AnnotKind::Underline => "underline",
        AnnotKind::StrikeOut => "strikeout",
        AnnotKind::Squiggly => "squiggly",
        AnnotKind::Line => "line",
        AnnotKind::Square => "square",
        AnnotKind::Circle => "circle",
        AnnotKind::Polygon => "polygon",
        AnnotKind::PolyLine => "polyline",
        AnnotKind::Ink => "ink",
        AnnotKind::Stamp => "stamp",
        _ => return None,
    })
}

/// Exporta los comentarios del documento como XFDF. Se omiten adjuntos, enlaces y sellos de imagen
/// (los sellos de texto y predefinidos se exportan como `<stamp>`).
pub fn export_xfdf(doc: &mut PdfDocument) -> Result<String> {
    let hs = page_heights(doc)?;
    let items = list(doc, None)?;
    let nm_of = |xref: i32| items.iter().find(|i| i.id.xref == xref).and_then(|i| i.nm.clone());
    let mut s = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<xfdf xmlns=\"http://ns.adobe.com/xfdf/\" xml:space=\"preserve\">\n<annots>\n",
    );
    for i in &items {
        let Some(t) = tag(i.kind) else { continue };
        if i.kind == AnnotKind::Stamp && i.stamp.is_none() {
            continue;
        }
        let h = hs[i.id.page];
        let pt = |p: PointPts| format!("{},{}", num(p[0]), num(h - p[1]));
        let r = i.rect;
        let _ = write!(
            s,
            "<{t} page=\"{}\" rect=\"{},{},{},{}\"",
            i.id.page,
            num(r[0]),
            num(h - r[3]),
            num(r[2]),
            num(h - r[1])
        );
        if let Some(nm) = &i.nm {
            let _ = write!(s, " name=\"{}\"", escape(nm));
        }
        if let Some((text, c)) = &i.stamp {
            let _ = write!(
                s,
                " icon=\"Custom\" pliego-text=\"{}\" pliego-color=\"{}\"",
                escape(text),
                hex(*c)
            );
        }
        if let Some(c) = i.style.color {
            let _ = write!(s, " color=\"{}\"", hex(c));
        }
        if let Some(c) = i.style.interior_color {
            let _ = write!(s, " interior-color=\"{}\"", hex(c));
        }
        if let Some(o) = i.style.opacity {
            let _ = write!(s, " opacity=\"{}\"", num(o));
        }
        if let Some(w) = i.style.border_width {
            let _ = write!(s, " width=\"{}\"", num(w));
        }
        if let Some(a) = &i.author {
            let _ = write!(s, " title=\"{}\"", escape(a));
        }
        if let Some(a) = &i.subject {
            let _ = write!(s, " subject=\"{}\"", escape(a));
        }
        if let Some(d) = &i.created {
            let _ = write!(s, " creationdate=\"{}\"", escape(d));
        }
        if let Some(d) = &i.modified {
            let _ = write!(s, " date=\"{}\"", escape(d));
        }
        if let Some(parent) = i.reply_to.and_then(nm_of) {
            let _ = write!(s, " inreplyto=\"{}\" replyType=\"R\"", escape(&parent));
        }
        if let Some(st) = i.state {
            let _ = write!(s, " statemodel=\"Review\" state=\"{}\"", st.pdf_name());
        }
        if let Some((a, b)) = i.line {
            let _ = write!(s, " start=\"{}\" end=\"{}\"", pt(a), pt(b));
        }
        if !i.quads.is_empty() {
            let v: Vec<String> = i
                .quads
                .iter()
                .flat_map(|q| (0..4).map(move |k| (q[2 * k], q[2 * k + 1])))
                .flat_map(|(x, y)| [num(x), num(h - y)])
                .collect();
            let _ = write!(s, " coords=\"{}\"", v.join(","));
        }
        s.push('>');
        if let Some(c) = &i.contents {
            let _ = write!(s, "<contents>{}</contents>", escape(c));
        }
        if !i.points.is_empty() {
            let v: Vec<String> = i.points.iter().map(|p| pt(*p)).collect();
            let _ = write!(s, "<vertices>{}</vertices>", v.join(";"));
        }
        if !i.strokes.is_empty() {
            s.push_str("<inklist>");
            for st in &i.strokes {
                let v: Vec<String> = st.iter().map(|p| pt(*p)).collect();
                let _ = write!(s, "<gesture>{}</gesture>", v.join(";"));
            }
            s.push_str("</inklist>");
        }
        let _ = writeln!(s, "</{t}>");
    }
    s.push_str("</annots>\n</xfdf>\n");
    Ok(s)
}

fn floats(s: &str) -> Vec<f32> {
    s.split([',', ' ', ';']).filter_map(|x| x.trim().parse().ok()).collect()
}

fn points(s: &str, h: f32) -> Vec<PointPts> {
    s.split(';')
        .filter_map(|p| {
            let v = floats(p);
            (v.len() == 2).then(|| [v[0], h - v[1]])
        })
        .collect()
}

/// Importa comentarios desde XFDF y devuelve cuántos se crearon. Los que ya existen (mismo `name`/NM) se omiten,
/// de modo que importar dos veces el mismo archivo no duplica nada.
pub fn import_xfdf(doc: &mut PdfDocument, xfdf: &str) -> Result<usize> {
    let root = xml::parse(xfdf)?;
    if root.name != "xfdf" {
        return Err(Error::InvalidInput("no es un archivo XFDF".into()));
    }
    let Some(annots) = root.child("annots") else {
        return Ok(0);
    };
    let hs = page_heights(doc)?;
    let mut known: Vec<(String, AnnotId)> = list(doc, None)?
        .into_iter()
        .filter_map(|i| i.nm.map(|n| (n, i.id)))
        .collect();
    let mut created = 0;
    // Dos pasadas para que una respuesta pueda aparecer antes que su comentario.
    let mut pending: Vec<&xml::Node> = annots.children.iter().collect();
    for _ in 0..2 {
        let mut next = Vec::new();
        for n in pending {
            match build(n, &hs, &known)? {
                Built::Skip => {}
                Built::Later => next.push(n),
                Built::Spec(spec) => {
                    let nm = spec.nm.clone();
                    let id = add(doc, &spec)?;
                    created += 1;
                    if let Some(nm) = nm {
                        known.push((nm, id));
                    }
                }
            }
        }
        pending = next;
        if pending.is_empty() {
            break;
        }
    }
    Ok(created)
}

enum Built {
    Skip,
    Later,
    Spec(Box<AnnotSpec>),
}

impl From<AnnotSpec> for Built {
    fn from(s: AnnotSpec) -> Self {
        Built::Spec(Box::new(s))
    }
}

fn build(n: &xml::Node, hs: &[f32], known: &[(String, AnnotId)]) -> Result<Built> {
    let name = n.attr("name").map(str::to_owned);
    if name.as_ref().is_some_and(|nm| known.iter().any(|(k, _)| k == nm)) {
        return Ok(Built::Skip);
    }
    let page: usize = n.attr("page").and_then(|p| p.parse().ok()).unwrap_or(0);
    let Some(&h) = hs.get(page) else {
        return Err(Error::InvalidInput(format!(
            "la página {} no existe en el documento",
            page + 1
        )));
    };
    let r = n.attr("rect").map(floats).filter(|v| v.len() == 4);
    let rect = r.map(|v| [v[0], h - v[3], v[2], h - v[1]]);
    let need_rect = || rect.ok_or_else(|| Error::InvalidInput(format!("<{}> sin rect válido", n.name)));
    let quads = || -> Vec<QuadPts> {
        let v = n.attr("coords").map(floats).unwrap_or_default();
        (0..v.len() / 8)
            .map(|i| {
                let c = &v[i * 8..i * 8 + 8];
                [c[0], h - c[1], c[2], h - c[3], c[4], h - c[5], c[6], h - c[7]]
            })
            .collect()
    };
    let one = |a: Option<&str>| a.map(|s| points(s, h)).and_then(|p| p.first().copied());
    let geometry = match n.name.as_str() {
        "text" => {
            let r = need_rect()?;
            Geometry::Note { at: [r[0], r[1]] }
        }
        "freetext" => Geometry::FreeText {
            rect: need_rect()?,
            font_size: 12.0,
        },
        "highlight" => Geometry::Highlight { quads: quads() },
        "underline" => Geometry::Underline { quads: quads() },
        "strikeout" => Geometry::StrikeOut { quads: quads() },
        "squiggly" => Geometry::Squiggly { quads: quads() },
        "line" => Geometry::Line {
            start: one(n.attr("start")).ok_or_else(|| Error::InvalidInput("<line> sin start".into()))?,
            end: one(n.attr("end")).ok_or_else(|| Error::InvalidInput("<line> sin end".into()))?,
            start_style: LineEnd::None,
            end_style: LineEnd::None,
        },
        "square" => Geometry::Square { rect: need_rect()? },
        "circle" => Geometry::Circle { rect: need_rect()? },
        "polygon" => Geometry::Polygon {
            points: n.child("vertices").map(|v| points(&v.text, h)).unwrap_or_default(),
        },
        "polyline" => Geometry::PolyLine {
            points: n.child("vertices").map(|v| points(&v.text, h)).unwrap_or_default(),
        },
        "ink" => Geometry::Ink {
            strokes: n
                .child("inklist")
                .map(|l| l.children.iter().map(|g| points(&g.text, h)).collect())
                .unwrap_or_default(),
        },
        "stamp" => match n.attr("pliego-text") {
            Some(text) => Geometry::Stamp {
                rect: need_rect()?,
                source: StampSource::Text {
                    text: text.to_owned(),
                    color: n.attr("pliego-color").and_then(parse_hex).unwrap_or([0.1, 0.3, 0.7]),
                },
            },
            None => return Ok(Built::Skip),
        },
        _ => return Ok(Built::Skip),
    };
    let mut spec = AnnotSpec::new(page, geometry);
    spec.nm = name;
    spec.author = n.attr("title").map(str::to_owned);
    spec.subject = n.attr("subject").map(str::to_owned);
    spec.date = n.attr("creationdate").or(n.attr("date")).map(str::to_owned);
    spec.contents = n
        .child("contents")
        .map(|c| c.text.clone())
        .or_else(|| n.attr("contents").map(str::to_owned));
    spec.style = Style {
        color: n.attr("color").and_then(parse_hex),
        interior_color: n.attr("interior-color").and_then(parse_hex),
        opacity: n.attr("opacity").and_then(|v| v.parse().ok()),
        border_width: n.attr("width").and_then(|v| v.parse().ok()),
    };
    spec.state = n.attr("state").and_then(ReviewState::from_pdf_name);
    if let Some(parent) = n.attr("inreplyto") {
        match known.iter().find(|(k, _)| k == parent) {
            Some((_, id)) => spec.reply_to = Some(*id),
            None => return Ok(Built::Later),
        }
    }
    Ok(spec.into())
}
