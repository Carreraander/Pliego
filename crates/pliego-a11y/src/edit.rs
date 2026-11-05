//! Correcciones: idioma, título, texto alternativo y orden de tabulación.

use std::fmt::Write as _;
use std::path::Path;

use pliego_core::{Error, Report, Result};
use pliego_pdf::{Object, PdfDocument};

use crate::check::is_valid_language_tag;
use crate::io::{open_pdf, save_atomic};

/// Cadena de texto PDF: ASCII tal cual, el resto en UTF-16BE con BOM (como exige el formato).
pub(crate) fn text_string(doc: &PdfDocument, s: &str) -> Result<Object> {
    let mut hex = String::from("<");
    if s.is_ascii() {
        for b in s.bytes() {
            let _ = write!(hex, "{b:02X}");
        }
    } else {
        hex.push_str("FEFF");
        for u in s.encode_utf16() {
            let _ = write!(hex, "{u:04X}");
        }
    }
    hex.push('>');
    doc.new_object_from_str(&hex)
}

/// Fija el idioma principal del documento (`/Lang` del catálogo).
pub fn set_language(doc: &mut PdfDocument, lang: &str) -> Result<()> {
    if !is_valid_language_tag(lang) {
        return Err(Error::InvalidInput(format!(
            "«{lang}» no es una etiqueta de idioma válida; usa algo como es, es-ES o en-US"
        )));
    }
    let mut catalog = doc.catalog()?;
    let value = text_string(doc, lang)?;
    catalog.dict_put("Lang", value)
}

/// Fija el título (`/Info /Title`) y activa que el visor lo muestre en lugar del nombre del archivo.
pub fn set_title(doc: &mut PdfDocument, title: &str) -> Result<()> {
    if title.trim().is_empty() {
        return Err(Error::InvalidInput("el título no puede estar vacío".into()));
    }
    let mut trailer = doc.trailer()?;
    let mut info = match trailer.get_dict("Info")? {
        Some(i) if i.is_dict().unwrap_or(false) => i,
        _ => {
            let new = doc.new_dict()?;
            let ind = doc.add_object(&new)?;
            trailer.dict_put("Info", ind.try_clone()?)?;
            ind
        }
    };
    let value = text_string(doc, title)?;
    info.dict_put("Title", value)?;

    let mut catalog = doc.catalog()?;
    let mut prefs = match catalog.get_dict("ViewerPreferences")? {
        Some(p) if p.is_dict().unwrap_or(false) => p,
        _ => {
            let new = doc.new_dict()?;
            catalog.dict_put("ViewerPreferences", new)?;
            catalog
                .get_dict("ViewerPreferences")?
                .ok_or_else(|| Error::Pdf("no se pudo crear ViewerPreferences".into()))?
        }
    };
    prefs.dict_put("DisplayDocTitle", doc.new_bool(true))
}

/// Fija el texto alternativo de un elemento de estructura (por ejemplo, una figura). `xref` es el número de
/// objeto que devuelve el comprobador en cada hallazgo. Un texto vacío quita el existente.
pub fn set_alt_text(doc: &mut PdfDocument, xref: i32, alt: &str) -> Result<()> {
    if xref <= 0 || xref as u32 >= doc.count_objects()? {
        return Err(Error::InvalidInput(format!(
            "el objeto {xref} no existe en el documento"
        )));
    }
    let mut elem = doc.new_indirect(xref, 0)?;
    if !elem.is_dict().unwrap_or(false) || elem.get_dict("S")?.is_none() {
        return Err(Error::InvalidInput(format!(
            "el objeto {xref} no es un elemento de estructura al que se pueda poner texto alternativo"
        )));
    }
    if alt.trim().is_empty() {
        if elem.get_dict("Alt")?.is_some() {
            elem.dict_delete("Alt")?;
        }
        return Ok(());
    }
    let value = text_string(doc, alt)?;
    elem.dict_put("Alt", value)
}

/// Fija el orden de tabulación de todas las páginas a «estructura» (`/Tabs /S`).
pub fn set_tab_order(doc: &mut PdfDocument) -> Result<usize> {
    let mut changed = 0;
    for p in 0..doc.page_count()? {
        let mut page = doc.find_page(p)?;
        let ok = page
            .get_dict("Tabs")?
            .and_then(|t| t.as_name().ok())
            .is_some_and(|n| n == b"S");
        if !ok {
            page.dict_put("Tabs", Object::new_name("S")?)?;
            changed += 1;
        }
    }
    Ok(changed)
}

/// Qué corregir en [`fix_file`].
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct FixOptions {
    pub language: Option<String>,
    pub title: Option<String>,
    /// Pares (objeto, texto alternativo).
    pub alt_texts: Vec<(i32, String)>,
    pub tab_order: bool,
}

/// Aplica correcciones a un archivo y guarda el resultado en `output` (nunca sobrescribe la entrada).
pub fn fix_file(input: &Path, output: &Path, opts: &FixOptions, password: Option<&str>) -> Result<Report> {
    let mut doc = open_pdf(input, password)?;
    let mut done = Vec::new();
    if let Some(l) = &opts.language {
        set_language(&mut doc, l)?;
        done.push(format!("idioma {l}"));
    }
    if let Some(t) = &opts.title {
        set_title(&mut doc, t)?;
        done.push("título".to_string());
    }
    for (xref, alt) in &opts.alt_texts {
        set_alt_text(&mut doc, *xref, alt)?;
    }
    if !opts.alt_texts.is_empty() {
        done.push(format!("{} texto(s) alternativo(s)", opts.alt_texts.len()));
    }
    if opts.tab_order {
        let n = set_tab_order(&mut doc)?;
        done.push(format!("orden de tabulación en {n} página(s)"));
    }
    if done.is_empty() {
        return Err(Error::InvalidInput("no se pidió ninguna corrección".into()));
    }
    let mut options = pliego_pdf::WriteOptions::default();
    options.set_garbage_level(1);
    save_atomic(&doc, input, output, options)?;
    Ok(Report::with_output(output, format!("Corregido: {}.", done.join(", "))))
}
