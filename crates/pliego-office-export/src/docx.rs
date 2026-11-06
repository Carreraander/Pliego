//! Escritor de DOCX (WordprocessingML) a partir del modelo de maquetación.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use pliego_core::Result;

use crate::model::*;
use crate::util::{Package, twips, xml_escape};

const NS: &str = "xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\" \
xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" \
xmlns:wp=\"http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing\" \
xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" \
xmlns:pic=\"http://schemas.openxmlformats.org/drawingml/2006/picture\"";

/// Familia de fuente de Office a partir del nombre que trae el PDF.
pub(crate) fn font_family(run: &Run) -> String {
    let lower = run.font.to_lowercase();
    for (needle, name) in [
        ("helvetica", "Arial"),
        ("arial", "Arial"),
        ("times", "Times New Roman"),
        ("courier", "Courier New"),
        ("calibri", "Calibri"),
        ("cambria", "Cambria"),
        ("verdana", "Verdana"),
        ("tahoma", "Tahoma"),
        ("georgia", "Georgia"),
        ("segoe", "Segoe UI"),
        ("consolas", "Consolas"),
    ] {
        if lower.contains(needle) {
            return name.to_string();
        }
    }
    let base = run.font.split(['-', ',']).next().unwrap_or("");
    let base = base
        .strip_suffix("PSMT")
        .or(base.strip_suffix("MT"))
        .or(base.strip_suffix("PS"))
        .unwrap_or(base);
    if base.is_empty() {
        return if run.monospace {
            "Courier New"
        } else if run.serif {
            "Times New Roman"
        } else {
            "Arial"
        }
        .into();
    }
    // CamelCase → palabras ("CenturyGothic" → "Century Gothic").
    let mut out = String::new();
    for (i, c) in base.chars().enumerate() {
        if i > 0 && c.is_uppercase() && !base.contains(' ') {
            out.push(' ');
        }
        out.push(c);
    }
    out
}

fn run_props(r: &Run) -> String {
    let font = xml_escape(&font_family(r));
    let mut s = format!("<w:rFonts w:ascii=\"{font}\" w:hAnsi=\"{font}\" w:cs=\"{font}\"/>");
    if r.bold {
        s.push_str("<w:b/><w:bCs/>");
    }
    if r.italic {
        s.push_str("<w:i/><w:iCs/>");
    }
    if r.color.len() == 7 && r.color != "#000000" {
        let _ = write!(s, "<w:color w:val=\"{}\"/>", r.color[1..].to_uppercase());
    }
    let half = (r.size * 2.0).round().max(2.0) as i32;
    let _ = write!(s, "<w:sz w:val=\"{half}\"/><w:szCs w:val=\"{half}\"/>");
    s
}

fn runs_xml(runs: &[Run]) -> String {
    let mut out = String::new();
    for r in runs {
        let props = run_props(r);
        if let Some(f) = r.field {
            let instr = if f == Field::Page { "PAGE" } else { "NUMPAGES" };
            let _ = write!(
                out,
                "<w:r><w:rPr>{props}</w:rPr><w:fldChar w:fldCharType=\"begin\"/></w:r>\
                 <w:r><w:rPr>{props}</w:rPr><w:instrText xml:space=\"preserve\"> {instr} </w:instrText></w:r>\
                 <w:r><w:rPr>{props}</w:rPr><w:fldChar w:fldCharType=\"separate\"/></w:r>\
                 <w:r><w:rPr>{props}</w:rPr><w:t>{}</w:t></w:r>\
                 <w:r><w:rPr>{props}</w:rPr><w:fldChar w:fldCharType=\"end\"/></w:r>",
                xml_escape(&r.text)
            );
        } else if r.text == "\t" {
            let _ = write!(out, "<w:r><w:rPr>{props}</w:rPr><w:tab/></w:r>");
        } else if !r.text.is_empty() {
            let _ = write!(
                out,
                "<w:r><w:rPr>{props}</w:rPr><w:t xml:space=\"preserve\">{}</w:t></w:r>",
                xml_escape(&r.text)
            );
        }
    }
    out
}

