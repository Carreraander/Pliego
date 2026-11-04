//! Validación propia de las reglas más comunes de PDF/A. No sustituye a veraPDF (validación completa).

use std::collections::HashSet;
use std::path::Path;

use mupdf::pdf::{PdfDocument, PdfObject};
use pliego_core::Result;
use serde::{Deserialize, Serialize};

use crate::Level;
use crate::io::{name_of, open_pdf, pdf_err, xref_of};
use crate::util::{array_items, entries, get};

const MAX_DEPTH: usize = 32;
const MAX_OBJECTS: usize = 200_000;

/// Regla incumplida (código estable).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rule {
    Encrypted,
    NoTrailerId,
    NoXmp,
    XmpWithoutPdfaId,
    XmpLevelMismatch,
    NoOutputIntent,
    OutputIntentWithoutProfile,
    FontNotEmbedded,
    FontWithoutUnicode,
    ForbiddenAction,
    PageAdditionalActions,
    Transparency,
    EmbeddedFile,
    AttachmentWithoutRelationship,
    LzwFilter,
    OptionalContent,
    AnnotationNotPrintable,
    AnnotationWithoutAppearance,
    ForbiddenAnnotation,
    ExternalStream,
    Truncated,
}

/// Un incumplimiento, con mensaje para personas.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdfaIssue {
    pub rule: Rule,
    /// Página (desde 0), si aplica.
    pub page: Option<usize>,
    pub message: String,
}

/// Resultado de [`validate`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Validation {
    pub level: Level,
    pub issues: Vec<PdfaIssue>,
}

impl Validation {
    /// ¿Cumple todas las reglas comprobadas por la validación propia?
    pub fn conforms(&self) -> bool {
        self.issues.is_empty()
    }
    pub fn has(&self, rule: Rule) -> bool {
        self.issues.iter().any(|i| i.rule == rule)
    }
}

/// Extrae el valor de una propiedad XMP, tanto en forma de elemento como de atributo.
pub(crate) fn xmp_value(xmp: &str, name: &str) -> Option<String> {
    let attr = format!("{name}=\"");
    if let Some(i) = xmp.find(&attr) {
        let rest = &xmp[i + attr.len()..];
        return rest.find('"').map(|e| rest[..e].trim().to_string());
    }
    let open = format!("<{name}>");
    let i = xmp.find(&open)?;
    let rest = &xmp[i + open.len()..];
    rest.find('<').map(|e| rest[..e].trim().to_string())
}

struct Ctx<'a> {
    doc: &'a PdfDocument,
    level: Level,
    issues: Vec<PdfaIssue>,
    seen: HashSet<i32>,
    budget: usize,
    truncated: bool,
    /// Evita repetir el mismo incumplimiento global (p. ej. transparencia) en cada página.
    reported: HashSet<(Rule, Option<usize>)>,
}

impl Ctx<'_> {
    fn add(&mut self, rule: Rule, page: Option<usize>, message: impl Into<String>) {
        self.issues.push(PdfaIssue {
            rule,
            page,
            message: message.into(),
        });
    }
    fn add_once(&mut self, rule: Rule, page: Option<usize>, message: impl Into<String>) {
        if self.reported.insert((rule, page)) {
            self.add(rule, page, message);
        }
    }
    /// Devuelve `false` si el objeto ya se visitó o se agotó el presupuesto.
    fn enter(&mut self, obj: &PdfObject) -> bool {
        if self.budget == 0 {
            self.truncated = true;
            return false;
        }
        self.budget -= 1;
        let x = xref_of(obj);
        x == 0 || self.seen.insert(x)
    }
}

fn page_label(page: Option<usize>) -> String {
    page.map(|p| format!(" (página {})", p + 1)).unwrap_or_default()
}

