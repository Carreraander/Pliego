//! Escritor de PPTX (PresentationML): una diapositiva por página, con texto editable, tablas e imágenes.

use std::collections::{BTreeSet, HashMap};
use std::fmt::Write as _;

use pliego_core::Result;

use crate::docx::font_family;
use crate::model::*;
use crate::util::{Package, emu, xml_escape};

const NS: &str = "xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" \
xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" \
xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\"";
const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

/// Los textos se escriben con `lang="es-ES"` (idioma de revisión por defecto); en inglés se cambia por `en-US` para que
/// Office no marque como erróneo el texto del documento. Las comillas del contenido van escapadas, no hay falsas coincidencias.
fn proofing_lang(xml: &str, lang: pliego_core::Lang) -> std::borrow::Cow<'_, str> {
    match lang {
        pliego_core::Lang::Es => std::borrow::Cow::Borrowed(xml),
        pliego_core::Lang::En => std::borrow::Cow::Owned(xml.replace("lang=\"es-ES\"", "lang=\"en-US\"")),
    }
}

fn rpr(r: &Run, k: f32, tag: &str) -> String {
    let sz = (r.size * k * 100.0).round().max(100.0) as i32;
    let mut s = format!("<a:{tag} lang=\"es-ES\" sz=\"{sz}\"");
    if r.bold {
        s.push_str(" b=\"1\"");
    }
    if r.italic {
        s.push_str(" i=\"1\"");
    }
    s.push_str(" dirty=\"0\">");
    if r.color.len() == 7 {
        let _ = write!(
            s,
            "<a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill>",
            r.color[1..].to_uppercase()
        );
    }
    let f = xml_escape(&font_family(r));
    let _ = write!(s, "<a:latin typeface=\"{f}\"/><a:cs typeface=\"{f}\"/></a:{tag}>");
    s
}

fn runs_xml(runs: &[Run], k: f32) -> String {
    let mut out = String::new();
    for r in runs.iter().filter(|r| !r.text.is_empty()) {
        let _ = write!(out, "<a:r>{}<a:t>{}</a:t></a:r>", rpr(r, k, "rPr"), xml_escape(&r.text));
    }
    out
}

fn algn(a: Align) -> &'static str {
    match a {
        Align::Left => "l",
        Align::Right => "r",
        Align::Center => "ctr",
        Align::Justify => "just",
    }
}