fn jc(a: Align) -> &'static str {
    match a {
        Align::Left => "",
        Align::Right => "<w:jc w:val=\"right\"/>",
        Align::Center => "<w:jc w:val=\"center\"/>",
        Align::Justify => "<w:jc w:val=\"both\"/>",
    }
}

/// Imágenes que acabarán en `word/media`.
#[derive(Default)]
struct Media {
    files: Vec<(String, Vec<u8>)>,
    exts: BTreeSet<&'static str>,
}

impl Media {
    fn add(&mut self, img: &ImageData) -> String {
        let n = self.files.len() + 1;
        let name = format!("image{n}.{}", img.ext);
        self.exts.insert(img.ext);
        self.files.push((name, img.data.clone()));
        format!("rIdImg{n}")
    }
}

/// Posición horizontal de una columna dentro de la página.
#[derive(Clone, Copy)]
struct ParaCtx {
    /// x absoluta del borde izquierdo de la columna de Word.
    origin: f32,
    /// x absoluta del borde derecho de la columna de Word.
    right: f32,
}

fn para_xml(p: &Paragraph, cx: ParaCtx, before: f32, tail: &str) -> String {
    let mut ppr = String::from("<w:widowControl w:val=\"0\"/>");
    if let Some(l) = p.list {
        let num = if l.kind == ListKind::Bullet { 1 } else { l.id + 1 };
        let _ = write!(
            ppr,
            "<w:numPr><w:ilvl w:val=\"0\"/><w:numId w:val=\"{num}\"/></w:numPr>"
        );
    }
    if !p.tabs.is_empty() {
        ppr.push_str("<w:tabs>");
        for (x, right) in &p.tabs {
            let _ = write!(
                ppr,
                "<w:tab w:val=\"{}\" w:pos=\"{}\"/>",
                if *right { "right" } else { "left" },
                twips((x - cx.origin).max(0.0))
            );
        }
        ppr.push_str("</w:tabs>");
    }
    let line = twips(p.line_pitch.max(1.0));
    let _ = write!(
        ppr,
        "<w:spacing w:before=\"{}\" w:after=\"0\" w:line=\"{line}\" w:lineRule=\"exact\"/>",
        twips(before)
    );
    let left = twips((p.left_x - cx.origin).max(0.0));
    let right_ind = if p.lines >= 2 {
        twips((cx.right - (p.bbox.x1 + 3.0)).max(0.0))
    } else {
        0
    };
    let delta = p.first_x - p.left_x;
    let first = if delta < -0.5 {
        format!(" w:hanging=\"{}\"", twips(-delta))
    } else if delta > 0.5 {
        format!(" w:firstLine=\"{}\"", twips(delta))
    } else {
        String::new()
    };
    let _ = write!(ppr, "<w:ind w:left=\"{left}\" w:right=\"{right_ind}\"{first}/>");
    ppr.push_str(jc(p.align));
    if p.list.is_some()
        && let Some(r) = p.runs.first()
    {
        // La marca de párrafo da el tipo y tamaño del número o la viñeta.
        let _ = write!(ppr, "<w:rPr>{}</w:rPr>", run_props(r));
    }
    ppr.push_str(tail);
    format!("<w:p><w:pPr>{ppr}</w:pPr>{}</w:p>", runs_xml(&p.runs))
}

fn spacer_xml(height: f32, tail: &str) -> String {
    format!(
        "<w:p><w:pPr><w:widowControl w:val=\"0\"/><w:spacing w:before=\"0\" w:after=\"0\" w:line=\"{}\" w:lineRule=\"exact\"/>{tail}</w:pPr></w:p>",
        twips(height.max(1.0))
    )
}

