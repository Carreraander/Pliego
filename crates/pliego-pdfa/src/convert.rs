//! Conversión a PDF/A: limpia lo prohibido, incrusta fuentes estándar, escribe metadatos XMP y la
//! intención de salida sRGB. Lo que no se puede arreglar sin cambiar el documento se rechaza con un
//! mensaje claro: nunca se declara PDF/A un documento que no lo es.

use std::collections::HashSet;
use std::path::Path;

use pliego_core::{Error, JobContext, Report, Result};
use pliego_pdf::{Object, PdfDocument, WriteOptions};

use crate::Level;
use crate::io::{name_of, open_pdf, save_atomic, xref_of};
use crate::util::{array_items, entries, get, text_of, text_string};
use crate::xmp::{InfoValues, build_xmp, now_pdf_date};

const MAX_DEPTH: usize = 32;
const MAX_OBJECTS: usize = 200_000;

pub(crate) const FORBIDDEN_ACTIONS: &[&str] = &[
    "Launch",
    "Sound",
    "Movie",
    "ResetForm",
    "ImportData",
    "JavaScript",
    "Hide",
    "SetOCGState",
    "Rendition",
    "Trans",
    "GoTo3DView",
];

#[derive(Default)]
struct Notes {
    warnings: Vec<String>,
    removed_actions: usize,
    removed_attachments: usize,
    removed_transparency: usize,
    embedded_fonts: usize,
    recoded_streams: usize,
    fixed_annotations: usize,
    removed_annotations: usize,
    unembeddable: Vec<String>,
}

fn action_is_forbidden(a: &Object, depth: usize) -> bool {
    if depth > 16 || !a.is_dict().unwrap_or(false) {
        return false;
    }
    if name_of(a, "S").is_some_and(|s| FORBIDDEN_ACTIONS.contains(&s.as_str())) {
        return true;
    }
    match get(a, "Next") {
        Some(n) if n.is_array().unwrap_or(false) => array_items(&n).iter().any(|x| action_is_forbidden(x, depth + 1)),
        Some(n) => action_is_forbidden(&n, depth + 1),
        None => false,
    }
}

/// Quita las acciones prohibidas de `holder/AA`. Devuelve cuántas quitó.
fn strip_additional_actions(holder: &mut Object) -> Result<usize> {
    let Some(mut aa) = get(holder, "AA") else { return Ok(0) };
    let mut n = 0;
    for (k, a) in entries(&aa) {
        if action_is_forbidden(&a, 0) {
            aa.dict_delete(k.as_str())?;
            n += 1;
        }
    }
    Ok(n)
}

fn is_lzw_only(stream: &Object) -> bool {
    let Some(f) = get(stream, "Filter") else { return false };
    let lzw = |o: &Object| o.as_name().is_ok_and(|n| n == b"LZWDecode" || n == b"LZW");
    if f.is_array().unwrap_or(false) {
        let items = array_items(&f);
        items.len() == 1 && lzw(&items[0])
    } else {
        lzw(&f)
    }
}

fn has_lzw_anywhere(stream: &Object) -> bool {
    let Some(f) = get(stream, "Filter") else { return false };
    let lzw = |o: &Object| o.as_name().is_ok_and(|n| n == b"LZWDecode" || n == b"LZW");
    lzw(&f) || array_items(&f).iter().any(lzw)
}

/// Vuelve a escribir un flujo con compresión LZW sin filtro (PDF/A no admite LZW).
fn recode_lzw(stream: &mut Object, notes: &mut Notes) -> Result<()> {
    if !has_lzw_anywhere(stream) {
        return Ok(());
    }
    if !is_lzw_only(stream) {
        notes
            .warnings
            .push("Un flujo combina LZW con otros filtros y no se pudo recodificar.".into());
        return Ok(());
    }
    let data = stream.read_stream()?;
    stream.write_stream(&data)?;
    notes.recoded_streams += 1;
    Ok(())
}

const STANDARD_14: &[&str] = &[
    "Helvetica",
    "Helvetica-Bold",
    "Helvetica-Oblique",
    "Helvetica-BoldOblique",
    "Times-Roman",
    "Times-Bold",
    "Times-Italic",
    "Times-BoldItalic",
    "Courier",
    "Courier-Bold",
    "Courier-Oblique",
    "Courier-BoldOblique",
];

