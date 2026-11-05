//! Comprobador de accesibilidad (reglas habituales de PDF/UA y de lectores de pantalla).

use pliego_core::Result;
use pliego_pdf::{Object, PdfDocument, Permission};

use crate::io::{name_of, string_of};
use crate::report::{A11yIssue, A11yReport, IssueCode, Severity};
use crate::structure::{Tree, read_tree};

/// Documentos de al menos estas páginas deberían tener marcadores.
const BOOKMARK_MIN_PAGES: usize = 20;

struct Collector(Vec<A11yIssue>);

impl Collector {
    fn add(
        &mut self,
        code: IssueCode,
        severity: Severity,
        page: Option<usize>,
        xref: Option<i32>,
        message: impl Into<String>,
        suggestion: impl Into<String>,
    ) {
        self.0.push(A11yIssue {
            code,
            severity,
            page,
            xref,
            message: message.into(),
            suggestion: suggestion.into(),
        });
    }
}

/// ¿Parece una etiqueta de idioma BCP 47 (`es`, `es-ES`, `zh-Hant-TW`)?
pub fn is_valid_language_tag(tag: &str) -> bool {
    let mut parts = tag.split('-');
    let Some(first) = parts.next() else { return false };
    if !(2..=3).contains(&first.len()) || !first.chars().all(|c| c.is_ascii_alphabetic()) {
        return false;
    }
    parts.all(|p| (1..=8).contains(&p.len()) && p.chars().all(|c| c.is_ascii_alphanumeric()))
}

fn document_title(doc: &PdfDocument) -> Option<String> {
    let trailer = doc.trailer().ok()?;
    let info = trailer.get_dict("Info").ok()??;
    string_of(&info, "Title").filter(|t| !t.trim().is_empty())
}

fn has_xmp_title(catalog: &Object) -> bool {
    catalog
        .get_dict("Metadata")
        .ok()
        .flatten()
        .and_then(|m| m.read_stream().ok())
        .is_some_and(|b| String::from_utf8_lossy(&b).contains("<dc:title"))
}

fn check_document(doc: &PdfDocument, report: &mut A11yReport, out: &mut Collector) -> Result<()> {
    let catalog = doc.catalog()?;

    let marked = catalog
        .get_dict("MarkInfo")?
        .and_then(|m| m.get_dict("Marked").ok().flatten())
        .and_then(|v| v.as_bool().ok())
        .unwrap_or(false);
    let has_tree = catalog.get_dict("StructTreeRoot")?.is_some();
    report.tagged = marked && has_tree;
    if !has_tree {
        out.add(
            IssueCode::NoStructureTree,
            Severity::Error,
            None,
            None,
            "El documento no tiene estructura lógica (no está etiquetado).",
            "Etiqueta el documento: `pliego a11y autotag` crea una estructura inicial o genera el PDF desde una aplicación que etiquete.",
        );
    } else if !marked {
        out.add(
            IssueCode::NotTagged,
            Severity::Error,
            None,
            None,
            "El documento tiene estructura, pero no está marcado como etiquetado (MarkInfo/Marked).",
            "Marca el documento como etiquetado para que los lectores de pantalla usen la estructura.",
        );
    }

    // Idioma.
    match string_of(&catalog, "Lang") {
        None => out.add(
            IssueCode::NoLanguage,
            Severity::Error,
            None,
            None,
            "El documento no declara su idioma.",
            "Indica el idioma principal, por ejemplo `pliego a11y set-language doc.pdf -o salida.pdf --lang es-ES`.",
        ),
        Some(l) if !is_valid_language_tag(&l) => out.add(
            IssueCode::InvalidLanguage,
            Severity::Error,
            None,
            None,
            format!("El idioma declarado («{l}») no es una etiqueta válida."),
            "Usa una etiqueta como `es`, `es-ES` o `en-US`.",
        ),
        Some(l) => report.language = Some(l),
    }

    // Título.
    report.title = document_title(doc);
    if report.title.is_none() && !has_xmp_title(&catalog) {
        out.add(
            IssueCode::NoTitle,
            Severity::Error,
            None,
            None,
            "El documento no tiene título.",
            "Añade un título descriptivo: `pliego a11y set-title`.",
        );
    }
    let display_title = catalog
        .get_dict("ViewerPreferences")?
        .and_then(|v| v.get_dict("DisplayDocTitle").ok().flatten())
        .and_then(|v| v.as_bool().ok())
        .unwrap_or(false);
    if !display_title {
        out.add(
            IssueCode::TitleNotDisplayed,
            Severity::Warning,
            None,
            None,
            "El visor mostrará el nombre del archivo en lugar del título.",
            "Activa «mostrar el título del documento» en las preferencias de visualización.",
        );
    }

    // Permisos: la copia para accesibilidad no debe estar bloqueada.
    if !doc.permissions().contains(Permission::ACCESSIBILITY) {
        out.add(
            IssueCode::CopyRestricted,
            Severity::Error,
            None,
            None,
            "El cifrado impide que los lectores de pantalla extraigan el texto.",
            "Vuelve a cifrar permitiendo la extracción para accesibilidad.",
        );
    }
    Ok(())
}