fn table_xml(t: &Table, cx: ParaCtx) -> String {
    let mut widths: Vec<i32> = t.cols.iter().map(|(l, r)| twips(r - l).max(200)).collect();
    if let Some(first) = widths.first_mut() {
        *first += 40;
    }
    if let Some(last) = widths.last_mut() {
        *last += 40;
    }
    let total: i32 = widths.iter().sum();
    let ind = twips(t.cols[0].0 - 2.0 - cx.origin).max(0);
    let border = |side: &str| format!("<w:{side} w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"BFBFBF\"/>");
    let mut s = format!(
        "<w:tbl><w:tblPr><w:tblW w:w=\"{total}\" w:type=\"dxa\"/><w:tblInd w:w=\"{ind}\" w:type=\"dxa\"/>\
         <w:tblBorders>{}{}{}{}{}{}</w:tblBorders><w:tblLayout w:type=\"fixed\"/>\
         <w:tblCellMar><w:top w:w=\"0\" w:type=\"dxa\"/><w:left w:w=\"40\" w:type=\"dxa\"/>\
         <w:bottom w:w=\"0\" w:type=\"dxa\"/><w:right w:w=\"40\" w:type=\"dxa\"/></w:tblCellMar></w:tblPr><w:tblGrid>",
        border("top"),
        border("left"),
        border("bottom"),
        border("right"),
        border("insideH"),
        border("insideV"),
    );
    for w in &widths {
        let _ = write!(s, "<w:gridCol w:w=\"{w}\"/>");
    }
    s.push_str("</w:tblGrid>");
    for (ri, row) in t.rows.iter().enumerate() {
        let _ = write!(
            s,
            "<w:tr><w:trPr><w:cantSplit/><w:trHeight w:val=\"{}\" w:hRule=\"atLeast\"/></w:trPr>",
            twips(t.row_heights[ri])
        );
        let mut col = 0;
        for cell in row {
            let w: i32 = widths[col..(col + cell.span).min(widths.len())].iter().sum();
            let span = if cell.span > 1 {
                format!("<w:gridSpan w:val=\"{}\"/>", cell.span)
            } else {
                String::new()
            };
            let _ = write!(s, "<w:tc><w:tcPr><w:tcW w:w=\"{w}\" w:type=\"dxa\"/>{span}</w:tcPr>");
            let _ = write!(
                s,
                "<w:p><w:pPr><w:widowControl w:val=\"0\"/><w:spacing w:before=\"0\" w:after=\"0\"/>{}</w:pPr>{}</w:p></w:tc>",
                jc(cell.align),
                runs_xml(&cell.runs)
            );
            col += cell.span;
        }
        s.push_str("</w:tr>");
    }
    s.push_str("</w:tbl>");
    s
}

fn image_xml(img: &ImageData, rid: &str, id: usize, cx: ParaCtx, before: f32, tail: &str) -> String {
    let w = crate::util::emu(img.bbox.x1 - img.bbox.x0);
    let h = crate::util::emu(img.bbox.y1 - img.bbox.y0);
    let left = twips((img.bbox.x0 - cx.origin).max(0.0));
    format!(
        "<w:p><w:pPr><w:widowControl w:val=\"0\"/><w:spacing w:before=\"{}\" w:after=\"0\" w:line=\"240\" w:lineRule=\"auto\"/>\
         <w:ind w:left=\"{left}\"/>{tail}</w:pPr><w:r><w:drawing>\
         <wp:inline distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\"><wp:extent cx=\"{w}\" cy=\"{h}\"/>\
         <wp:docPr id=\"{id}\" name=\"Imagen {id}\"/><a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/picture\">\
         <pic:pic><pic:nvPicPr><pic:cNvPr id=\"{id}\" name=\"Imagen {id}\"/><pic:cNvPicPr/></pic:nvPicPr>\
         <pic:blipFill><a:blip r:embed=\"{rid}\"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill>\
         <pic:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{w}\" cy=\"{h}\"/></a:xfrm>\
         <a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline>\
         </w:drawing></w:r></w:p>",
        twips(before)
    )
}

