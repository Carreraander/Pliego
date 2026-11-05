use std::path::Path;

use pliego_pdf::{PdfDocument, Object, WriteOptions};
use pliego_core::{JobContext, Report, Result};
use serde::{Deserialize, Serialize};

use crate::io::{open_pdf, save_atomic};

/// Qué quitar. Por defecto, todo. Los objetos que queden sin referencia se descartan siempre al guardar:
/// si no, lo "eliminado" seguiría dentro del archivo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SanitizeOptions {
    /// Información del documento (Info), XMP y datos privados de aplicaciones (PieceInfo).
    pub metadata: bool,
    /// JavaScript de documento, de páginas, de campos y de enlaces.
    pub javascript: bool,
    /// Acción al abrir el documento.
    pub open_actions: bool,
    /// Anotaciones de archivo adjunto.
    pub attachments: bool,
    /// Archivos incrustados (EmbeddedFiles, AF).
    pub embedded_files: bool,
    /// Comentarios y marcas (todas las anotaciones salvo enlaces, campos y marcas de redacción).
    pub comments: bool,
    /// Valores de los campos de formulario.
    pub form_data: bool,
    /// Miniaturas de página.
    pub thumbnails: bool,
    /// Imágenes, formularios XObject y anotaciones que pertenecen a capas ocultas (desactivadas por defecto).
    /// El texto o los dibujos de una capa oculta que van sueltos en el contenido de la página no se pueden
    /// separar así: se avisa en el informe y hay que cubrirlos con una redacción.
    pub hidden_layers: bool,
}

impl Default for SanitizeOptions {
    fn default() -> Self {
        Self {
            metadata: true,
            javascript: true,
            open_actions: true,
            attachments: true,
            embedded_files: true,
            comments: true,
            form_data: true,
            thumbnails: true,
            hidden_layers: true,
        }
    }
}

/// Una línea del informe: qué se quitó y cuántos elementos.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemovedItem {
    pub kind: String,
    pub count: usize,
}

#[derive(Default)]
struct Tally(Vec<RemovedItem>);

impl Tally {
    fn add(&mut self, kind: &str, n: usize) {
        if n == 0 {
            return;
        }
        match self.0.iter_mut().find(|i| i.kind == kind) {
            Some(i) => i.count += n,
            None => self.0.push(RemovedItem {
                kind: kind.into(),
                count: n,
            }),
        }
    }
}

fn name_of(obj: &Object, key: &str) -> Option<String> {
    let v = obj.get_dict(key).ok()??;
    String::from_utf8(v.as_name().ok()?).ok()
}

/// Quita la clave si existe; devuelve si había algo que quitar.
fn remove_key(obj: &mut Object, key: &str) -> Result<bool> {
    let had = obj.get_dict(key)?.is_some();
    if had {
        obj.dict_delete(key)?;
    }
    Ok(had)
}

/// Cuenta las claves de un diccionario (para describir el tamaño de Info).
fn dict_len(obj: &Object) -> usize {
    obj.dict_len().unwrap_or(1)
}

fn is_form_field_button(field: &Object) -> bool {
    name_of_inherited(field, "FT").as_deref() == Some("Btn")
}

fn name_of_inherited(obj: &Object, key: &str) -> Option<String> {
    let v = obj.get_dict_inheritable(key).ok()??;
    String::from_utf8(v.as_name().ok()?).ok()
}

fn is_pushbutton(field: &Object) -> bool {
    let ff = field
        .get_dict_inheritable("Ff")
        .ok()
        .flatten()
        .and_then(|v| v.as_int().ok())
        .unwrap_or(0);
    ff & (1 << 16) != 0
}

/// Vacía los valores de un campo y, recursivamente, de sus hijos. Devuelve cuántos campos tenían valor.
fn clear_field(field: &mut Object, depth: u32) -> Result<usize> {
    if depth > 32 {
        return Ok(0);
    }
    let mut n = 0;
    if remove_key(field, "V")? {
        n += 1;
    }
    match name_of_inherited(field, "FT").as_deref() {
        // El aspecto guardado mostraría el valor antiguo: se descarta y el lector lo regenera.
        Some("Tx") | Some("Ch") => {
            remove_key(field, "AP")?;
        }
        Some("Btn")
            if is_form_field_button(field)
                && !is_pushbutton(field)
                && field.get_dict("AS")?.is_some() =>
        {
            field
                .dict_put("AS", Object::new_name("Off")?)
                ?;
        }
        _ => {}
    }
    if let Some(kids) = field.get_dict("Kids")? {
        for i in 0..kids.len()? {
            if let Some(mut kid) = kids.get_array(i as i32)? {
                n += clear_field(&mut kid, depth + 1)?;
            }
        }
    }
    Ok(n)
}

