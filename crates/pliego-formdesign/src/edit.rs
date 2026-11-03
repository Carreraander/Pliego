use std::collections::HashSet;

use mupdf::Buffer;
use mupdf::pdf::{PdfDocument, PdfObject};
use pliego_core::{Error, Result};

use crate::ap::rebuild_ap;
use crate::objects::*;
use crate::pe;
use crate::read::page_widgets;
use crate::spec::{FieldPatch, FieldSpec, FieldType};

/// Diccionario o arreglo hijo de `parent[key]`; si falta lo crea con `src`. Se vuelve a leer tras escribirlo
/// porque `try_clone` hace una copia profunda de los objetos directos.
fn child(doc: &PdfDocument, parent: &mut PdfObject, key: &str, src: &str, want_array: bool) -> Result<PdfObject> {
    if let Some(c) = parent.get_dict(key).map_err(pe)?
        && (if want_array { c.is_array() } else { c.is_dict() }).unwrap_or(false)
    {
        return Ok(c);
    }
    put(parent, key, obj(doc, src)?)?;
    parent
        .get_dict(key)
        .map_err(pe)?
        .ok_or_else(|| Error::Pdf(format!("no se pudo crear /{key}")))
}

/// Devuelve el `/AcroForm` (lo crea si falta) con fuente por defecto y `NeedAppearances`.
fn acroform(doc: &mut PdfDocument) -> Result<PdfObject> {
    let mut cat = doc.catalog().map_err(pe)?;
    let mut af = match cat.get_dict("AcroForm").map_err(pe)? {
        Some(a) if a.is_dict().unwrap_or(false) => a,
        _ => {
            let o = obj(doc, "<< /Fields [] >>")?;
            let ind = doc.add_object(&o).map_err(pe)?;
            put(&mut cat, "AcroForm", ind.try_clone().map_err(pe)?)?;
            ind
        }
    };
    child(doc, &mut af, "Fields", "[]", true)?;
    if af.get_dict("DA").map_err(pe)?.is_none() {
        put(&mut af, "DA", obj(doc, &pdf_str(&da(0.0)))?)?;
    }
    let mut dr = child(doc, &mut af, "DR", "<< >>", false)?;
    let mut fonts = child(doc, &mut dr, "Font", "<< >>", false)?;
    if fonts.get_dict("Helv").map_err(pe)?.is_none() {
        let f = obj(
            doc,
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>",
        )?;
        let ind = doc.add_object(&f).map_err(pe)?;
        put(&mut fonts, "Helv", ind)?;
    }
    put(&mut af, "NeedAppearances", doc.new_bool(true))?;
    Ok(af)
}