/// Párrafos de una columna: calcula el espacio anterior de cada uno siguiendo el cursor vertical.
fn column_xml(els: &[Element], cx: ParaCtx, start: f32, media: &mut Media, next_id: &mut usize) -> String {
    let mut out = String::new();
    let mut cursor = start;
    for el in els {
        match el {
            Element::Paragraph(p) => {
                let n = p.lines.max(1) as f32;
                let box_h = ((p.bbox.y1 - p.bbox.y0) - (n - 1.0) * p.line_pitch).max(1.0);
                let extra = (p.line_pitch - box_h).max(0.0);
                let begin = p.bbox.y0 - 0.8 * extra;
                let before = (begin - cursor).max(0.0);
                cursor += before + n * p.line_pitch;
                out.push_str(&para_xml(p, cx, before, ""));
            }
            Element::Table(t) => {
                let gap = t.bbox.y0 - cursor;
                if gap > 3.0 {
                    out.push_str(&spacer_xml(gap, ""));
                }
                out.push_str(&table_xml(t, cx));
                // Word exige un párrafo tras cada tabla; este es el mínimo posible.
                out.push_str(&spacer_xml(1.0, ""));
                cursor = t.bbox.y0 + t.row_heights.iter().sum::<f32>() + 1.0;
            }
            Element::Image(img) => {
                let before = (img.bbox.y0 - cursor).max(0.0);
                let rid = media.add(img);
                *next_id += 1;
                out.push_str(&image_xml(img, &rid, *next_id, cx, before, ""));
                cursor = img.bbox.y1 + 3.0;
            }
        }
    }
    out
}

fn sect_pr(page: &PageLayout, sec: &Section, first_of_page: bool, layout: &Layout) -> String {
    let m = &page.margins;
    let mut s = String::from("<w:sectPr>");
    if layout.header.is_some() && !page.scanned {
        s.push_str("<w:headerReference w:type=\"default\" r:id=\"rIdHdr1\"/>");
    }
    if layout.footer.is_some() && !page.scanned {
        s.push_str("<w:footerReference w:type=\"default\" r:id=\"rIdFtr1\"/>");
    }
    let _ = write!(
        s,
        "<w:type w:val=\"{}\"/><w:pgSz w:w=\"{}\" w:h=\"{}\"/>\
         <w:pgMar w:top=\"{}\" w:right=\"{}\" w:bottom=\"{}\" w:left=\"{}\" w:header=\"{}\" w:footer=\"{}\" w:gutter=\"0\"/>",
        if first_of_page { "nextPage" } else { "continuous" },
        twips(page.width),
        twips(page.height),
        twips(m.top),
        twips(m.right),
        twips(m.bottom),
        twips(m.left),
        twips(layout.header.as_ref().map_or(0.0, |b| b.distance)),
        twips(layout.footer.as_ref().map_or(0.0, |b| b.distance)),
    );
    if sec.columns.len() == 2 {
        let gl = sec.columns[0].x1;
        let rx = sec.columns[1].x0;
        let w0 = twips((gl + 3.0 - m.left).max(10.0));
        let space = twips((rx - gl - 3.0).max(1.0));
        let w1 = twips((page.width - m.right - rx).max(10.0));
        let _ = write!(
            s,
            "<w:cols w:num=\"2\" w:space=\"{space}\" w:equalWidth=\"0\"><w:col w:w=\"{w0}\" w:space=\"{space}\"/><w:col w:w=\"{w1}\"/></w:cols>"
        );
    } else {
        s.push_str("<w:cols w:space=\"0\"/>");
    }
    s.push_str("</w:sectPr>");
    s
}