/// ¿Es una acción de JavaScript?
fn is_js_action(action: &Object) -> bool {
    name_of(action, "S").as_deref() == Some("JavaScript")
}

/// Números de objeto de las capas (OCG) desactivadas en la configuración por defecto.
fn hidden_ocgs(catalog: &Object) -> Result<Vec<i32>> {
    let Some(props) = catalog.get_dict("OCProperties")? else {
        return Ok(Vec::new());
    };
    let Some(default) = props.get_dict("D")? else {
        return Ok(Vec::new());
    };
    let refs = |key: &str| -> Result<Vec<i32>> {
        let mut out = Vec::new();
        if let Some(arr) = default.get_dict(key)? {
            for i in 0..arr.len()? {
                if let Some(o) = arr.get_array(i as i32)?
                    && o.is_indirect()?
                {
                    out.push(o.as_indirect()?);
                }
            }
        }
        Ok(out)
    };
    if name_of(&default, "BaseState").as_deref() == Some("OFF") {
        // Todas apagadas salvo las de /ON.
        let on = refs("ON")?;
        let mut all = refs_of_array(props.get_dict("OCGs")?)?;
        all.retain(|n| !on.contains(n));
        Ok(all)
    } else {
        refs("OFF")
    }
}

fn refs_of_array(arr: Option<Object>) -> Result<Vec<i32>> {
    let mut out = Vec::new();
    if let Some(arr) = arr {
        for i in 0..arr.len()? {
            if let Some(o) = arr.get_array(i as i32)?
                && o.is_indirect()?
            {
                out.push(o.as_indirect()?);
            }
        }
    }
    Ok(out)
}

/// ¿El objeto pertenece solo a capas ocultas? (`/OC` apunta a una capa apagada, o a un grupo de
/// pertenencia cuyas capas están todas apagadas).
fn in_hidden_layer(obj: &Object, hidden: &[i32]) -> Result<bool> {
    let Some(oc) = obj.get_dict("OC")? else {
        return Ok(false);
    };
    if oc.is_indirect()? && hidden.contains(&oc.as_indirect()?) {
        return Ok(true);
    }
    // Grupo de pertenencia (OCMD): /OCGs es una capa o una lista de capas.
    if let Some(g) = oc.get_dict("OCGs")? {
        let ids = if g.is_array()? {
            refs_of_array(Some(g))?
        } else if g.is_indirect()? {
            vec![g.as_indirect()?]
        } else {
            Vec::new()
        };
        return Ok(!ids.is_empty() && ids.iter().all(|n| hidden.contains(n)));
    }
    Ok(false)
}

