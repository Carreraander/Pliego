use mupdf::color::AnnotationColor;
use mupdf::pdf::{LineEndingStyle, PdfAnnotation, PdfAnnotationType, PdfDocument, PdfObject, PdfPage};
use mupdf::{Point, Quad, Rect};
use pliego_core::{Error, Result};

use crate::types::*;

fn pe(e: mupdf::Error) -> Error {
    Error::Pdf(e.to_string())
}

fn pt(p: PointPts) -> Point {
    Point::new(p[0], p[1])
}

fn quad(q: QuadPts) -> Quad {
    Quad::new(pt([q[0], q[1]]), pt([q[2], q[3]]), pt([q[4], q[5]]), pt([q[6], q[7]]))
}

fn quad_pts(q: &Quad) -> QuadPts {
    [q.ul.x, q.ul.y, q.ur.x, q.ur.y, q.ll.x, q.ll.y, q.lr.x, q.lr.y]
}

fn rect(r: RectPts) -> Rect {
    Rect::new(r[0], r[1], r[2], r[3])
}

fn rgb(c: Rgb) -> AnnotationColor {
    AnnotationColor::Rgb {
        red: c[0],
        green: c[1],
        blue: c[2],
    }
}

fn from_color(c: Option<AnnotationColor>) -> Option<Rgb> {
    match c? {
        AnnotationColor::Gray(g) => Some([g, g, g]),
        AnnotationColor::Rgb { red, green, blue } => Some([red, green, blue]),
        AnnotationColor::Cmyk {
            cyan,
            magenta,
            yellow,
            key,
        } => Some([
            (1.0 - cyan) * (1.0 - key),
            (1.0 - magenta) * (1.0 - key),
            (1.0 - yellow) * (1.0 - key),
        ]),
    }
}

fn line_end(e: LineEnd) -> LineEndingStyle {
    match e {
        LineEnd::None => LineEndingStyle::None,
        LineEnd::Square => LineEndingStyle::Square,
        LineEnd::Circle => LineEndingStyle::Circle,
        LineEnd::Diamond => LineEndingStyle::Diamond,
        LineEnd::OpenArrow => LineEndingStyle::OpenArrow,
        LineEnd::ClosedArrow => LineEndingStyle::ClosedArrow,
        LineEnd::Butt => LineEndingStyle::Butt,
        LineEnd::Slash => LineEndingStyle::Slash,
    }
}

fn kind_of(t: PdfAnnotationType) -> AnnotKind {
    use PdfAnnotationType as T;
    match t {
        T::Text => AnnotKind::Note,
        T::FreeText => AnnotKind::FreeText,
        T::Highlight => AnnotKind::Highlight,
        T::Underline => AnnotKind::Underline,
        T::StrikeOut => AnnotKind::StrikeOut,
        T::Squiggly => AnnotKind::Squiggly,
        T::Line => AnnotKind::Line,
        T::Square => AnnotKind::Square,
        T::Circle => AnnotKind::Circle,
        T::Polygon => AnnotKind::Polygon,
        T::PolyLine => AnnotKind::PolyLine,
        T::Ink => AnnotKind::Ink,
        T::Stamp => AnnotKind::Stamp,
        T::FileAttachment => AnnotKind::FileAttachment,
        T::Link => AnnotKind::Link,
        T::Caret => AnnotKind::Caret,
        T::Redact => AnnotKind::Redact,
        _ => AnnotKind::Other,
    }
}

/// Fecha PDF (`D:AAAAMMDDhhmmssZ`) del instante actual, en UTC.
fn pdf_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Algoritmo de fecha civil de Howard Hinnant.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "D:{y:04}{m:02}{d:02}{:02}{:02}{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

fn dict_string(obj: &PdfObject, key: &str) -> Option<String> {
    let v = obj.get_dict(key).ok()??;
    if v.is_string().ok()? {
        v.as_string().ok()
    } else if v.is_name().ok()? {
        v.as_name().ok().map(|n| String::from_utf8_lossy(&n).into_owned())
    } else {
        None
    }
}

fn put_string(obj: &mut PdfObject, key: &str, value: &str) -> Result<()> {
    obj.dict_put(key, PdfObject::new_string(value).map_err(pe)?).map_err(pe)
}