fn has_lzw(stream: &PdfObject) -> bool {
    let Some(f) = get(stream, "Filter") else { return false };
    let is_lzw = |o: &PdfObject| o.as_name().is_ok_and(|n| n == b"LZWDecode" || n == b"LZW");
    is_lzw(&f) || array_items(&f).iter().any(is_lzw)
}

fn check_stream_basics(ctx: &mut Ctx, stream: &PdfObject, page: Option<usize>) {
    if has_lzw(stream) {
        ctx.add_once(
            Rule::LzwFilter,
            page,
            format!("Usa compresión LZW, que PDF/A no permite{}.", page_label(page)),
        );
    }
    if get(stream, "F").is_some() {
        ctx.add_once(
            Rule::ExternalStream,
            page,
            format!("Un flujo de datos está en un archivo externo{}.", page_label(page)),
        );
    }
}

fn check_action(ctx: &mut Ctx, action: &PdfObject, page: Option<usize>, depth: usize) {
    if depth > 16 || !action.is_dict().unwrap_or(false) {
        return;
    }
    if let Some(s) = name_of(action, "S")
        && matches!(
            s.as_str(),
            "Launch"
                | "Sound"
                | "Movie"
                | "ResetForm"
                | "ImportData"
                | "JavaScript"
                | "Hide"
                | "SetOCGState"
                | "Rendition"
                | "Trans"
                | "GoTo3DView"
        )
    {
        ctx.add_once(
            Rule::ForbiddenAction,
            page,
            format!("Contiene una acción «{s}», que PDF/A no permite{}.", page_label(page)),
        );
    }
    if let Some(next) = get(action, "Next") {
        if next.is_array().unwrap_or(false) {
            for a in array_items(&next) {
                check_action(ctx, &a, page, depth + 1);
            }
        } else {
            check_action(ctx, &next, page, depth + 1);
        }
    }
}

fn check_additional_actions(ctx: &mut Ctx, holder: &PdfObject, page: Option<usize>) {
    if let Some(aa) = get(holder, "AA") {
        for (_, a) in entries(&aa) {
            check_action(ctx, &a, page, 0);
        }
    }
}

fn font_has_file(desc: &PdfObject) -> bool {
    ["FontFile", "FontFile2", "FontFile3"]
        .iter()
        .any(|k| get(desc, k).is_some())
}

fn check_font(ctx: &mut Ctx, font: &PdfObject, page: usize) {
    if !font.is_dict().unwrap_or(false) || !ctx.enter(font) {
        return;
    }
    let subtype = name_of(font, "Subtype").unwrap_or_default();
    let base = name_of(font, "BaseFont").unwrap_or_else(|| "(sin nombre)".into());
    let embedded = match subtype.as_str() {
        "Type3" => true,
        "Type0" => get(font, "DescendantFonts")
            .and_then(|d| array_items(&d).into_iter().next())
            .and_then(|d| get(&d, "FontDescriptor"))
            .is_some_and(|d| font_has_file(&d)),
        _ => get(font, "FontDescriptor").is_some_and(|d| font_has_file(&d)),
    };
    if !embedded {
        ctx.add(
            Rule::FontNotEmbedded,
            Some(page),
            format!("La fuente «{base}» no está incrustada (página {}).", page + 1),
        );
    }
    if ctx.level.requires_unicode() && (subtype == "Type0" || subtype == "Type3") && get(font, "ToUnicode").is_none() {
        ctx.add(
            Rule::FontWithoutUnicode,
            Some(page),
            format!(
                "La fuente «{base}» no tiene correspondencia a Unicode (página {}).",
                page + 1
            ),
        );
    }
}