fn body_xml(layout: &Layout, media: &mut Media) -> String {
    let mut body = String::new();
    let mut next_id = 0usize;
    let total_sections: usize = layout.pages.iter().map(|p| p.sections.len().max(1)).sum();
    let mut seen = 0usize;
    for page in &layout.pages {
        // Página sin contenido: una sola sección vacía para conservar la hoja.
        let empty = Section {
            columns: vec![ColumnSpec {
                x0: 0.0,
                x1: page.width,
            }],
            content: vec![Vec::new()],
            top: 0.0,
        };
        let sections: Vec<&Section> = if page.sections.is_empty() {
            vec![&empty]
        } else {
            page.sections.iter().collect()
        };
        for (si, sec) in sections.iter().enumerate() {
            seen += 1;
            let last_of_doc = seen == total_sections;
            let sp = sect_pr(page, sec, si == 0, layout);
            let origin_for = |k: usize| if k == 0 { page.margins.left } else { sec.columns[k].x0 };
            let right_for = |k: usize| {
                if sec.columns.len() == 2 && k == 0 {
                    sec.columns[0].x1 + 3.0
                } else {
                    page.width - page.margins.right
                }
            };
            let ncols = sec.content.len();
            for (k, els) in sec.content.iter().enumerate() {
                let cx = ParaCtx {
                    origin: origin_for(k),
                    right: right_for(k),
                };
                body.push_str(&column_xml(els, cx, sec.top, media, &mut next_id));
                if k + 1 < ncols {
                    // Salto de columna en un párrafo mínimo.
                    body.push_str(
                        "<w:p><w:pPr><w:widowControl w:val=\"0\"/><w:spacing w:before=\"0\" w:after=\"0\" w:line=\"20\" w:lineRule=\"exact\"/></w:pPr>\
                         <w:r><w:br w:type=\"column\"/></w:r></w:p>",
                    );
                }
            }
            if last_of_doc {
                body.push_str(&sp);
            } else {
                // La sección termina en un párrafo vacío que lleva sus propiedades.
                body.push_str(&spacer_xml(1.0, &sp));
            }
        }
    }
    body
}

fn band_xml(band: &Band, tag: &str, origin: f32, right: f32) -> String {
    let mut body = String::new();
    let mut cursor = band.paragraphs.first().map_or(0.0, |p| p.bbox.y0);
    let cx = ParaCtx { origin, right };
    for p in &band.paragraphs {
        let n = p.lines.max(1) as f32;
        let before = (p.bbox.y0 - cursor).max(0.0);
        cursor = p.bbox.y0 + n * p.line_pitch;
        body.push_str(&para_xml(p, cx, before, ""));
    }
    if body.is_empty() {
        body.push_str("<w:p/>");
    }
    format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><w:{tag} {NS}>{body}</w:{tag}>")
}