fn put_name(obj: &mut PdfObject, key: &str, value: &str) -> Result<()> {
    obj.dict_put(key, PdfObject::new_name(value).map_err(pe)?).map_err(pe)
}

fn r4(r: Rect) -> RectPts {
    [r.x0, r.y0, r.x1, r.y1]
}

fn page_of(doc: &PdfDocument, page: usize) -> Result<PdfPage> {
    let n = doc.page_count().map_err(pe)? as usize;
    if page >= n {
        return Err(Error::InvalidInput(format!(
            "la página {} no existe (el documento tiene {n})",
            page + 1
        )));
    }
    doc.load_pdf_page(page as i32).map_err(pe)
}

fn find(page: &PdfPage, xref: i32) -> Result<PdfAnnotation> {
    for a in page.annotations() {
        if a.xref().map_err(pe)? == xref {
            return Ok(a);
        }
    }
    Err(Error::InvalidInput(format!(
        "no se encontró la anotación {xref} en esa página"
    )))
}

fn info_of(page_no: usize, a: &PdfAnnotation) -> Result<Option<AnnotInfo>> {
    let t = a.r#type().map_err(pe)?;
    if matches!(t, PdfAnnotationType::Popup | PdfAnnotationType::Widget) {
        return Ok(None);
    }
    let kind = kind_of(t);
    let obj = a.object();
    let reply_to = obj.get_dict("IRT").ok().flatten().and_then(|o| o.as_indirect().ok());
    let mut info = AnnotInfo {
        id: AnnotId {
            page: page_no,
            xref: a.xref().map_err(pe)?,
        },
        kind,
        rect: r4(a.rect().map_err(pe)?),
        style: Style {
            color: from_color(a.color().map_err(pe)?),
            interior_color: from_color(a.interior_color().ok().flatten()),
            opacity: Some(a.opacity().map_err(pe)?),
            border_width: a.border_width().ok(),
        },
        author: a.author().map_err(pe)?.map(str::to_owned),
        subject: dict_string(&obj, "Subj"),
        contents: a.contents().map_err(pe)?.map(str::to_owned),
        created: dict_string(&obj, "CreationDate"),
        modified: dict_string(&obj, "M"),
        quads: Vec::new(),
        points: Vec::new(),
        strokes: Vec::new(),
        line: None,
        reply_to,
        state: dict_string(&obj, "State").and_then(|s| ReviewState::from_pdf_name(&s)),
    };
    match kind {
        AnnotKind::Highlight | AnnotKind::Underline | AnnotKind::StrikeOut | AnnotKind::Squiggly => {
            info.quads = a.quad_points().map_err(pe)?.iter().map(quad_pts).collect();
        }
        AnnotKind::Polygon | AnnotKind::PolyLine => {
            info.points = a.vertices().map_err(pe)?.iter().map(|p| [p.x, p.y]).collect();
        }
        AnnotKind::Ink => {
            info.strokes = a
                .ink_list()
                .map_err(pe)?
                .iter()
                .map(|s| s.iter().map(|p| [p.x, p.y]).collect())
                .collect();
        }
        AnnotKind::Line => {
            let (s, e) = a.line().map_err(pe)?;
            info.line = Some(([s.x, s.y], [e.x, e.y]));
        }
        _ => {}
    }
    Ok(Some(info))
}

/// Lista las anotaciones de una página (`Some(n)`) o de todo el documento (`None`).
/// Excluye los campos de formulario y las ventanas emergentes.
pub fn list(doc: &mut PdfDocument, page: Option<usize>) -> Result<Vec<AnnotInfo>> {
    let n = doc.page_count().map_err(pe)? as usize;
    let range = match page {
        Some(p) => p..p + 1,
        None => 0..n,
    };
    let mut out = Vec::new();
    for p in range {
        let pg = page_of(doc, p)?;
        for a in pg.annotations() {
            if let Some(i) = info_of(p, &a)? {
                out.push(i);
            }
        }
    }
    Ok(out)
}