fn check_pages(doc: &PdfDocument, report: &mut A11yReport, out: &mut Collector) -> Result<()> {
    let pages = report.pages;
    for p in 0..pages {
        let page = doc.find_page(p as i32)?;
        if let Ok(bytes) = crate::autotag::read_contents(&page)
            && let Some(low) = crate::contrast::scan(&bytes)
        {
            out.add(
                IssueCode::LowContrast,
                Severity::Warning,
                Some(p),
                None,
                format!(
                    "{} fragmento(s) de texto tienen poco contraste con el fondo (el peor, {:.1}:1, «{}»; se pide {}:1, o {}:1 en texto grande).",
                    low.count,
                    low.worst_ratio,
                    low.text,
                    crate::contrast::MIN_RATIO,
                    crate::contrast::MIN_RATIO_LARGE
                ),
                "Oscurece el texto o aclara el fondo hasta alcanzar la razón de contraste mínima.",
            );
        }
        let Some(annots) = page.get_dict("Annots")? else {
            continue;
        };
        if !annots.is_array().unwrap_or(false) {
            continue;
        }
        let mut any = false;
        for i in 0..annots.len()?.min(10_000) {
            let Some(a) = annots.get_array(i as i32)? else {
                continue;
            };
            let subtype = name_of(&a, "Subtype");
            let x = crate::io::xref_of(&a);
            let xref = (x != 0).then_some(x);
            match subtype.as_deref() {
                Some("Widget") => {
                    any = true;
                    if string_of(&a, "TU").is_none_or(|t| t.trim().is_empty()) {
                        out.add(
                            IssueCode::FieldWithoutLabel,
                            Severity::Error,
                            Some(p),
                            xref,
                            "Un campo de formulario no tiene descripción (tooltip).",
                            "Añade una descripción al campo para que el lector de pantalla pueda anunciarlo.",
                        );
                    }
                }
                Some("Link") => {
                    any = true;
                    if string_of(&a, "Contents").is_none_or(|t| t.trim().is_empty()) {
                        out.add(
                            IssueCode::LinkWithoutDescription,
                            Severity::Warning,
                            Some(p),
                            xref,
                            "Un enlace no tiene descripción alternativa.",
                            "Añade una descripción al enlace (/Contents) que diga a dónde lleva.",
                        );
                    }
                }
                Some("Popup") | None => {}
                Some(_) => any = true,
            }
        }
        if any && name_of(&page, "Tabs").as_deref() != Some("S") {
            out.add(
                IssueCode::NoTabOrder,
                Severity::Warning,
                Some(p),
                None,
                "La página tiene anotaciones o campos pero el orden de tabulación no sigue la estructura.",
                "Fija el orden de tabulación de la página a «estructura» (/Tabs /S).",
            );
        }
    }
    Ok(())
}

fn heading_level(role: &str) -> Option<u8> {
    let rest = role.strip_prefix('H')?;
    rest.parse::<u8>().ok().filter(|n| (1..=6).contains(n))
}