fn numbering_xml(layout: &Layout) -> String {
    let mut ids: BTreeSet<u32> = BTreeSet::new();
    let mut starts = std::collections::BTreeMap::new();
    let collect = |p: &Paragraph, ids: &mut BTreeSet<u32>, starts: &mut std::collections::BTreeMap<u32, u32>| {
        if let Some(l) = p.list
            && l.kind == ListKind::Decimal
        {
            ids.insert(l.id);
            starts.entry(l.id).or_insert(l.start);
        }
    };
    for page in &layout.pages {
        for e in page.elements() {
            if let Element::Paragraph(p) = e {
                collect(p, &mut ids, &mut starts);
            }
        }
    }
    for b in layout.header.iter().chain(layout.footer.iter()) {
        for p in &b.paragraphs {
            collect(p, &mut ids, &mut starts);
        }
    }
    let mut s = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><w:numbering {NS}>\
         <w:abstractNum w:abstractNumId=\"1\"><w:multiLevelType w:val=\"hybridMultilevel\"/>\
         <w:lvl w:ilvl=\"0\"><w:start w:val=\"1\"/><w:numFmt w:val=\"bullet\"/><w:lvlText w:val=\"•\"/><w:lvlJc w:val=\"left\"/>\
         <w:pPr><w:ind w:left=\"720\" w:hanging=\"360\"/></w:pPr><w:rPr><w:rFonts w:ascii=\"Arial\" w:hAnsi=\"Arial\" w:cs=\"Arial\"/></w:rPr></w:lvl></w:abstractNum>\
         <w:abstractNum w:abstractNumId=\"2\"><w:multiLevelType w:val=\"hybridMultilevel\"/>\
         <w:lvl w:ilvl=\"0\"><w:start w:val=\"1\"/><w:numFmt w:val=\"decimal\"/><w:lvlText w:val=\"%1.\"/><w:lvlJc w:val=\"left\"/>\
         <w:pPr><w:ind w:left=\"720\" w:hanging=\"360\"/></w:pPr></w:lvl></w:abstractNum>\
         <w:num w:numId=\"1\"><w:abstractNumId w:val=\"1\"/></w:num>"
    );
    for id in ids {
        let _ = write!(
            s,
            "<w:num w:numId=\"{}\"><w:abstractNumId w:val=\"2\"/><w:lvlOverride w:ilvl=\"0\"><w:startOverride w:val=\"{}\"/></w:lvlOverride></w:num>",
            id + 1,
            starts[&id]
        );
    }
    s.push_str("</w:numbering>");
    s
}

const STYLES: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<w:styles xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">\
<w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii=\"Arial\" w:hAnsi=\"Arial\" w:cs=\"Arial\" w:eastAsia=\"Arial\"/>\
<w:sz w:val=\"22\"/><w:szCs w:val=\"22\"/><w:lang w:val=\"es-ES\"/></w:rPr></w:rPrDefault>\
<w:pPrDefault><w:pPr><w:spacing w:after=\"0\" w:line=\"240\" w:lineRule=\"auto\"/></w:pPr></w:pPrDefault></w:docDefaults>\
<w:style w:type=\"paragraph\" w:default=\"1\" w:styleId=\"Normal\"><w:name w:val=\"Normal\"/><w:qFormat/></w:style>\
<w:style w:type=\"table\" w:default=\"1\" w:styleId=\"TableNormal\"><w:name w:val=\"Normal Table\"/><w:uiPriority w:val=\"99\"/>\
<w:semiHidden/><w:tblPr><w:tblInd w:w=\"0\" w:type=\"dxa\"/><w:tblCellMar><w:top w:w=\"0\" w:type=\"dxa\"/>\
<w:left w:w=\"108\" w:type=\"dxa\"/><w:bottom w:w=\"0\" w:type=\"dxa\"/><w:right w:w=\"108\" w:type=\"dxa\"/></w:tblCellMar></w:tblPr></w:style>\
</w:styles>";

const SETTINGS: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<w:settings xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">\
<w:compat><w:compatSetting w:name=\"compatibilityMode\" w:uri=\"http://schemas.microsoft.com/office/word\" w:val=\"15\"/></w:compat></w:settings>";