fn check_gstate(ctx: &mut Ctx, gs: &PdfObject, page: usize) {
    if ctx.level.allows_transparency() || !ctx.enter(gs) {
        return;
    }
    let alpha = |k: &str| get(gs, k).and_then(|v| v.as_float().ok()).is_some_and(|v| v < 1.0);
    let smask = get(gs, "SMask").is_some_and(|s| s.as_name().map(|n| n != b"None").unwrap_or(true));
    let blend = get(gs, "BM").is_some_and(|b| {
        b.as_name()
            .map(|n| n != b"Normal" && n != b"Compatible")
            .unwrap_or(true)
    });
    if alpha("CA") || alpha("ca") || smask || blend {
        ctx.add_once(
            Rule::Transparency,
            Some(page),
            format!(
                "Usa transparencia, que {} no permite (página {}).",
                ctx.level.name(),
                page + 1
            ),
        );
    }
}

fn check_resources(ctx: &mut Ctx, res: &PdfObject, page: usize, depth: usize) {
    if depth > MAX_DEPTH || !res.is_dict().unwrap_or(false) {
        return;
    }
    if let Some(fonts) = get(res, "Font") {
        for (_, f) in entries(&fonts) {
            check_font(ctx, &f, page);
        }
    }
    if let Some(gs) = get(res, "ExtGState") {
        for (_, g) in entries(&gs) {
            check_gstate(ctx, &g, page);
        }
    }
    if let Some(xo) = get(res, "XObject") {
        for (_, x) in entries(&xo) {
            if !x.is_dict().unwrap_or(false) || !ctx.enter(&x) {
                continue;
            }
            check_stream_basics(ctx, &x, Some(page));
            match name_of(&x, "Subtype").as_deref() {
                Some("Image") => {
                    if !ctx.level.allows_transparency() && get(&x, "SMask").is_some() {
                        ctx.add_once(
                            Rule::Transparency,
                            Some(page),
                            format!(
                                "Una imagen tiene máscara suave (transparencia), que {} no permite (página {}).",
                                ctx.level.name(),
                                page + 1
                            ),
                        );
                    }
                }
                Some("Form") => {
                    if !ctx.level.allows_transparency()
                        && get(&x, "Group").is_some_and(|g| name_of(&g, "S").as_deref() == Some("Transparency"))
                    {
                        ctx.add_once(
                            Rule::Transparency,
                            Some(page),
                            format!(
                                "Un objeto de formulario es un grupo de transparencia (página {}).",
                                page + 1
                            ),
                        );
                    }
                    if let Some(r) = get(&x, "Resources") {
                        check_resources(ctx, &r, page, depth + 1);
                    }
                }
                _ => {}
            }
        }
    }
}

fn check_annotations(ctx: &mut Ctx, page_obj: &PdfObject, page: usize) {
    let Some(annots) = get(page_obj, "Annots") else { return };
    for a in array_items(&annots).into_iter().take(10_000) {
        if !a.is_dict().unwrap_or(false) {
            continue;
        }
        let sub = name_of(&a, "Subtype").unwrap_or_default();
        if matches!(sub.as_str(), "Movie" | "Sound" | "Screen" | "3D") {
            ctx.add(
                Rule::ForbiddenAnnotation,
                Some(page),
                format!(
                    "Contiene una anotación «{sub}», que PDF/A no permite (página {}).",
                    page + 1
                ),
            );
            continue;
        }
        if sub == "FileAttachment" && !ctx.level.allows_attachments() {
            ctx.add_once(
                Rule::EmbeddedFile,
                Some(page),
                format!(
                    "Tiene un archivo adjunto, que {} no permite (página {}).",
                    ctx.level.name(),
                    page + 1
                ),
            );
        }
        if let Some(act) = get(&a, "A") {
            check_action(ctx, &act, Some(page), 0);
        }
        check_additional_actions(ctx, &a, Some(page));
        if sub == "Popup" {
            continue;
        }
        let flags = get(&a, "F").and_then(|f| f.as_int().ok()).unwrap_or(0);
        // 1 = invisible, 2 = oculta, 4 = imprimir, 32 = no ver
        if flags & 4 == 0 || flags & (1 | 2 | 32) != 0 {
            ctx.add_once(
                Rule::AnnotationNotPrintable,
                Some(page),
                format!(
                    "Hay anotaciones que no se imprimen o están ocultas (página {}).",
                    page + 1
                ),
            );
        }
        if sub != "Link" && get(&a, "AP").and_then(|ap| get(&ap, "N")).is_none() {
            ctx.add_once(
                Rule::AnnotationWithoutAppearance,
                Some(page),
                format!("Hay anotaciones sin aspecto visual guardado (página {}).", page + 1),
            );
        }
    }
}