struct Slide {
    xml: String,
    /// (nombre de archivo, bytes, ext) de las imágenes, en el orden de `rIdImgN`.
    media: Vec<(String, Vec<u8>, &'static str)>,
}

struct Ids {
    shape: u32,
    media_total: usize,
}

fn xfrm(tag: &str, b: &BBox, k: f32, ox: f32, oy: f32, extra_w: f32) -> String {
    format!(
        "<{tag}><a:off x=\"{}\" y=\"{}\"/><a:ext cx=\"{}\" cy=\"{}\"/></{tag}>",
        emu(b.x0 * k + ox),
        emu(b.y0 * k + oy),
        emu((b.x1 - b.x0) * k + extra_w),
        emu((b.y1 - b.y0) * k)
    )
}

fn textbox(p: &Paragraph, k: f32, ox: f32, oy: f32, ids: &mut Ids, counters: &mut HashMap<u32, u32>) -> String {
    ids.shape += 1;
    let mut runs = p.runs.clone();
    if let Some(l) = p.list {
        let prefix = match l.kind {
            ListKind::Bullet => "• ".to_string(),
            ListKind::Decimal => {
                let n = counters.entry(l.id).or_insert(l.start);
                let s = format!("{n}. ");
                *n += 1;
                s
            }
        };
        if let Some(first) = runs.first().cloned() {
            runs.insert(
                0,
                Run {
                    text: prefix,
                    field: None,
                    ..first
                },
            );
        }
    }
    let wrap = if p.lines <= 1 { "none" } else { "square" };
    let slack = if p.lines <= 1 {
        4.0
    } else {
        3.0 + 0.02 * (p.bbox.x1 - p.bbox.x0)
    };
    let mut ppr = format!("<a:pPr algn=\"{}\"", algn(p.align));
    let lm = (p.left_x - p.bbox.x0).max(0.0) * k;
    if lm > 0.5 {
        let _ = write!(ppr, " marL=\"{}\"", emu(lm));
    }
    let _ = write!(
        ppr,
        "><a:lnSpc><a:spcPts val=\"{}\"/></a:lnSpc><a:spcBef><a:spcPts val=\"0\"/></a:spcBef><a:buNone/></a:pPr>",
        (p.line_pitch * k * 100.0).round().max(100.0) as i32
    );
    format!(
        "<p:sp><p:nvSpPr><p:cNvPr id=\"{id}\" name=\"Texto {id}\"/><p:cNvSpPr txBox=\"1\"/><p:nvPr/></p:nvSpPr>\
         <p:spPr>{x}<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom><a:noFill/></p:spPr>\
         <p:txBody><a:bodyPr wrap=\"{wrap}\" lIns=\"0\" tIns=\"0\" rIns=\"0\" bIns=\"0\" anchor=\"t\"><a:noAutofit/></a:bodyPr><a:lstStyle/>\
         <a:p>{ppr}{runs}</a:p></p:txBody></p:sp>",
        id = ids.shape,
        x = xfrm("a:xfrm", &p.bbox, k, ox, oy, slack * k),
        runs = runs_xml(&runs, k)
    )
}

fn table(t: &Table, k: f32, ox: f32, oy: f32, ids: &mut Ids) -> String {
    ids.shape += 1;
    let widths: Vec<i64> = t.cols.iter().map(|(l, r)| emu((r - l) * k)).collect();
    let total: i64 = widths.iter().sum();
    let height: i64 = t.row_heights.iter().map(|h| emu(h * k)).sum();
    let mut s = format!(
        "<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id=\"{id}\" name=\"Tabla {id}\"/><p:cNvGraphicFramePr><a:graphicFrameLocks noGrp=\"1\"/></p:cNvGraphicFramePr><p:nvPr/></p:nvGraphicFramePr>\
         <p:xfrm><a:off x=\"{x}\" y=\"{y}\"/><a:ext cx=\"{total}\" cy=\"{height}\"/></p:xfrm>\
         <a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/table\"><a:tbl><a:tblPr/><a:tblGrid>",
        id = ids.shape,
        x = emu(t.cols[0].0 * k + ox),
        y = emu(t.bbox.y0 * k + oy),
    );
    for w in &widths {
        let _ = write!(s, "<a:gridCol w=\"{w}\"/>");
    }
    s.push_str("</a:tblGrid>");
    let line =
        |tag: &str| format!("<a:{tag} w=\"6350\"><a:solidFill><a:srgbClr val=\"BFBFBF\"/></a:solidFill></a:{tag}>");
    let borders = format!("{}{}{}{}", line("lnL"), line("lnR"), line("lnT"), line("lnB"));
    for (ri, row) in t.rows.iter().enumerate() {
        let _ = write!(s, "<a:tr h=\"{}\">", emu(t.row_heights[ri] * k));
        for cell in row {
            let span = if cell.span > 1 {
                format!(" gridSpan=\"{}\"", cell.span)
            } else {
                String::new()
            };
            let body = if cell.runs.iter().all(|r| r.text.trim().is_empty()) {
                format!(
                    "<a:p><a:pPr algn=\"{}\"/><a:endParaRPr lang=\"es-ES\" sz=\"1000\"/></a:p>",
                    algn(cell.align)
                )
            } else {
                format!(
                    "<a:p><a:pPr algn=\"{}\"/>{}</a:p>",
                    algn(cell.align),
                    runs_xml(&cell.runs, k)
                )
            };
            let _ = write!(
                s,
                "<a:tc{span}><a:txBody><a:bodyPr/><a:lstStyle/>{body}</a:txBody>\
                 <a:tcPr marL=\"25400\" marR=\"25400\" marT=\"0\" marB=\"0\" anchor=\"ctr\">{borders}<a:noFill/></a:tcPr></a:tc>"
            );
            for _ in 1..cell.span {
                let _ = write!(
                    s,
                    "<a:tc hMerge=\"1\"><a:txBody><a:bodyPr/><a:lstStyle/><a:p><a:endParaRPr lang=\"es-ES\"/></a:p></a:txBody><a:tcPr/></a:tc>"
                );
            }
        }
        s.push_str("</a:tr>");
    }
    s.push_str("</a:tbl></a:graphicData></a:graphic></p:graphicFrame>");
    s
}

fn picture(img: &ImageData, rid: &str, k: f32, ox: f32, oy: f32, ids: &mut Ids) -> String {
    ids.shape += 1;
    format!(
        "<p:pic><p:nvPicPr><p:cNvPr id=\"{id}\" name=\"Imagen {id}\"/><p:cNvPicPr><a:picLocks noChangeAspect=\"1\"/></p:cNvPicPr><p:nvPr/></p:nvPicPr>\
         <p:blipFill><a:blip r:embed=\"{rid}\"/><a:stretch><a:fillRect/></a:stretch></p:blipFill>\
         <p:spPr>{x}<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr></p:pic>",
        id = ids.shape,
        x = xfrm("a:xfrm", &img.bbox, k, ox, oy, 0.0)
    )
}

fn slide(page: &PageLayout, (sw, sh): (f32, f32), ids: &mut Ids) -> Slide {
    // Las páginas de otro tamaño se escalan para caber en la diapositiva.
    let k = (sw / page.width).min(sh / page.height);
    let (ox, oy) = ((sw - page.width * k) / 2.0, (sh - page.height * k) / 2.0);
    let mut shapes = String::new();
    let mut media = Vec::new();
    let mut counters = HashMap::new();
    for el in page.elements() {
        match el {
            Element::Paragraph(p) => shapes.push_str(&textbox(p, k, ox, oy, ids, &mut counters)),
            Element::Table(t) => shapes.push_str(&table(t, k, ox, oy, ids)),
            Element::Image(img) => {
                ids.media_total += 1;
                let name = format!("image{}.{}", ids.media_total, img.ext);
                let rid = format!("rIdImg{}", media.len() + 1);
                media.push((name, img.data.clone(), img.ext));
                shapes.push_str(&picture(img, &rid, k, ox, oy, ids));
            }
        }
    }
    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><p:sld {NS}><p:cSld><p:spTree>\
         <p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>\
         <p:grpSpPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/><a:chOff x=\"0\" y=\"0\"/><a:chExt cx=\"0\" cy=\"0\"/></a:xfrm></p:grpSpPr>\
         {shapes}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>"
    );
    Slide { xml, media }
}