fn font_embedded(font: &Object) -> bool {
    let has_file = |d: &Object| {
        ["FontFile", "FontFile2", "FontFile3"]
            .iter()
            .any(|k| get(d, k).is_some())
    };
    match name_of(font, "Subtype").as_deref() {
        Some("Type3") => true,
        Some("Type0") => get(font, "DescendantFonts")
            .and_then(|d| array_items(&d).into_iter().next())
            .and_then(|d| get(&d, "FontDescriptor"))
            .is_some_and(|d| has_file(&d)),
        _ => get(font, "FontDescriptor").is_some_and(|d| has_file(&d)),
    }
}

/// Sustituye una de las 14 fuentes estándar no incrustadas por una fuente Type3 equivalente.
fn embed_standard_font(doc: &mut PdfDocument, font: &mut Object) -> Result<bool> {
    let Some(base) = name_of(font, "BaseFont") else {
        return Ok(false);
    };
    let subtype = name_of(font, "Subtype").unwrap_or_default();
    if subtype != "Type1" || !STANDARD_14.contains(&base.as_str()) {
        return Ok(false);
    }
    // Solo si los códigos se interpretan como WinAnsi (o por defecto): las anchuras que se escriben son las suyas.
    match get(font, "Encoding") {
        None => {}
        Some(e) if e.as_name().is_ok_and(|n| n == b"WinAnsiEncoding") => {}
        Some(_) => return Ok(false),
    }
    crate::substitute::make_type3(doc, font, &base)?;
    Ok(true)
}

struct Walk {
    seen: HashSet<i32>,
    budget: usize,
    level: Level,
}

impl Walk {
    fn enter(&mut self, o: &Object) -> bool {
        if self.budget == 0 {
            return false;
        }
        self.budget -= 1;
        let x = xref_of(o);
        x == 0 || self.seen.insert(x)
    }
}