fn exists(doc: &PdfDocument, name: &str) -> Result<bool> {
    for p in 0..doc.page_count().map_err(pe)? {
        if page_widgets(doc, p)?.iter().any(|(n, _)| n == name) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Flujo de apariencia (formulario XObject) con el contenido dado.
fn form_xobject(doc: &mut PdfDocument, w: f32, h: f32, content: &str) -> Result<PdfObject> {
    let dict = obj(
        doc,
        &format!("<< /Type /XObject /Subtype /Form /BBox [0 0 {w} {h}] /Resources << >> >>"),
    )?;
    let buf = Buffer::from_bytes(content.as_bytes()).map_err(pe)?;
    doc.add_stream(&buf, Some(&dict), false).map_err(pe)
}

/// Aspecto de casilla (cruz) o radio (punto) y de reposo.
fn button_ap(doc: &mut PdfDocument, kind: FieldType, on: &str, w: f32, h: f32) -> Result<PdfObject> {
    let frame = format!("0 G 1 w 0.5 0.5 {} {} re S", w - 1.0, h - 1.0);
    let mark = if kind == FieldType::RadioButton {
        let (cx, cy, r) = (w / 2.0, h / 2.0, (w.min(h) / 2.0 - 3.0).max(1.0));
        let k = 0.5523 * r;
        format!(
            "0 g {} {} m {} {} {} {} {} {} c {} {} {} {} {} {} c {} {} {} {} {} {} c {} {} {} {} {} {} c f",
            cx + r,
            cy,
            cx + r,
            cy + k,
            cx + k,
            cy + r,
            cx,
            cy + r,
            cx - k,
            cy + r,
            cx - r,
            cy + k,
            cx - r,
            cy,
            cx - r,
            cy - k,
            cx - k,
            cy - r,
            cx,
            cy - r,
            cx + k,
            cy - r,
            cx + r,
            cy - k,
            cx + r,
            cy,
        )
    } else {
        format!(
            "0 G 1.5 w 3 3 m {} {} l S 3 {} m {} 3 l S",
            w - 3.0,
            h - 3.0,
            h - 3.0,
            w - 3.0
        )
    };
    let off = form_xobject(doc, w, h, &frame)?;
    let onx = form_xobject(doc, w, h, &format!("{frame} {mark}"))?;
    let mut n = obj(doc, "<< >>")?;
    put(&mut n, on, onx)?;
    put(&mut n, "Off", off)?;
    let mut ap = obj(doc, "<< >>")?;
    put(&mut ap, "N", n)?;
    Ok(ap)
}

/// Aspecto fijo de un campo de firma: caja con fondo suave.
fn signature_ap(doc: &mut PdfDocument, w: f32, h: f32) -> Result<PdfObject> {
    let n = form_xobject(
        doc,
        w,
        h,
        &format!(
            "0.93 0.95 1 rg 0 0 {w} {h} re f 0.3 0.3 0.6 RG 1 w 0.5 0.5 {} {} re S",
            w - 1.0,
            h - 1.0
        ),
    )?;
    let mut ap = obj(doc, "<< >>")?;
    put(&mut ap, "N", n)?;
    Ok(ap)
}

fn append_unique(arr: &mut PdfObject, item: PdfObject) -> Result<()> {
    arr.array_push(item).map_err(pe)
}

fn page_annots(doc: &PdfDocument, page: usize) -> Result<PdfObject> {
    let mut p = doc.find_page(page as i32).map_err(pe)?;
    child(doc, &mut p, "Annots", "[]", true)
}

/// Añade un campo al documento.
///
/// Los radios con el mismo nombre se agrupan: el primero crea el grupo y los siguientes añaden botones
/// (cada uno con su `export_value`).
pub fn add_field(doc: &mut PdfDocument, spec: &FieldSpec) -> Result<()> {
    check_name(&spec.name)?;
    let rect = check_rect(spec.rect)?;
    let pages = doc.page_count().map_err(pe)?;
    if spec.page as i32 >= pages {
        return Err(Error::InvalidInput(format!("la página {} no existe", spec.page + 1)));
    }
    let kind = spec.kind;
    let radio_member = kind == FieldType::RadioButton && exists(doc, &spec.name)?;
    if exists(doc, &spec.name)? && !radio_member {
        return Err(Error::InvalidInput(format!(
            "ya existe un campo llamado «{}»",
            spec.name
        )));
    }
    let on = match kind {
        FieldType::CheckBox => spec.export_value.clone().unwrap_or_else(|| "Yes".into()),
        FieldType::RadioButton => spec
            .export_value
            .clone()
            .filter(|v| !v.is_empty() && v != "Off")
            .ok_or_else(|| Error::InvalidInput("un botón de radio necesita un valor de exportación".into()))?,
        _ => String::new(),
    };
    let (w, h) = (rect[2] - rect[0], rect[3] - rect[1]);
    let af = acroform(doc)?;
    let mut fields = af
        .get_dict("Fields")
        .map_err(pe)?
        .ok_or_else(|| Error::Pdf("el formulario no tiene /Fields".into()))?;

    let ft = match kind {
        FieldType::Text => "/FT /Tx",
        FieldType::ComboBox | FieldType::ListBox => "/FT /Ch",
        FieldType::Signature => "/FT /Sig",
        _ => "/FT /Btn",
    };
    let mut ff = 0u32;
    if spec.read_only {
        ff |= F_READ_ONLY;
    }
    if spec.required {
        ff |= F_REQUIRED;
    }
    match kind {
        FieldType::Text => {
            if spec.multiline {
                ff |= F_MULTILINE;
            }
            if spec.password {
                ff |= F_PASSWORD;
            }
        }
        FieldType::ComboBox => ff |= F_COMBO,
        FieldType::RadioButton => ff |= F_RADIO | F_NO_TOGGLE_OFF,
        FieldType::PushButton => ff |= F_PUSHBUTTON,
        _ => {}
    }
    let checked = match kind {
        FieldType::CheckBox | FieldType::RadioButton => spec
            .value
            .as_deref()
            .is_some_and(|v| v == on || v.eq_ignore_ascii_case("true") || v == "1"),
        _ => false,
    };
    let state = if checked { pdf_name(&on) } else { "/Off".into() };

    // Diccionario del widget (con sus datos de campo si no depende de un padre).
    let mut src = format!(
        "<< /Type /Annot /Subtype /Widget /Rect {} /F 4 /DA {}",
        rect_src(rect),
        pdf_str(&da(spec.font_size))
    );
    let has_border = !matches!(kind, FieldType::Signature);
    if has_border {
        let bg = if kind == FieldType::PushButton {
            "[0.85 0.85 0.85]"
        } else {
            "[1 1 1]"
        };
        src.push_str(&format!(
            " /MK << /BC [0 0 0] /BG {bg}{} >> /BS << /W 1 /S /S >>",
            match (&spec.caption, kind) {
                (Some(c), FieldType::PushButton) => format!(" /CA {}", pdf_str(c)),
                _ => String::new(),
            }
        ));
    }
    if !radio_member {
        if kind != FieldType::RadioButton {
            src.push_str(&format!(" {ft} /T {}", pdf_str(&spec.name)));
            if ff != 0 {
                src.push_str(&format!(" /Ff {ff}"));
            }
        }
        if let Some(t) = spec.tooltip.as_deref().filter(|t| !t.is_empty())
            && kind != FieldType::RadioButton
        {
            src.push_str(&format!(" /TU {}", pdf_str(t)));
        }
    }
    if kind != FieldType::RadioButton {
        match (kind, spec.value.as_deref()) {
            (FieldType::CheckBox, _) => src.push_str(&format!(" /V {state} /DV {state}")),
            (FieldType::PushButton | FieldType::Signature, _) | (_, None) => {}
            (_, Some(v)) if !v.is_empty() => src.push_str(&format!(" /V {} /DV {}", pdf_str(v), pdf_str(v))),
            _ => {}
        }
        if kind == FieldType::Text
            && let Some(m) = spec.max_len.filter(|m| *m > 0)
        {
            src.push_str(&format!(" /MaxLen {m}"));
        }
        if matches!(kind, FieldType::ComboBox | FieldType::ListBox) {
            src.push_str(" /Opt [");
            for o in &spec.options {
                src.push_str(&pdf_str(o));
                src.push(' ');
            }
            src.push(']');
        }
    }
    if matches!(kind, FieldType::CheckBox | FieldType::RadioButton) {
        src.push_str(&format!(" /AS {state}"));
    }
    src.push_str(" >>");

    let mut widget = obj(doc, &src)?;
    match kind {
        FieldType::CheckBox | FieldType::RadioButton => {
            let ap = button_ap(doc, kind, &on, w, h)?;
            put(&mut widget, "AP", ap)?;
        }
        FieldType::Signature => {
            let ap = signature_ap(doc, w, h)?;
            put(&mut widget, "AP", ap)?;
        }
        _ => {}
    }
    if kind == FieldType::Signature {
        // Sin /MK no hay borde propio; el aspecto ya lo dibuja.
        put(&mut widget, "MK", obj(doc, "<< >>")?)?;
    }

    if kind == FieldType::RadioButton {
        let parent = if radio_member {
            find_field_object(doc, &spec.name)?.ok_or_else(|| Error::Pdf("no se encontró el grupo de radio".into()))?
        } else {
            let mut psrc = format!("<< /FT /Btn /Ff {ff} /T {} /V /Off /Kids [] >>", pdf_str(&spec.name));
            if let Some(t) = spec.tooltip.as_deref().filter(|t| !t.is_empty()) {
                psrc = psrc.replace(" /Kids", &format!(" /TU {} /Kids", pdf_str(t)));
            }
            let p = obj(doc, &psrc)?;
            let ind = doc.add_object(&p).map_err(pe)?;
            append_unique(&mut fields, ind.try_clone().map_err(pe)?)?;
            ind
        };
        if radio_member && !parent_is_radio(&parent) {
            return Err(Error::InvalidInput(format!(
                "«{}» ya existe y no es un grupo de radio",
                spec.name
            )));
        }
        put(&mut widget, "Parent", parent.try_clone().map_err(pe)?)?;
        let ind = doc.add_object(&widget).map_err(pe)?;
        let mut parent = parent;
        let mut kids = parent
            .get_dict("Kids")
            .map_err(pe)?
            .ok_or_else(|| Error::Pdf("el grupo de radio no tiene /Kids".into()))?;
        append_unique(&mut kids, ind.try_clone().map_err(pe)?)?;
        if checked {
            put(&mut parent, "V", obj(doc, &pdf_name(&on))?)?;
            put(&mut parent, "DV", obj(doc, &pdf_name(&on))?)?;
        }
        append_unique(&mut page_annots(doc, spec.page)?, ind)?;
        return Ok(());
    }

    let ind = doc.add_object(&widget).map_err(pe)?;
    append_unique(&mut fields, ind.try_clone().map_err(pe)?)?;
    let x = xref_of(&ind);
    append_unique(&mut page_annots(doc, spec.page)?, ind)?;
    // Apariencia propia de MuPDF para texto, listas y botones.
    if matches!(
        kind,
        FieldType::Text | FieldType::ComboBox | FieldType::ListBox | FieldType::PushButton
    ) {
        let page = doc.load_pdf_page(spec.page as i32).map_err(pe)?;
        if let Some(w) = page.load_widget(x).map_err(pe)? {
            rebuild_ap(doc, &mut w.annotation().object())?;
        }
    }
    Ok(())
}

fn parent_is_radio(field: &PdfObject) -> bool {
    flags_of(field) & F_RADIO != 0
}

/// Objeto de campo (el que lleva `/T`) del primer widget con ese nombre completo.
fn find_field_object(doc: &PdfDocument, name: &str) -> Result<Option<PdfObject>> {
    for p in 0..doc.page_count().map_err(pe)? {
        for (n, w) in page_widgets(doc, p)? {
            if n == name {
                return Ok(Some(field_of(&w.annotation().object())?));
            }
        }
    }
    Ok(None)
}

/// Modifica propiedades de un campo existente.
pub fn update_field(doc: &mut PdfDocument, name: &str, patch: &FieldPatch) -> Result<()> {
    let mut targets: Vec<(i32, i32)> = Vec::new(); // (página, xref)
    for p in 0..doc.page_count().map_err(pe)? {
        for (n, w) in page_widgets(doc, p)? {
            if n == name {
                targets.push((p, w.xref().map_err(pe)?));
            }
        }
    }
    if targets.is_empty() {
        return Err(Error::InvalidInput(format!("el campo «{name}» no existe")));
    }
    if let Some(new) = &patch.name {
        check_name(new)?;
        if new != name && exists(doc, &leaf_renamed(name, new))? {
            return Err(Error::InvalidInput(format!("ya existe un campo llamado «{new}»")));
        }
    }
    if let Some(r) = patch.rect
        && targets.len() > 1
    {
        check_rect(r)?;
        return Err(Error::InvalidInput(
            "el campo tiene varios botones; mueve cada uno por separado".into(),
        ));
    }
    let rect = patch.rect.map(check_rect).transpose()?;

    let first = {
        let page = doc.load_pdf_page(targets[0].0).map_err(pe)?;
        page.load_widget(targets[0].1)
            .map_err(pe)?
            .ok_or_else(|| Error::Pdf("no se encontró el campo".into()))?
            .annotation()
            .object()
    };
    let mut field = field_of(&first)?;
    let is_btn = field
        .get_dict_inheritable("FT")
        .map_err(pe)?
        .and_then(|t| t.as_name().ok())
        .is_some_and(|t| t == b"Btn");

    if let Some(v) = patch.required {
        set_flag(&mut field, doc, F_REQUIRED, v)?;
    }
    if let Some(v) = patch.read_only {
        set_flag(&mut field, doc, F_READ_ONLY, v)?;
    }
    if let Some(v) = patch.multiline {
        set_flag(&mut field, doc, F_MULTILINE, v)?;
    }
    if let Some(v) = patch.password {
        set_flag(&mut field, doc, F_PASSWORD, v)?;
    }
    if let Some(m) = patch.max_len {
        if m == 0 {
            field.dict_delete("MaxLen").map_err(pe)?;
        } else {
            put(&mut field, "MaxLen", doc.new_int(m as i32).map_err(pe)?)?;
        }
    }
    if let Some(t) = &patch.tooltip {
        if t.is_empty() {
            field.dict_delete("TU").map_err(pe)?;
        } else {
            put(&mut field, "TU", obj(doc, &pdf_str(t))?)?;
        }
    }
    if let Some(opts) = &patch.options {
        let mut src = String::from("[");
        for o in opts {
            src.push_str(&pdf_str(o));
            src.push(' ');
        }
        src.push(']');
        put(&mut field, "Opt", obj(doc, &src)?)?;
    }
    if let Some(new) = &patch.name {
        put(&mut field, "T", obj(doc, &pdf_str(new))?)?;
    }
    if let Some(v) = &patch.value {
        set_default_value(doc, &mut field, is_btn, v, &targets)?;
    }

    for (p, x) in &targets {
        let page = doc.load_pdf_page(*p).map_err(pe)?;
        let Some(w) = page.load_widget(*x).map_err(pe)? else {
            continue;
        };
        let mut o = w.annotation().object();
        if let Some(r) = rect {
            put(&mut o, "Rect", obj(doc, &rect_src(r))?)?;
        }
        if let Some(fs) = patch.font_size {
            put(&mut o, "DA", obj(doc, &pdf_str(&da(fs)))?)?;
        }
        if !is_btn
            && field
                .get_dict_inheritable("FT")
                .map_err(pe)?
                .is_some_and(|t| t.as_name().is_ok_and(|n| n != b"Sig"))
        {
            rebuild_ap(doc, &mut o)?;
        }
    }
    Ok(())
}

fn leaf_renamed(old: &str, new: &str) -> String {
    match old.rsplit_once('.') {
        Some((parent, _)) => format!("{parent}.{new}"),
        None => new.to_string(),
    }
}

fn set_default_value(
    doc: &mut PdfDocument,
    field: &mut PdfObject,
    is_btn: bool,
    v: &str,
    targets: &[(i32, i32)],
) -> Result<()> {
    if v.is_empty() {
        field.dict_delete("V").map_err(pe)?;
        field.dict_delete("DV").map_err(pe)?;
        if is_btn {
            put(field, "V", obj(doc, "/Off")?)?;
            for (p, x) in targets {
                let page = doc.load_pdf_page(*p).map_err(pe)?;
                if let Some(w) = page.load_widget(*x).map_err(pe)? {
                    let mut o = w.annotation().object();
                    put(&mut o, "AS", obj(doc, "/Off")?)?;
                }
            }
        }
        return Ok(());
    }
    if !is_btn {
        put(field, "V", obj(doc, &pdf_str(v))?)?;
        put(field, "DV", obj(doc, &pdf_str(v))?)?;
        return Ok(());
    }
    // Botones: el valor es el estado marcado (o true/false en una casilla).
    let mut chosen = String::from("Off");
    for (p, x) in targets {
        let page = doc.load_pdf_page(*p).map_err(pe)?;
        if let Some(w) = page.load_widget(*x).map_err(pe)? {
            let mut o = w.annotation().object();
            let on = crate::read::on_state(&o);
            let hit = match &on {
                Some(s) => v == s || (v.eq_ignore_ascii_case("true") && targets.len() == 1),
                None => false,
            };
            if hit {
                chosen = on.unwrap_or_default();
            }
            let state = if hit { pdf_name(&chosen) } else { "/Off".into() };
            put(&mut o, "AS", obj(doc, &state)?)?;
        }
    }
    if v.eq_ignore_ascii_case("false") || v == "Off" {
        chosen = "Off".into();
    } else if chosen == "Off" {
        return Err(Error::InvalidInput(format!(
            "«{v}» no es un estado válido de este campo"
        )));
    }
    put(field, "V", obj(doc, &pdf_name(&chosen))?)?;
    put(field, "DV", obj(doc, &pdf_name(&chosen))?)?;
    Ok(())
}

/// Quita de los árboles de campos las referencias a widgets borrados (y los padres que se quedan sin hijos).
fn prune(arr: &mut PdfObject, gone: &HashSet<i32>) -> Result<()> {
    let mut i = 0;
    while i < arr.len().map_err(pe)? {
        let Some(item) = arr.get_array(i as i32).map_err(pe)? else {
            i += 1;
            continue;
        };
        if gone.contains(&xref_of(&item)) {
            arr.array_delete(i as i32).map_err(pe)?;
            continue;
        }
        if let Some(mut kids) = item.get_dict("Kids").map_err(pe)?
            && kids.is_array().unwrap_or(false)
        {
            prune(&mut kids, gone)?;
            if kids.len().map_err(pe)? == 0 {
                arr.array_delete(i as i32).map_err(pe)?;
                continue;
            }
        }
        i += 1;
    }
    Ok(())
}

/// Borra un campo (todos sus widgets) y lo quita del formulario.
pub fn delete_field(doc: &mut PdfDocument, name: &str) -> Result<()> {
    let mut gone = HashSet::new();
    for p in 0..doc.page_count().map_err(pe)? {
        let mut page = doc.load_pdf_page(p).map_err(pe)?;
        let hits: Vec<_> = page
            .widgets()
            .filter(|w| w.name().ok().flatten().as_deref() == Some(name))
            .collect();
        for w in hits {
            gone.insert(w.xref().map_err(pe)?);
            page.delete_widget(w).map_err(pe)?;
        }
    }
    if gone.is_empty() {
        return Err(Error::InvalidInput(format!("el campo «{name}» no existe")));
    }
    let af = doc.catalog().map_err(pe)?.get_dict("AcroForm").map_err(pe)?;
    if let Some(af) = af
        && let Some(mut fields) = af.get_dict("Fields").map_err(pe)?
    {
        prune(&mut fields, &gone)?;
    }
    Ok(())
}