const THEME: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<a:theme xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" name=\"Pliego\"><a:themeElements>\
<a:clrScheme name=\"Pliego\"><a:dk1><a:srgbClr val=\"000000\"/></a:dk1><a:lt1><a:srgbClr val=\"FFFFFF\"/></a:lt1>\
<a:dk2><a:srgbClr val=\"1F2937\"/></a:dk2><a:lt2><a:srgbClr val=\"F3F4F6\"/></a:lt2>\
<a:accent1><a:srgbClr val=\"2563EB\"/></a:accent1><a:accent2><a:srgbClr val=\"059669\"/></a:accent2>\
<a:accent3><a:srgbClr val=\"D97706\"/></a:accent3><a:accent4><a:srgbClr val=\"DC2626\"/></a:accent4>\
<a:accent5><a:srgbClr val=\"7C3AED\"/></a:accent5><a:accent6><a:srgbClr val=\"0891B2\"/></a:accent6>\
<a:hlink><a:srgbClr val=\"2563EB\"/></a:hlink><a:folHlink><a:srgbClr val=\"7C3AED\"/></a:folHlink></a:clrScheme>\
<a:fontScheme name=\"Pliego\"><a:majorFont><a:latin typeface=\"Arial\"/><a:ea typeface=\"\"/><a:cs typeface=\"\"/></a:majorFont>\
<a:minorFont><a:latin typeface=\"Arial\"/><a:ea typeface=\"\"/><a:cs typeface=\"\"/></a:minorFont></a:fontScheme>\
<a:fmtScheme name=\"Pliego\"><a:fillStyleLst><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill>\
<a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill></a:fillStyleLst>\
<a:lnStyleLst><a:ln w=\"6350\"><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill></a:ln>\
<a:ln w=\"12700\"><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill></a:ln>\
<a:ln w=\"19050\"><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill></a:ln></a:lnStyleLst>\
<a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle>\
<a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst>\
<a:bgFillStyleLst><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill>\
<a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill></a:bgFillStyleLst></a:fmtScheme></a:themeElements></a:theme>";