/// Empaqueta el modelo como DOCX.
pub(crate) fn write_docx(layout: &Layout, title: &str) -> Result<Vec<u8>> {
    let mut media = Media::default();
    let body = body_xml(layout, &mut media);
    let document = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><w:document {NS}><w:body>{body}</w:body></w:document>"
    );

    let first = layout.pages.first();
    let (origin, right) = first.map_or((0.0, 0.0), |p| (p.margins.left, p.width - p.margins.right));
    let mut pkg = Package::new();

    let mut ct = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
         <Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
         <Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
         <Default Extension=\"xml\" ContentType=\"application/xml\"/>",
    );
    for ext in &media.exts {
        let _ = write!(ct, "<Default Extension=\"{ext}\" ContentType=\"image/{ext}\"/>");
    }
    let wml = "application/vnd.openxmlformats-officedocument.wordprocessingml";
    let _ = write!(
        ct,
        "<Override PartName=\"/word/document.xml\" ContentType=\"{wml}.document.main+xml\"/>\
         <Override PartName=\"/word/styles.xml\" ContentType=\"{wml}.styles+xml\"/>\
         <Override PartName=\"/word/numbering.xml\" ContentType=\"{wml}.numbering+xml\"/>\
         <Override PartName=\"/word/settings.xml\" ContentType=\"{wml}.settings+xml\"/>\
         <Override PartName=\"/docProps/core.xml\" ContentType=\"application/vnd.openxmlformats-package.core-properties+xml\"/>"
    );
    if layout.header.is_some() {
        let _ = write!(
            ct,
            "<Override PartName=\"/word/header1.xml\" ContentType=\"{wml}.header+xml\"/>"
        );
    }
    if layout.footer.is_some() {
        let _ = write!(
            ct,
            "<Override PartName=\"/word/footer1.xml\" ContentType=\"{wml}.footer+xml\"/>"
        );
    }
    ct.push_str("</Types>");
    pkg.add("[Content_Types].xml", ct.as_bytes())?;

    pkg.add(
        "_rels/.rels",
        b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
          <Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
          <Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"word/document.xml\"/>\
          <Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties\" Target=\"docProps/core.xml\"/>\
          </Relationships>",
    )?;
    let core = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
         <cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" \
         xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><dc:title>{}</dc:title><dc:creator>Pliego</dc:creator></cp:coreProperties>",
        xml_escape(title)
    );
    pkg.add("docProps/core.xml", core.as_bytes())?;

    let rel = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
    let mut rels = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
         <Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
         <Relationship Id=\"rIdSty\" Type=\"{rel}/styles\" Target=\"styles.xml\"/>\
         <Relationship Id=\"rIdNum\" Type=\"{rel}/numbering\" Target=\"numbering.xml\"/>\
         <Relationship Id=\"rIdSet\" Type=\"{rel}/settings\" Target=\"settings.xml\"/>"
    );
    if layout.header.is_some() {
        let _ = write!(
            rels,
            "<Relationship Id=\"rIdHdr1\" Type=\"{rel}/header\" Target=\"header1.xml\"/>"
        );
    }
    if layout.footer.is_some() {
        let _ = write!(
            rels,
            "<Relationship Id=\"rIdFtr1\" Type=\"{rel}/footer\" Target=\"footer1.xml\"/>"
        );
    }
    for (i, (name, _)) in media.files.iter().enumerate() {
        let _ = write!(
            rels,
            "<Relationship Id=\"rIdImg{}\" Type=\"{rel}/image\" Target=\"media/{name}\"/>",
            i + 1
        );
    }
    rels.push_str("</Relationships>");
    pkg.add("word/_rels/document.xml.rels", rels.as_bytes())?;

    pkg.add("word/document.xml", document.as_bytes())?;
    pkg.add(
        "word/styles.xml",
        match layout.lang {
            pliego_core::Lang::Es => STYLES.to_string(),
            pliego_core::Lang::En => STYLES.replace("w:val=\"es-ES\"", "w:val=\"en-US\""),
        }
        .as_bytes(),
    )?;
    pkg.add("word/settings.xml", SETTINGS.as_bytes())?;
    pkg.add("word/numbering.xml", numbering_xml(layout).as_bytes())?;
    if let Some(h) = &layout.header {
        pkg.add("word/header1.xml", band_xml(h, "hdr", origin, right).as_bytes())?;
    }
    if let Some(f) = &layout.footer {
        pkg.add("word/footer1.xml", band_xml(f, "ftr", origin, right).as_bytes())?;
    }
    for (name, data) in &media.files {
        pkg.add(&format!("word/media/{name}"), data)?;
    }
    pkg.finish()
}