fn fix_resources(doc: &mut PdfDocument, res: &Object, w: &mut Walk, notes: &mut Notes, depth: usize) -> Result<()> {
    if depth > MAX_DEPTH || !res.is_dict().unwrap_or(false) {
        return Ok(());
    }
    if let Some(fonts) = get(res, "Font") {
        for (_, mut f) in entries(&fonts) {
            if !f.is_dict().unwrap_or(false) || !w.enter(&f) || font_embedded(&f) {
                continue;
            }
            if embed_standard_font(doc, &mut f)? {
                notes.embedded_fonts += 1;
            } else {
                let name = name_of(&f, "BaseFont").unwrap_or_else(|| "(sin nombre)".into());
                if !notes.unembeddable.contains(&name) {
                    notes.unembeddable.push(name);
                }
            }
        }
    }
    if !w.level.allows_transparency()
        && let Some(gs) = get(res, "ExtGState")
    {
        for (_, mut g) in entries(&gs) {
            if !g.is_dict().unwrap_or(false) || !w.enter(&g) {
                continue;
            }
            let mut changed = false;
            for k in ["CA", "ca"] {
                if get(&g, k).and_then(|v| v.as_float().ok()).is_some_and(|v| v < 1.0) {
                    g.dict_put(k, Object::new_real(1.0)?)?;
                    changed = true;
                }
            }
            if get(&g, "SMask").is_some_and(|s| s.as_name().map(|n| n != b"None").unwrap_or(true)) {
                g.dict_put("SMask", Object::new_name("None")?)?;
                changed = true;
            }
            if get(&g, "BM").is_some_and(|b| {
                b.as_name()
                    .map(|n| n != b"Normal" && n != b"Compatible")
                    .unwrap_or(true)
            }) {
                g.dict_put("BM", Object::new_name("Normal")?)?;
                changed = true;
            }
            if changed {
                notes.removed_transparency += 1;
            }
        }
    }
    if let Some(xo) = get(res, "XObject") {
        for (_, mut x) in entries(&xo) {
            if !x.is_dict().unwrap_or(false) || !w.enter(&x) {
                continue;
            }
            recode_lzw(&mut x, notes)?;
            match name_of(&x, "Subtype").as_deref() {
                Some("Image") => {
                    if !w.level.allows_transparency() && get(&x, "SMask").is_some() {
                        x.dict_delete("SMask")?;
                        notes.removed_transparency += 1;
                    }
                }
                Some("Form") => {
                    if !w.level.allows_transparency()
                        && get(&x, "Group").is_some_and(|g| name_of(&g, "S").as_deref() == Some("Transparency"))
                    {
                        x.dict_delete("Group")?;
                        notes.removed_transparency += 1;
                    }
                    if let Some(r) = get(&x, "Resources") {
                        fix_resources(doc, &r, w, notes, depth + 1)?;
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn fix_annotations(doc: &mut PdfDocument, page_no: usize, notes: &mut Notes, level: Level) -> Result<()> {
    let page_obj = doc.find_page(page_no as i32)?;
    let Some(mut annots) = get(&page_obj, "Annots") else {
        return Ok(());
    };
    if !annots.is_array().unwrap_or(false) {
        return Ok(());
    }

    // 1) Quitar las anotaciones no admitidas y arreglar banderas y acciones.
    let mut i = annots.len()? as i32 - 1;
    while i >= 0 {
        let a = annots.get_array(i)?;
        let Some(mut a) = a.filter(|a| a.is_dict().unwrap_or(false)) else {
            i -= 1;
            continue;
        };
        let sub = name_of(&a, "Subtype").unwrap_or_default();
        let drop = matches!(sub.as_str(), "Movie" | "Sound" | "Screen" | "3D")
            || (sub == "FileAttachment" && !level.allows_attachments());
        if drop {
            annots.array_delete(i)?;
            if sub == "FileAttachment" {
                notes.removed_attachments += 1;
            } else {
                notes.removed_annotations += 1;
            }
            i -= 1;
            continue;
        }
        if let Some(act) = get(&a, "A")
            && action_is_forbidden(&act, 0)
        {
            a.dict_delete("A")?;
            notes.removed_actions += 1;
        }
        notes.removed_actions += strip_additional_actions(&mut a)?;
        if sub != "Popup" {
            let flags = get(&a, "F").and_then(|f| f.as_int().ok()).unwrap_or(0);
            let fixed = (flags | 4) & !(1 | 2 | 32);
            if fixed != flags {
                a.dict_put("F", Object::new_int(fixed)?)?;
                notes.fixed_annotations += 1;
            }
        }
        i -= 1;
    }

    // 2) Generar el aspecto de las que no lo tienen.
    let needs_ap = |a: &Object| {
        let sub = name_of(a, "Subtype").unwrap_or_default();
        sub != "Link" && sub != "Popup" && get(a, "AP").and_then(|ap| get(&ap, "N")).is_none()
    };
    if array_items(&annots).iter().any(needs_ap) {
        let page = doc.load_pdf_page(page_no as i32)?;
        for mut an in page.annotations() {
            if needs_ap(&an.object()) {
                let _ = an.update();
            }
        }
    }
    // 3) Las que sigan sin aspecto se quitan.
    let mut i = annots.len()? as i32 - 1;
    while i >= 0 {
        if let Some(a) = annots.get_array(i)?
            && a.is_dict().unwrap_or(false)
            && needs_ap(&a)
        {
            annots.array_delete(i)?;
            notes.removed_annotations += 1;
        }
        i -= 1;
    }
    Ok(())
}

fn mime_for(ext: &str) -> &'static str {
    match ext {
        "pdf" => "application/pdf",
        "xml" => "text/xml",
        "txt" => "text/plain",
        "csv" => "text/csv",
        "json" => "application/json",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        _ => "application/octet-stream",
    }
}

/// Identificador del documento: ISO 19005 exige `/ID` (dos cadenas) en el trailer.
fn ensure_trailer_id(doc: &mut PdfDocument, seed: &[u8]) -> Result<()> {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let mut trailer = doc.trailer()?;
    if let Some(id) = get(&trailer, "ID")
        && id.is_array().unwrap_or(false)
        && array_items(&id)
            .iter()
            .filter(|s| s.is_string().unwrap_or(false))
            .count()
            >= 2
    {
        return Ok(());
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut hex = String::with_capacity(32);
    for salt in [1u8, 2u8] {
        let mut h = DefaultHasher::new();
        (salt, nanos, seed.len()).hash(&mut h);
        seed.hash(&mut h);
        hex.push_str(&format!("{:016x}", h.finish()));
    }
    let a = doc.new_object_from_str(&format!("<{}>", &hex[..32]))?;
    let b = doc.new_object_from_str(&format!("<{}>", &hex[..32]))?;
    let mut arr = doc.new_array()?;
    arr.array_push(a)?;
    arr.array_push(b)?;
    trailer.dict_put("ID", arr)?;
    Ok(())
}

/// Contenido opcional (PDF/A-2 y 3): cada configuración necesita un nombre (6.9).
fn name_oc_configs(doc: &PdfDocument, catalog: &Object) -> Result<()> {
    let Some(ocp) = get(catalog, "OCProperties") else {
        return Ok(());
    };
    let mut configs: Vec<Object> = Vec::new();
    if let Some(d) = get(&ocp, "D") {
        configs.push(d);
    }
    if let Some(c) = get(&ocp, "Configs") {
        configs.extend(array_items(&c));
    }
    for (i, mut c) in configs.into_iter().enumerate() {
        if c.is_dict().unwrap_or(false) && get(&c, "Name").is_none() {
            c.dict_put("Name", doc.new_string(&format!("Config {}", i + 1))?)?;
        }
    }
    Ok(())
}

/// Adjuntos en PDF/A-3: cada archivo incrustado se asocia al documento (AF con AFRelationship).
fn associate_attachments(doc: &PdfDocument, catalog: &mut Object) -> Result<usize> {
    fn collect(node: &Object, out: &mut Vec<Object>, depth: usize) {
        if depth > 16 || out.len() > 10_000 {
            return;
        }
        if let Some(names) = get(node, "Names") {
            let items = array_items(&names);
            out.extend(items.into_iter().skip(1).step_by(2));
        }
        if let Some(kids) = get(node, "Kids") {
            for k in array_items(&kids) {
                collect(&k, out, depth + 1);
            }
        }
    }
    let Some(tree) = get(catalog, "Names").and_then(|n| get(&n, "EmbeddedFiles")) else {
        return Ok(0);
    };
    let mut specs = Vec::new();
    collect(&tree, &mut specs, 0);
    let mut af = doc.new_array()?;
    let mut n = 0;
    for mut s in specs {
        if !s.is_dict().unwrap_or(false) {
            continue;
        }
        if get(&s, "AFRelationship").is_none() {
            s.dict_put("AFRelationship", Object::new_name("Unspecified")?)?;
        }
        if get(&s, "UF").is_none()
            && let Some(f) = get(&s, "F")
        {
            s.dict_put("UF", f)?;
        }
        // ISO 19005-3 (6.8): el flujo del archivo incrustado debe declarar su tipo MIME (Subtype).
        if let Some(mut ef) = get(&s, "EF").and_then(|e| get(&e, "F"))
            && ef.is_stream().unwrap_or(false)
            && get(&ef, "Subtype").is_none()
        {
            let name = get(&s, "UF")
                .or_else(|| get(&s, "F"))
                .and_then(|f| {
                    f.as_string()
                        .ok()
                        .map(|b| String::from_utf8_lossy(b.as_bytes()).to_lowercase())
                })
                .unwrap_or_default();
            let ext = name.rsplit('.').next().unwrap_or("");
            ef.dict_put("Subtype", Object::new_name(mime_for(ext))?)?;
        }
        af.array_push(s)?;
        n += 1;
    }
    if n > 0 {
        catalog.dict_put("AF", af)?;
    }
    Ok(n)
}

pub(crate) fn read_info(doc: &PdfDocument) -> Result<InfoValues> {
    let trailer = doc.trailer()?;
    let info = get(&trailer, "Info").filter(|i| i.is_dict().unwrap_or(false));
    let f = |k: &str| info.as_ref().and_then(|i| text_of(i, k));
    Ok(InfoValues {
        title: f("Title"),
        author: f("Author"),
        subject: f("Subject"),
        keywords: f("Keywords"),
        creator: f("Creator"),
        producer: f("Producer"),
        created: f("CreationDate"),
        modified: f("ModDate"),
    })
}

pub(crate) fn write_info(doc: &mut PdfDocument, v: &InfoValues) -> Result<()> {
    let mut trailer = doc.trailer()?;
    let mut info = match get(&trailer, "Info").filter(|i| i.is_dict().unwrap_or(false)) {
        Some(i) => i,
        None => {
            let d = doc.new_dict()?;
            let ind = doc.add_object(&d)?;
            trailer.dict_put("Info", ind.try_clone()?)?;
            ind
        }
    };
    for (k, val) in [
        ("Title", &v.title),
        ("Author", &v.author),
        ("Subject", &v.subject),
        ("Keywords", &v.keywords),
        ("Creator", &v.creator),
        ("Producer", &v.producer),
        ("CreationDate", &v.created),
        ("ModDate", &v.modified),
    ] {
        if let Some(val) = val {
            let obj = if k.ends_with("Date") {
                doc.new_string(val)?
            } else {
                text_string(doc, val)?
            };
            info.dict_put(k, obj)?;
        }
    }
    Ok(())
}

/// Escribe el XMP (sin filtros) y la intención de salida sRGB.
fn write_metadata_and_intent(doc: &mut PdfDocument, level: Level) -> Result<()> {
    let mut info = read_info(doc)?;
    let now = now_pdf_date();
    info.producer.get_or_insert_with(|| "Pliego".to_string());
    info.created.get_or_insert_with(|| now.clone());
    info.modified = Some(now);
    write_info(doc, &info)?;

    let xmp = build_xmp(level, &info);
    let mut dict = doc.new_dict()?;
    dict.dict_put("Type", Object::new_name("Metadata")?)?;
    dict.dict_put("Subtype", Object::new_name("XML")?)?;
    let meta = doc.add_stream(xmp.as_bytes(), Some(&dict), false)?;

    let mut icc_dict = doc.new_dict()?;
    icc_dict.dict_put("N", Object::new_int(3)?)?;
    let icc = doc.add_stream(&crate::icc::srgb_profile(), Some(&icc_dict), false)?;
    let mut oi = doc.new_dict()?;
    oi.dict_put("Type", Object::new_name("OutputIntent")?)?;
    oi.dict_put("S", Object::new_name("GTS_PDFA1")?)?;
    oi.dict_put("OutputConditionIdentifier", doc.new_string("sRGB IEC61966-2.1")?)?;
    oi.dict_put("Info", doc.new_string("sRGB IEC61966-2.1")?)?;
    oi.dict_put("DestOutputProfile", icc)?;
    let oi = doc.add_object(&oi)?;
    let mut intents = doc.new_array()?;
    intents.array_push(oi)?;

    let mut catalog = doc.catalog()?;
    catalog.dict_put("Metadata", meta)?;
    catalog.dict_put("OutputIntents", intents)?;
    Ok(())
}

fn summarize(level: Level, n: &Notes) -> String {
    let mut parts = Vec::new();
    let mut add = |count: usize, text: &str| {
        if count > 0 {
            parts.push(format!("{count} {text}"));
        }
    };
    add(n.embedded_fonts, "fuente(s) incrustada(s)");
    add(n.removed_actions, "acción(es) o JavaScript quitados");
    add(n.removed_attachments, "adjunto(s) quitados");
    add(n.removed_transparency, "elemento(s) de transparencia aplanados");
    add(n.recoded_streams, "flujo(s) LZW recodificados");
    add(n.fixed_annotations, "anotación(es) ajustadas");
    add(n.removed_annotations, "anotación(es) quitadas");
    if parts.is_empty() {
        format!("Convertido a {}.", level.name())
    } else {
        format!("Convertido a {}: {}.", level.name(), parts.join(", "))
    }
}

/// Convierte `input` a PDF/A del nivel pedido y lo guarda en `output` (nunca sobrescribe la entrada).
pub fn convert(input: &Path, output: &Path, level: Level, ctx: &JobContext) -> Result<Report> {
    let mut doc = open_pdf(input, None)?;
    let mut notes = Notes::default();
    let pages = doc.page_count()?.max(0) as usize;
    ctx.step(0, pages as u64 + 2, "Preparando")?;

    // Catálogo: acciones, capas y adjuntos.
    let mut catalog = doc.catalog()?;
    if let Some(a) = get(&catalog, "OpenAction")
        && action_is_forbidden(&a, 0)
    {
        catalog.dict_delete("OpenAction")?;
        notes.removed_actions += 1;
    }
    notes.removed_actions += strip_additional_actions(&mut catalog)?;
    if let Some(mut names) = get(&catalog, "Names") {
        if get(&names, "JavaScript").is_some() {
            names.dict_delete("JavaScript")?;
            notes.removed_actions += 1;
        }
        if !level.allows_attachments() && get(&names, "EmbeddedFiles").is_some() {
            names.dict_delete("EmbeddedFiles")?;
            notes.removed_attachments += 1;
        }
    }
    if !level.allows_attachments() && get(&catalog, "AF").is_some() {
        catalog.dict_delete("AF")?;
    }
    if level == Level::A1b && get(&catalog, "OCProperties").is_some() {
        catalog.dict_delete("OCProperties")?;
        notes
            .warnings
            .push("Se quitaron las capas (contenido opcional), que PDF/A-1 no permite.".into());
    }
    if level != Level::A1b {
        name_oc_configs(&doc, &catalog)?;
    }
    if level.allows_attachments() {
        associate_attachments(&doc, &mut catalog)?;
    }

    // Páginas.
    let mut walk = Walk {
        seen: HashSet::new(),
        budget: MAX_OBJECTS,
        level,
    };
    for p in 0..pages {
        ctx.step(p as u64 + 1, pages as u64 + 2, "Ajustando páginas")?;
        let mut page = doc.find_page(p as i32)?;
        if let Some(res) = page.get_dict_inheritable("Resources")? {
            fix_resources(&mut doc, &res, &mut walk, &mut notes, 0)?;
        }
        if !level.allows_transparency()
            && get(&page, "Group").is_some_and(|g| name_of(&g, "S").as_deref() == Some("Transparency"))
        {
            page.dict_delete("Group")?;
            notes.removed_transparency += 1;
        }
        if let Some(c) = get(&page, "Contents") {
            let streams = if c.is_array().unwrap_or(false) {
                array_items(&c)
            } else {
                vec![c]
            };
            for mut s in streams {
                if s.is_stream().unwrap_or(false) {
                    recode_lzw(&mut s, &mut notes)?;
                }
            }
        }
        notes.removed_actions += strip_additional_actions(&mut page)?;
        // PDF/A-2 y -3 (6.5.2) no admiten ni la entrada /AA de página, aunque quede vacía.
        if level.allows_transparency() && get(&page, "AA").is_some() {
            page.dict_delete("AA")?;
        }
        fix_annotations(&mut doc, p, &mut notes, level)?;
    }

    if !notes.unembeddable.is_empty() {
        return Err(Error::Unsupported(format!(
            "no se puede convertir a {} porque estas fuentes no están incrustadas y no se pueden sustituir de forma segura: {}. Incrusta las fuentes en el programa que creó el documento",
            level.name(),
            notes.unembeddable.join(", ")
        )));
    }

    ctx.step(pages as u64 + 1, pages as u64 + 2, "Escribiendo metadatos")?;
    write_metadata_and_intent(&mut doc, level)?;
    let seed = std::fs::read(input)
        .map(|b| b.len().to_le_bytes().to_vec())
        .unwrap_or_default();
    ensure_trailer_id(&mut doc, &[input.as_os_str().as_encoded_bytes(), &seed].concat())?;

    let mut options = WriteOptions::default();
    options.set_garbage_level(1);
    save_atomic(&doc, input, output, options)?;

    // Comprobación final con la validación propia: lo que quede se avisa, no se oculta.
    let result = crate::validate::validate(output, level)?;
    let mut report = Report::with_output(output, summarize(level, &notes));
    report.warnings = notes.warnings;
    for i in result.issues {
        report.warnings.push(format!("Sigue sin cumplir: {}", i.message));
    }
    if !report.warnings.iter().any(|w| w.starts_with("Sigue")) {
        report
            .summary
            .push_str(" Cumple las reglas comprobadas; para una validación completa usa veraPDF.");
    }
    ctx.step(pages as u64 + 2, pages as u64 + 2, "Listo")?;
    Ok(report)
}