const MASTER: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<p:sldMaster xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" \
xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" \
xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\"><p:cSld>\
<p:bg><p:bgPr><a:solidFill><a:srgbClr val=\"FFFFFF\"/></a:solidFill><a:effectLst/></p:bgPr></p:bg><p:spTree>\
<p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>\
<p:grpSpPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/><a:chOff x=\"0\" y=\"0\"/><a:chExt cx=\"0\" cy=\"0\"/></a:xfrm></p:grpSpPr>\
</p:spTree></p:cSld>\
<p:clrMap bg1=\"lt1\" tx1=\"dk1\" bg2=\"lt2\" tx2=\"dk2\" accent1=\"accent1\" accent2=\"accent2\" accent3=\"accent3\" accent4=\"accent4\" accent5=\"accent5\" accent6=\"accent6\" hlink=\"hlink\" folHlink=\"folHlink\"/>\
<p:sldLayoutIdLst><p:sldLayoutId id=\"2147483649\" r:id=\"rId1\"/></p:sldLayoutIdLst>\
<p:txStyles><p:titleStyle><a:lvl1pPr><a:defRPr sz=\"1800\"/></a:lvl1pPr></p:titleStyle>\
<p:bodyStyle><a:lvl1pPr><a:defRPr sz=\"1800\"/></a:lvl1pPr></p:bodyStyle>\
<p:otherStyle><a:lvl1pPr><a:defRPr sz=\"1800\"/></a:lvl1pPr></p:otherStyle></p:txStyles></p:sldMaster>";

const LAYOUT: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<p:sldLayout xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" \
xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" \
xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\" type=\"blank\" preserve=\"1\"><p:cSld name=\"En blanco\"><p:spTree>\
<p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>\
<p:grpSpPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/><a:chOff x=\"0\" y=\"0\"/><a:chExt cx=\"0\" cy=\"0\"/></a:xfrm></p:grpSpPr>\
</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>";

fn rels(items: &[(&str, &str, String)]) -> String {
    let mut s = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
         <Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">",
    );
    for (id, ty, target) in items {
        let _ = write!(s, "<Relationship Id=\"{id}\" Type=\"{ty}\" Target=\"{target}\"/>");
    }
    s.push_str("</Relationships>");
    s
}