fn check_catalog(ctx: &mut Ctx) -> Result<()> {
    let level = ctx.level;
    let catalog = ctx.doc.catalog().map_err(pdf_err)?;
    let trailer = ctx.doc.trailer().map_err(pdf_err)?;

    if get(&trailer, "Encrypt").is_some() {
        ctx.add(
            Rule::Encrypted,
            None,
            "El documento está cifrado, y PDF/A no permite cifrado.",
        );
    }

    let id_ok = get(&trailer, "ID").is_some_and(|id| {
        id.is_array().unwrap_or(false)
            && array_items(&id)
                .iter()
                .filter(|s| s.is_string().unwrap_or(false))
                .count()
                >= 2
    });
    if !id_ok {
        ctx.add(
            Rule::NoTrailerId,
            None,
            "El trailer del archivo no tiene identificador (/ID) con dos cadenas, que PDF/A exige.",
        );
    }

    // Metadatos XMP con la identificación PDF/A.
    match get(&catalog, "Metadata").and_then(|m| m.read_stream().ok()) {
        None => ctx.add(Rule::NoXmp, None, "No tiene metadatos XMP."),
        Some(bytes) => {
            let xmp = String::from_utf8_lossy(&bytes);
            let part = xmp_value(&xmp, "pdfaid:part");
            let conf = xmp_value(&xmp, "pdfaid:conformance");
            match (part, conf) {
                (None, _) | (_, None) => ctx.add(
                    Rule::XmpWithoutPdfaId,
                    None,
                    "Los metadatos XMP no declaran que sea PDF/A (pdfaid:part y pdfaid:conformance).",
                ),
                (Some(p), Some(c)) => {
                    // Un documento 2u también es válido como 2b, pero no al revés.
                    let conf_ok =
                        c.eq_ignore_ascii_case(level.conformance()) || (level.conformance() == "B" && c == "U");
                    if p != level.part().to_string() || !conf_ok {
                        ctx.add(
                            Rule::XmpLevelMismatch,
                            None,
                            format!(
                                "Los metadatos declaran PDF/A-{p}{} pero se comprueba {}.",
                                c.to_ascii_lowercase(),
                                level.name()
                            ),
                        );
                    }
                }
            }
        }
    }

    // Intención de salida con perfil ICC.
    let intents = get(&catalog, "OutputIntents")
        .map(|o| array_items(&o))
        .unwrap_or_default();
    let pdfa_intent = intents.iter().find(|i| name_of(i, "S").as_deref() == Some("GTS_PDFA1"));
    match pdfa_intent {
        None => ctx.add(
            Rule::NoOutputIntent,
            None,
            "Falta la intención de salida PDF/A (OutputIntent GTS_PDFA1).",
        ),
        Some(i) if get(i, "DestOutputProfile").is_none() => ctx.add(
            Rule::OutputIntentWithoutProfile,
            None,
            "La intención de salida no incluye un perfil de color ICC.",
        ),
        Some(_) => {}
    }

    if !level.allows_transparency() && get(&catalog, "OCProperties").is_some() {
        ctx.add(
            Rule::OptionalContent,
            None,
            format!("Tiene capas (contenido opcional), que {} no permite.", level.name()),
        );
    }

    // Acciones globales: apertura, adicionales y JavaScript de nombres.
    if let Some(a) = get(&catalog, "OpenAction") {
        check_action(ctx, &a, None, 0);
    }
    check_additional_actions(ctx, &catalog, None);
    if let Some(names) = get(&catalog, "Names") {
        if get(&names, "JavaScript").is_some() {
            ctx.add(
                Rule::ForbiddenAction,
                None,
                "Contiene JavaScript de documento, que PDF/A no permite.",
            );
        }
        if get(&names, "EmbeddedFiles").is_some() {
            if !level.allows_attachments() {
                ctx.add(
                    Rule::EmbeddedFile,
                    None,
                    format!("Tiene archivos incrustados, que {} no permite.", level.name()),
                );
            } else if get(&catalog, "AF").is_none_or(|af| array_items(&af).is_empty()) {
                ctx.add(
                    Rule::AttachmentWithoutRelationship,
                    None,
                    "Los archivos adjuntos no están asociados al documento (falta AF con AFRelationship).",
                );
            }
        }
    }
    Ok(())
}