/// Saneado sobre un documento ya abierto (sin guardar). Devuelve lo que quitó.
///
/// Los objetos sin referencia no se quitan aquí sino al guardar (ver [`sanitize`]).
pub fn sanitize_document(
    doc: &mut PdfDocument,
    opts: &SanitizeOptions,
    ctx: &JobContext<'_>,
) -> Result<Vec<RemovedItem>> {
    let mut tally = Tally::default();
    let mut catalog = doc.catalog()?;
    let mut trailer = doc.trailer()?;

    if opts.metadata {
        if let Some(info) = trailer.get_dict("Info")? {
            tally.add("Información del documento (título, autor...)", dict_len(&info));
            trailer.dict_delete("Info")?;
        }
        if remove_key(&mut catalog, "Metadata")? {
            tally.add("Metadatos XMP", 1);
        }
        if remove_key(&mut catalog, "PieceInfo")? {
            tally.add("Datos privados de aplicaciones", 1);
        }
    }
    if opts.open_actions && remove_key(&mut catalog, "OpenAction")? {
        tally.add("Acción al abrir", 1);
    }
    if opts.javascript {
        if let Some(mut names) = catalog.get_dict("Names")?
            && remove_key(&mut names, "JavaScript")?
        {
            tally.add("JavaScript de documento", 1);
        }
        if remove_key(&mut catalog, "AA")? {
            tally.add("Acciones automáticas del documento", 1);
        }
    }
    if opts.embedded_files {
        if let Some(mut names) = catalog.get_dict("Names")?
            && remove_key(&mut names, "EmbeddedFiles")?
        {
            tally.add("Archivos incrustados", 1);
        }
        if remove_key(&mut catalog, "AF")? {
            tally.add("Archivos asociados", 1);
        }
    }
    if opts.form_data
        && let Some(mut form) = catalog.get_dict("AcroForm")?
    {
        let mut n = 0;
        if let Some(fields) = form.get_dict("Fields")? {
            for i in 0..fields.len()? {
                if let Some(mut f) = fields.get_array(i as i32)? {
                    n += clear_field(&mut f, 0)?;
                }
            }
        }
        if n > 0 {
            form.dict_put("NeedAppearances", Object::new_bool(true))
                ?;
        }
        tally.add("Valores de formulario", n);
    }

    let hidden = if opts.hidden_layers {
        hidden_ocgs(&catalog)?
    } else {
        Vec::new()
    };
    let pages = doc.page_count()?.max(0);
    for p in 0..pages {
        ctx.step(p as u64, pages as u64, "Saneando páginas")?;
        let mut page = doc.find_page(p)?;
        if opts.thumbnails && remove_key(&mut page, "Thumb")? {
            tally.add("Miniaturas", 1);
        }
        if opts.metadata {
            if remove_key(&mut page, "PieceInfo")? {
                tally.add("Datos privados de aplicaciones", 1);
            }
            if remove_key(&mut page, "Metadata")? {
                tally.add("Metadatos XMP", 1);
            }
        }
        if opts.javascript && remove_key(&mut page, "AA")? {
            tally.add("Acciones automáticas de página", 1);
        }
        if !hidden.is_empty() {
            remove_hidden_xobjects(&page, &hidden, &mut tally)?;
        }
        sanitize_annots(&mut page, opts, &hidden, &mut tally)?;
    }
    ctx.step(pages as u64, pages as u64, "Saneado completado")?;
    Ok(tally.0)
}

/// Quita de los recursos de la página las imágenes y formularios que están en capas ocultas.
fn remove_hidden_xobjects(page: &Object, hidden: &[i32], tally: &mut Tally) -> Result<()> {
    let Some(res) = page.get_dict_inheritable("Resources")? else {
        return Ok(());
    };
    let Some(mut xobjs) = res.get_dict("XObject")? else {
        return Ok(());
    };
    let mut doomed = Vec::new();
    for (key, val) in xobjs.dict_iter()? {
        if in_hidden_layer(&val, hidden)? {
            doomed.push(key);
        }
    }
    for name in doomed {
        xobjs.dict_delete(name.as_str())?;
        tally.add("Imágenes y formularios de capas ocultas", 1);
    }
    Ok(())
}

fn sanitize_annots(page: &mut Object, opts: &SanitizeOptions, hidden: &[i32], tally: &mut Tally) -> Result<()> {
    let Some(annots) = page.get_dict("Annots")? else {
        return Ok(());
    };
    let mut annots = annots;
    let len = annots.len()?;
    // De atrás hacia delante para que los índices sigan siendo válidos al borrar.
    for i in (0..len).rev() {
        let Some(mut annot) = annots.get_array(i as i32)? else {
            continue;
        };
        let subtype = name_of(&annot, "Subtype").unwrap_or_default();
        if !hidden.is_empty() && in_hidden_layer(&annot, hidden)? {
            annots.array_delete(i as i32)?;
            tally.add("Anotaciones de capas ocultas", 1);
            continue;
        }
        let remove = match subtype.as_str() {
            "FileAttachment" => opts.attachments,
            // Enlaces, campos y marcas de redacción pendientes se conservan.
            "Link" | "Widget" | "Redact" => false,
            _ => opts.comments,
        };
        if remove {
            annots.array_delete(i as i32)?;
            if subtype == "FileAttachment" {
                tally.add("Adjuntos", 1);
            } else {
                tally.add("Comentarios y marcas", 1);
            }
            continue;
        }
        if opts.javascript {
            if remove_key(&mut annot, "AA")? {
                tally.add("Acciones automáticas de campos y enlaces", 1);
            }
            if let Some(action) = annot.get_dict("A")?
                && is_js_action(&action)
            {
                annot.dict_delete("A")?;
                tally.add("Enlaces con JavaScript", 1);
            }
        }
    }
    Ok(())
}