fn apply_style(a: &mut PdfAnnotation, s: &Style) -> Result<()> {
    if let Some(c) = s.color {
        a.set_color(rgb(c)).map_err(pe)?;
    }
    if let Some(c) = s.interior_color {
        // Solo figuras cerradas y líneas admiten color interior (IC).
        if matches!(
            a.r#type().map_err(pe)?,
            PdfAnnotationType::Line
                | PdfAnnotationType::Square
                | PdfAnnotationType::Circle
                | PdfAnnotationType::Polygon
                | PdfAnnotationType::PolyLine
        ) {
            a.set_interior_color(rgb(c)).map_err(pe)?;
        }
    }
    if let Some(o) = s.opacity {
        a.set_opacity(o.clamp(0.0, 1.0)).map_err(pe)?;
    }
    if let Some(w) = s.border_width {
        // Solo algunos tipos admiten borde (BS); en el resto se ignora.
        if matches!(
            a.r#type().map_err(pe)?,
            PdfAnnotationType::FreeText
                | PdfAnnotationType::Line
                | PdfAnnotationType::Square
                | PdfAnnotationType::Circle
                | PdfAnnotationType::Polygon
                | PdfAnnotationType::PolyLine
                | PdfAnnotationType::Ink
        ) {
            a.set_border_width(w.max(0.0)).map_err(pe)?;
        }
    }
    Ok(())
}

fn apply_meta(
    a: &mut PdfAnnotation,
    author: &Option<String>,
    subject: &Option<String>,
    contents: &Option<String>,
) -> Result<()> {
    if let Some(x) = author {
        a.set_author(x).map_err(pe)?;
    }
    if let Some(x) = contents {
        a.set_contents(x).map_err(pe)?;
    }
    if let Some(x) = subject {
        put_string(&mut a.object(), "Subj", x)?;
    }
    Ok(())
}

/// Crea una anotación, genera su apariencia y devuelve su identificador.
pub fn add(doc: &mut PdfDocument, spec: &AnnotSpec) -> Result<AnnotId> {
    let mut page = page_of(doc, spec.page)?;
    let mut a = match &spec.geometry {
        Geometry::Note { at } => {
            let r = Rect::new(at[0], at[1], at[0] + 20.0, at[1] + 20.0);
            let mut a = page
                .add_text_annotation(r, spec.contents.as_deref().unwrap_or(""))
                .map_err(pe)?;
            a.set_icon_name("Comment").map_err(pe)?;
            a
        }
        Geometry::FreeText { rect: r, font_size } => {
            let mut a = page
                .add_free_text_annotation(rect(*r), spec.contents.as_deref().unwrap_or(""))
                .map_err(pe)?;
            a.set_default_appearance("Helv", *font_size, spec.style.color.map(rgb))
                .map_err(pe)?;
            a
        }
        Geometry::Highlight { quads } => page
            .add_highlight_annotation(quads.iter().map(|q| quad(*q)).collect::<Vec<_>>())
            .map_err(pe)?,
        Geometry::Underline { quads } => page
            .add_underline_annotation(quads.iter().map(|q| quad(*q)).collect::<Vec<_>>())
            .map_err(pe)?,
        Geometry::StrikeOut { quads } => page
            .add_strikeout_annotation(quads.iter().map(|q| quad(*q)).collect::<Vec<_>>())
            .map_err(pe)?,
        Geometry::Squiggly { quads } => page
            .add_squiggly_annotation(quads.iter().map(|q| quad(*q)).collect::<Vec<_>>())
            .map_err(pe)?,
        Geometry::Line {
            start,
            end,
            start_style,
            end_style,
        } => {
            let mut a = page.add_line_annotation(pt(*start), pt(*end)).map_err(pe)?;
            a.set_line_ending_styles(line_end(*start_style), line_end(*end_style))
                .map_err(pe)?;
            a
        }
        Geometry::Square { rect: r } => page.add_square_annotation(rect(*r)).map_err(pe)?,
        Geometry::Circle { rect: r } => page.add_circle_annotation(rect(*r)).map_err(pe)?,
        Geometry::Polygon { points } => page
            .add_polygon_annotation(points.iter().map(|p| pt(*p)).collect::<Vec<_>>())
            .map_err(pe)?,
        Geometry::PolyLine { points } => page
            .add_polyline_annotation(points.iter().map(|p| pt(*p)).collect::<Vec<_>>())
            .map_err(pe)?,
        Geometry::Ink { strokes } => page
            .add_ink_annotation(
                strokes
                    .iter()
                    .map(|s| s.iter().map(|p| pt(*p)).collect::<Vec<_>>())
                    .collect::<Vec<_>>(),
            )
            .map_err(pe)?,
    };
    apply_style(&mut a, &spec.style)?;
    apply_meta(&mut a, &spec.author, &spec.subject, &spec.contents)?;
    let mut obj = a.object();
    let now = pdf_now();
    put_string(&mut obj, "CreationDate", &now)?;
    put_string(&mut obj, "M", &now)?;
    if let Some(parent) = spec.reply_to {
        let ind = doc.new_indirect(parent.xref, 0).map_err(pe)?;
        obj.dict_put("IRT", ind).map_err(pe)?;
        put_name(&mut obj, "RT", "R")?;
        if let Some(s) = spec.state {
            put_string(&mut obj, "StateModel", "Review")?;
            put_string(&mut obj, "State", s.pdf_name())?;
        }
    }
    a.update().map_err(pe)?;
    let id = AnnotId {
        page: spec.page,
        xref: a.xref().map_err(pe)?,
    };
    Ok(id)
}