/// Valida un documento abierto contra `level`.
pub fn validate_doc(doc: &PdfDocument, level: Level) -> Result<Validation> {
    let mut ctx = Ctx {
        doc,
        level,
        issues: Vec::new(),
        seen: HashSet::new(),
        budget: MAX_OBJECTS,
        truncated: false,
        reported: HashSet::new(),
    };
    check_catalog(&mut ctx)?;
    for p in 0..doc.page_count().map_err(pdf_err)?.max(0) as usize {
        let page = doc.find_page(p as i32).map_err(pdf_err)?;
        if let Some(res) = page.get_dict_inheritable("Resources").map_err(pdf_err)? {
            check_resources(&mut ctx, &res, p, 0);
        }
        if let Some(g) = get(&page, "Group")
            && !level.allows_transparency()
            && name_of(&g, "S").as_deref() == Some("Transparency")
        {
            ctx.add_once(
                Rule::Transparency,
                Some(p),
                format!(
                    "La página usa un grupo de transparencia, que PDF/A-1 no permite (página {}).",
                    p + 1
                ),
            );
        }
        if let Some(c) = get(&page, "Contents") {
            let streams = if c.is_array().unwrap_or(false) {
                array_items(&c)
            } else {
                vec![c]
            };
            for s in streams.iter().take(1000) {
                check_stream_basics(&mut ctx, s, Some(p));
            }
        }
        check_additional_actions(&mut ctx, &page, Some(p));
        // ISO 19005-2/-3 6.5.2: la página no puede llevar la entrada /AA, ni vacía.
        if ctx.level.allows_transparency() && get(&page, "AA").is_some() {
            ctx.add(
                Rule::PageAdditionalActions,
                Some(p),
                format!(
                    "La página {} tiene acciones adicionales (/AA), que {} no permite.",
                    p + 1,
                    ctx.level.name()
                ),
            );
        }
        check_annotations(&mut ctx, &page, p);
    }
    if ctx.truncated {
        ctx.add(
            Rule::Truncated,
            None,
            "El documento es tan grande que solo se comprobó una parte; usa veraPDF para una validación completa.",
        );
    }
    Ok(Validation {
        level,
        issues: ctx.issues,
    })
}

/// Abre `input` y lo valida contra `level`.
pub fn validate(input: &Path, level: Level) -> Result<Validation> {
    let doc = open_pdf(input, None)?;
    validate_doc(&doc, level)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xmp_values() {
        let el = "<rdf:Description><pdfaid:part>2</pdfaid:part><pdfaid:conformance>B</pdfaid:conformance>";
        assert_eq!(xmp_value(el, "pdfaid:part").as_deref(), Some("2"));
        assert_eq!(xmp_value(el, "pdfaid:conformance").as_deref(), Some("B"));
        let at = r#"<rdf:Description pdfaid:part="1" pdfaid:conformance="B"/>"#;
        assert_eq!(xmp_value(at, "pdfaid:part").as_deref(), Some("1"));
        assert_eq!(xmp_value("<x/>", "pdfaid:part"), None);
    }
}