pub(crate) fn write_pptx(layout: &Layout) -> Result<Vec<u8>> {
    let first = layout.pages.first();
    let size = first.map_or((612.0, 792.0), |p| (p.width, p.height));
    let mut ids = Ids {
        shape: 1,
        media_total: 0,
    };
    let slides: Vec<Slide> = layout
        .pages
        .iter()
        .map(|p| {
            ids.shape = 1;
            slide(p, size, &mut ids)
        })
        .collect();

    let mut pkg = Package::new();
    let exts: BTreeSet<&str> = slides.iter().flat_map(|s| s.media.iter().map(|m| m.2)).collect();
    let pml = "application/vnd.openxmlformats-officedocument.presentationml";
    let mut ct = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
         <Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
         <Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
         <Default Extension=\"xml\" ContentType=\"application/xml\"/>\
         <Override PartName=\"/ppt/presentation.xml\" ContentType=\"{pml}.presentation.main+xml\"/>\
         <Override PartName=\"/ppt/slideMasters/slideMaster1.xml\" ContentType=\"{pml}.slideMaster+xml\"/>\
         <Override PartName=\"/ppt/slideLayouts/slideLayout1.xml\" ContentType=\"{pml}.slideLayout+xml\"/>\
         <Override PartName=\"/ppt/theme/theme1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.theme+xml\"/>"
    );
    for e in exts {
        let _ = write!(ct, "<Default Extension=\"{e}\" ContentType=\"image/{e}\"/>");
    }
    let mut pres = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><p:presentation {NS}>\
         <p:sldMasterIdLst><p:sldMasterId id=\"2147483648\" r:id=\"rIdM\"/></p:sldMasterIdLst><p:sldIdLst>"
    );
    let mut pres_rels = vec![
        ("rIdM", "slideMaster", "slideMasters/slideMaster1.xml".to_string()),
        ("rIdT", "theme", "theme/theme1.xml".to_string()),
    ];
    let ids_rels: Vec<String> = (1..=slides.len()).map(|i| format!("rIdS{i}")).collect();
    for (i, s) in slides.iter().enumerate() {
        let k = i + 1;
        let _ = write!(
            ct,
            "<Override PartName=\"/ppt/slides/slide{k}.xml\" ContentType=\"{pml}.slide+xml\"/>"
        );
        let _ = write!(pres, "<p:sldId id=\"{}\" r:id=\"rIdS{k}\"/>", 255 + k);
        pkg.add(
            &format!("ppt/slides/slide{k}.xml"),
            proofing_lang(&s.xml, layout.lang).as_bytes(),
        )?;
        let mut items: Vec<(String, String, String)> = vec![(
            "rIdL".into(),
            format!("{REL}/slideLayout"),
            "../slideLayouts/slideLayout1.xml".into(),
        )];
        for (j, (name, data, _)) in s.media.iter().enumerate() {
            items.push((
                format!("rIdImg{}", j + 1),
                format!("{REL}/image"),
                format!("../media/{name}"),
            ));
            pkg.add(&format!("ppt/media/{name}"), data)?;
        }
        let refs: Vec<(&str, &str, String)> = items
            .iter()
            .map(|(a, b, c)| (a.as_str(), b.as_str(), c.clone()))
            .collect();
        pkg.add(
            &format!("ppt/slides/_rels/slide{k}.xml.rels"),
            full_rels(&refs).as_bytes(),
        )?;
    }
    for (id, i) in ids_rels.iter().zip(1..) {
        pres_rels.push((id.as_str(), "slide", format!("slides/slide{i}.xml")));
    }
    let _ = write!(
        pres,
        "</p:sldIdLst><p:sldSz cx=\"{}\" cy=\"{}\"/><p:notesSz cx=\"6858000\" cy=\"9144000\"/></p:presentation>",
        emu(size.0),
        emu(size.1)
    );
    ct.push_str("</Types>");
    pkg.add("[Content_Types].xml", ct.as_bytes())?;
    pkg.add(
        "_rels/.rels",
        full_rels(&[(
            "rId1",
            &format!("{REL}/officeDocument"),
            "ppt/presentation.xml".to_string(),
        )])
        .as_bytes(),
    )?;
    pkg.add("ppt/presentation.xml", pres.as_bytes())?;
    let typed: Vec<(&str, String, String)> = pres_rels
        .iter()
        .map(|(id, ty, t)| (*id, format!("{REL}/{ty}"), t.clone()))
        .collect();
    let refs: Vec<(&str, &str, String)> = typed.iter().map(|(a, b, c)| (*a, b.as_str(), c.clone())).collect();
    pkg.add("ppt/_rels/presentation.xml.rels", full_rels(&refs).as_bytes())?;
    pkg.add("ppt/slideMasters/slideMaster1.xml", MASTER.as_bytes())?;
    pkg.add(
        "ppt/slideMasters/_rels/slideMaster1.xml.rels",
        full_rels(&[
            (
                "rId1",
                &format!("{REL}/slideLayout"),
                "../slideLayouts/slideLayout1.xml".to_string(),
            ),
            ("rId2", &format!("{REL}/theme"), "../theme/theme1.xml".to_string()),
        ])
        .as_bytes(),
    )?;
    pkg.add("ppt/slideLayouts/slideLayout1.xml", LAYOUT.as_bytes())?;
    pkg.add(
        "ppt/slideLayouts/_rels/slideLayout1.xml.rels",
        full_rels(&[(
            "rId1",
            &format!("{REL}/slideMaster"),
            "../slideMasters/slideMaster1.xml".to_string(),
        )])
        .as_bytes(),
    )?;
    pkg.add("ppt/theme/theme1.xml", THEME.as_bytes())?;
    pkg.finish()
}

/// Como [`rels`], pero con el tipo ya completo.
fn full_rels(items: &[(&str, &str, String)]) -> String {
    rels(items)
}