/// Modifica propiedades de una anotación y regenera su apariencia.
pub fn update(doc: &mut PdfDocument, id: AnnotId, patch: &AnnotPatch) -> Result<()> {
    let page = page_of(doc, id.page)?;
    let mut a = find(&page, id.xref)?;
    apply_style(&mut a, &patch.style)?;
    apply_meta(&mut a, &patch.author, &patch.subject, &patch.contents)?;
    if let Some([dx, dy]) = patch.move_by {
        let shift = |p: Point| Point::new(p.x + dx, p.y + dy);
        let t = a.r#type().map_err(pe)?;
        match t {
            PdfAnnotationType::Line => {
                let (s, e) = a.line().map_err(pe)?;
                a.set_line(shift(s), shift(e)).map_err(pe)?;
            }
            PdfAnnotationType::Polygon | PdfAnnotationType::PolyLine => {
                let v: Vec<Point> = a.vertices().map_err(pe)?.into_iter().map(shift).collect();
                a.set_vertices(v).map_err(pe)?;
            }
            PdfAnnotationType::Ink => {
                let strokes: Vec<Vec<Point>> = a
                    .ink_list()
                    .map_err(pe)?
                    .into_iter()
                    .map(|s| s.into_iter().map(shift).collect())
                    .collect();
                a.set_ink_list(strokes.iter().map(|s| s.iter().copied())).map_err(pe)?;
            }
            PdfAnnotationType::Highlight
            | PdfAnnotationType::Underline
            | PdfAnnotationType::StrikeOut
            | PdfAnnotationType::Squiggly => {
                let q: Vec<Quad> = a
                    .quad_points()
                    .map_err(pe)?
                    .iter()
                    .map(|q| Quad::new(shift(q.ul), shift(q.ur), shift(q.ll), shift(q.lr)))
                    .collect();
                a.set_quad_points(q).map_err(pe)?;
            }
            _ => {}
        }
        let r = a.rect().map_err(pe)?;
        a.set_rect(Rect::new(r.x0 + dx, r.y0 + dy, r.x1 + dx, r.y1 + dy))
            .map_err(pe)?;
    }
    put_string(&mut a.object(), "M", &pdf_now())?;
    a.update().map_err(pe)?;
    Ok(())
}

/// Borra una anotación y todas sus respuestas.
pub fn delete(doc: &mut PdfDocument, id: AnnotId) -> Result<()> {
    let mut page = page_of(doc, id.page)?;
    let mut doomed = vec![id.xref];
    // Respuestas, también las encadenadas.
    loop {
        let mut grew = false;
        for a in page.annotations() {
            let x = a.xref().map_err(pe)?;
            if doomed.contains(&x) {
                continue;
            }
            if let Some(p) = a
                .object()
                .get_dict("IRT")
                .ok()
                .flatten()
                .and_then(|o| o.as_indirect().ok())
                && doomed.contains(&p)
            {
                doomed.push(x);
                grew = true;
            }
        }
        if !grew {
            break;
        }
    }
    find(&page, id.xref)?;
    for x in doomed {
        let a = find(&page, x)?;
        page.delete_annotation(a).map_err(pe)?;
    }
    Ok(())
}