/// Sanea `input` y escribe `output`. El informe lista lo que se quitó, una línea por tipo.
pub fn sanitize(
    input: &Path,
    output: &Path,
    password: Option<&str>,
    opts: &SanitizeOptions,
    ctx: &JobContext<'_>,
) -> Result<Report> {
    let mut doc = open_pdf(input, password)?;
    let removed = sanitize_document(&mut doc, opts, ctx)?;
    let mut write = WriteOptions::default();
    // Nivel 3: quita objetos sin referencia, renumera y deduplica.
    write.set_garbage(true).set_garbage_level(3);
    save_atomic(doc, input, output, write)?;

    let mut report = Report::with_output(output, String::new());
    report.summary = if removed.is_empty() {
        "No se encontró nada que eliminar.".to_owned()
    } else {
        let mut s = String::from("Eliminado:");
        for r in &removed {
            s.push_str(&format!("\n- {}: {}", r.kind, r.count));
        }
        s
    };
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn dirty() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/protect/sucio.pdf")
    }

    fn raw_contains(path: &Path, needle: &str) -> bool {
        let b = std::fs::read(path).unwrap();
        b.windows(needle.len()).any(|w| w == needle.as_bytes())
    }

    fn run(opts: &SanitizeOptions) -> (tempfile::TempDir, PathBuf, Report) {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("limpio.pdf");
        let rep = sanitize(&dirty(), &out, None, opts, &JobContext::silent()).unwrap();
        (dir, out, rep)
    }

    #[test]
    fn dirty_corpus_really_is_dirty() {
        for s in [
            "Autora Privada",
            "Autor secreto XMP",
            "JavaScript",
            "Comentario privado",
            "datos adjuntos secretos",
            "Valor del formulario",
            "/Thumb",
            "Capa oculta secreta",
        ] {
            assert!(raw_contains(&dirty(), s), "el corpus debería contener {s}");
        }
    }

    #[test]
    fn removes_everything_by_default() {
        let (_d, out, rep) = run(&SanitizeOptions::default());
        for s in [
            "Autora Privada",
            "Titulo privado",
            "Autor secreto XMP",
            "app.alert",
            "Comentario privado",
            "datos adjuntos secretos",
            "secreto.txt",
            "Valor del formulario",
            "/Thumb",
            "/OpenAction",
            "Capa oculta secreta",
        ] {
            assert!(!raw_contains(&out, s), "no debería quedar {s}");
        }
        for kind in [
            "Información del documento",
            "Metadatos XMP",
            "Adjuntos",
            "Comentarios y marcas",
            "Miniaturas",
        ] {
            assert!(
                rep.summary.contains(kind),
                "el informe debe mencionar {kind}:\n{}",
                rep.summary
            );
        }
        // El documento sigue siendo válido y conserva el texto de la página.
        let doc = open_pdf(&out, None).unwrap();
        let page = doc.load_page(0).unwrap();
        let text = page
            .to_text_page(pliego_pdf::TextFlags::empty())
            .unwrap()
            .to_text()
            .unwrap();
        assert!(text.contains("Contenido confidencial"));
    }

    #[test]
    fn options_are_independent() {
        let opts = SanitizeOptions {
            metadata: false,
            comments: false,
            attachments: false,
            embedded_files: false,
            form_data: false,
            hidden_layers: false,
            ..Default::default()
        };
        let (_d, out, _) = run(&opts);
        assert!(raw_contains(&out, "Capa oculta secreta"));
        assert!(raw_contains(&out, "Autora Privada"));
        assert!(raw_contains(&out, "Comentario privado"));
        assert!(raw_contains(&out, "datos adjuntos secretos"));
        assert!(!raw_contains(&out, "app.alert"));
        assert!(!raw_contains(&out, "/Thumb"));
    }

    #[test]
    fn clean_document_reports_nothing_to_remove() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("1.pdf");
        let second = dir.path().join("2.pdf");
        sanitize(
            &dirty(),
            &first,
            None,
            &SanitizeOptions::default(),
            &JobContext::silent(),
        )
        .unwrap();
        let rep = sanitize(
            &first,
            &second,
            None,
            &SanitizeOptions::default(),
            &JobContext::silent(),
        )
        .unwrap();
        assert_eq!(rep.summary, "No se encontró nada que eliminar.");
    }
}