fn check_structure(tree: &Tree, report: &A11yReport, out: &mut Collector) {
    if tree.truncated {
        out.add(
            IssueCode::TruncatedStructure,
            Severity::Warning,
            None,
            None,
            "La estructura es tan grande o profunda que solo se comprobó una parte.",
            "Revisa el documento con otra herramienta o simplifica su estructura.",
        );
    }
    let mut last_level: Option<u8> = None;
    let mut any_heading = false;
    for (idx, e) in tree.elems.iter().enumerate() {
        match e.role.as_str() {
            "Figure" | "Formula" if !e.has_alt => out.add(
                IssueCode::FigureWithoutAlt,
                Severity::Error,
                e.page,
                (e.xref != 0).then_some(e.xref),
                "Una figura no tiene texto alternativo.",
                "Describe la imagen: `pliego a11y set-alt`. Si es decorativa, márcala como artefacto.",
            ),
            "Table" if !has_descendant(tree, idx, "TH") => out.add(
                IssueCode::TableWithoutHeader,
                Severity::Error,
                e.page,
                (e.xref != 0).then_some(e.xref),
                "Una tabla no tiene celdas de encabezado.",
                "Marca la primera fila (o columna) como encabezado (TH).",
            ),
            r => {
                if r == "H" {
                    any_heading = true;
                }
                if let Some(level) = heading_level(r) {
                    any_heading = true;
                    match last_level {
                        None if level != 1 => out.add(
                            IssueCode::FirstHeadingNotH1,
                            Severity::Warning,
                            e.page,
                            (e.xref != 0).then_some(e.xref),
                            format!("El primer encabezado es de nivel {level}, no 1."),
                            "Empieza la jerarquía por un encabezado de nivel 1.",
                        ),
                        Some(prev) if level > prev + 1 => out.add(
                            IssueCode::HeadingSkipped,
                            Severity::Warning,
                            e.page,
                            (e.xref != 0).then_some(e.xref),
                            format!("Se salta de un encabezado de nivel {prev} a uno de nivel {level}."),
                            "No te saltes niveles de encabezado.",
                        ),
                        _ => {}
                    }
                    last_level = Some(level);
                }
            }
        }
    }
    if !any_heading && report.pages > 1 {
        out.add(
            IssueCode::NoHeadings,
            Severity::Warning,
            None,
            None,
            "El documento no tiene encabezados.",
            "Marca los títulos de sección como encabezados para poder navegar por ellos.",
        );
    }
}

fn has_descendant(tree: &Tree, idx: usize, role: &str) -> bool {
    let mut stack = vec![idx];
    let mut visited = 0usize;
    while let Some(i) = stack.pop() {
        visited += 1;
        if visited > 100_000 {
            return true; // no declarar un error que no se ha podido comprobar
        }
        for &c in &tree.elems[i].children {
            if tree.elems[c].role == role {
                return true;
            }
            stack.push(c);
        }
    }
    false
}

/// Comprueba la accesibilidad de un documento abierto y devuelve el informe con correcciones sugeridas.
pub fn check(doc: &PdfDocument) -> Result<A11yReport> {
    let mut report = A11yReport {
        pages: doc.page_count()?.max(0) as usize,
        ..Default::default()
    };
    let mut out = Collector(Vec::new());
    check_document(doc, &mut report, &mut out)?;
    if let Some(tree) = read_tree(doc)? {
        check_structure(&tree, &report, &mut out);
    }
    check_pages(doc, &mut report, &mut out)?;
    if report.pages >= BOOKMARK_MIN_PAGES && doc.outlines().map(|o| o.is_empty()).unwrap_or(false) {
        out.add(
            IssueCode::NoBookmarks,
            Severity::Warning,
            None,
            None,
            format!("El documento tiene {} páginas y ningún marcador.", report.pages),
            "Añade marcadores a partir de los encabezados.",
        );
    }
    report.issues = out.0;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_tags() {
        for ok in ["es", "es-ES", "en-US", "zh-Hant-TW", "spa"] {
            assert!(is_valid_language_tag(ok), "{ok}");
        }
        for bad in ["", "e", "español", "es_ES", "es-", "12", "es-ES-abcdefghi"] {
            assert!(!is_valid_language_tag(bad), "{bad}");
        }
    }
}
