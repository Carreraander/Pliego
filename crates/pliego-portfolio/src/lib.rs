//! Carteras PDF (colecciones): un PDF que contiene otros archivos, con vista de detalles.
//!
//! - [`create`] crea una cartera nueva con una portada en blanco y los archivos incrustados.
//! - [`list`] lista los archivos incrustados de cualquier PDF y dice si es una cartera (`Collection`).
//! - [`extract`] guarda los archivos en una carpeta, sin salirse de ella y sin pisar nada existente.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use pliego_core::{Error, JobContext, Lang, Report, Result};
use pliego_pdf::{Document, Size};
use pliego_pdf::{EmbeddedFileOptions, Object, PdfDocument, WriteOptions};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// Nombre en el árbol de nombres de la cartera.
    pub name: String,
    /// Nombre de archivo original.
    pub filename: String,
    pub mime_type: Option<String>,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Listing {
    /// ¿El PDF declara una colección (`Collection`), es decir, se muestra como cartera?
    pub is_portfolio: bool,
    pub entries: Vec<Entry>,
}

fn mime_of(name: &str) -> Option<&'static str> {
    let ext = Path::new(name).extension()?.to_str()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "pdf" => "application/pdf",
        "txt" => "text/plain",
        "csv" => "text/csv",
        "html" | "htm" => "text/html",
        "xml" => "application/xml",
        "json" => "application/json",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "tif" | "tiff" => "image/tiff",
        "zip" => "application/zip",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "ppt" => "application/vnd.ms-powerpoint",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        _ => return None,
    })
}

/// Nombre de archivo seguro: solo el último componente, sin separadores ni cosas raras.
fn safe_name(name: &str) -> String {
    let last = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let cleaned: String = last
        .chars()
        .map(|c| {
            if c.is_control() || ":*?\"<>|".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').to_string();
    if cleaned.is_empty() { "archivo".into() } else { cleaned }
}

fn unique(name: &str, taken: &mut HashSet<String>) -> String {
    let path = Path::new(name);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(name);
    let ext = path.extension().and_then(|s| s.to_str());
    let mut candidate = name.to_string();
    let mut n = 2;
    while !taken.insert(candidate.to_lowercase()) {
        candidate = match ext {
            Some(e) => format!("{stem} ({n}).{e}"),
            None => format!("{stem} ({n})"),
        };
        n += 1;
    }
    candidate
}

fn collection_dict(doc: &mut PdfDocument, lang: Lang) -> Result<Object> {
    let field = |doc: &mut PdfDocument, subtype: &str, label: &str, order: i32| -> Result<Object> {
        let mut f = doc.new_dict()?;
        f.dict_put("Type", Object::new_name("CollectionField")?)?;
        f.dict_put("Subtype", Object::new_name(subtype)?)?;
        f.dict_put("N", Object::new_string(label)?)?;
        f.dict_put("O", Object::new_int(order)?)?;
        Ok(f)
    };
    let mut schema = doc.new_dict()?;
    schema.dict_put("Type", Object::new_name("CollectionSchema")?)?;
    let f = field(doc, "F", lang.pick("Nombre", "Name"), 1)?;
    schema.dict_put("F", f)?;
    let s = field(doc, "Size", lang.pick("Tamaño", "Size"), 2)?;
    schema.dict_put("Size", s)?;
    let m = field(doc, "ModDate", lang.pick("Modificado", "Modified"), 3)?;
    schema.dict_put("ModDate", m)?;
    let mut c = doc.new_dict()?;
    c.dict_put("Type", Object::new_name("Collection")?)?;
    c.dict_put("View", Object::new_name("D")?)?;
    c.dict_put("Schema", schema)?;
    Ok(c)
}

fn save_atomic(doc: &PdfDocument, out: &Path) -> Result<()> {
    let dir = out
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let tmp = tempfile::Builder::new()
        .prefix(".pliego-")
        .suffix(".tmp")
        .tempfile_in(dir)
        .map_err(|e| Error::io_at(dir, e))?;
    let mut opts = WriteOptions::default();
    opts.set_garbage_level(1).set_compress(true);
    doc.save_with_options(tmp.path(), &opts)?;
    tmp.persist(out).map_err(|e| Error::io_at(out, e.error))?;
    Ok(())
}

/// Crea una cartera con `files` (se incrustan tal cual). Nunca sobrescribe un archivo de entrada.
pub fn create(files: &[PathBuf], out: &Path, ctx: &JobContext) -> Result<Report> {
    create_in(files, out, ctx, Lang::Es)
}

/// Como [`create`], con los encabezados de columna de la cartera (Nombre, Tamaño, Modificado) en `lang`.
pub fn create_in(files: &[PathBuf], out: &Path, ctx: &JobContext, lang: Lang) -> Result<Report> {
    if files.is_empty() {
        return Err(Error::InvalidInput("indica al menos un archivo para la cartera".into()));
    }
    for f in files {
        if f.canonicalize()
            .ok()
            .is_some_and(|c| out.canonicalize().ok().is_some_and(|o| o == c))
        {
            return Err(Error::InvalidInput(
                "la cartera no puede sobrescribir uno de sus archivos".into(),
            ));
        }
    }
    let mut doc = PdfDocument::new();
    doc.new_page(Size {
        width: 595.0,
        height: 842.0,
    })?;
    let mut taken = HashSet::new();
    let total = files.len() as u64;
    for (i, f) in files.iter().enumerate() {
        ctx.step(i as u64, total, &f.display().to_string())?;
        let data = std::fs::read(f).map_err(|e| Error::io_at(f, e))?;
        let name = unique(
            &safe_name(
                &f.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            ),
            &mut taken,
        );
        let mut opts = EmbeddedFileOptions::new(&name);
        opts.mime_type = mime_of(&name);
        doc.add_embedded_file(&name, &data, opts)?;
    }
    let coll = collection_dict(&mut doc, lang)?;
    doc.catalog()?.dict_put("Collection", coll)?;
    save_atomic(&doc, out)?;
    ctx.step(total, total, "Terminado")?;
    Ok(Report::with_output(
        out,
        format!("Cartera con {} archivos.", files.len()),
    ))
}

fn open(path: &Path, password: Option<&str>) -> Result<PdfDocument> {
    let bytes = std::fs::read(path).map_err(|e| Error::io_at(path, e))?;
    let mut d = Document::from_bytes(&bytes, "application/pdf")?;
    if d.needs_password()? && !password.is_some_and(|p| d.authenticate(p).unwrap_or(false)) {
        return Err(Error::Password);
    }
    d.into_pdf()
        .map_err(|_| Error::InvalidInput(format!("{} no es un PDF", path.display())))
}

/// Archivos incrustados de un PDF.
pub fn list(path: &Path, password: Option<&str>) -> Result<Listing> {
    let doc = open(path, password)?;
    let is_portfolio = doc.catalog()?.get_dict("Collection")?.is_some();
    let entries = doc
        .embedded_files()?
        .into_iter()
        .map(|e| Entry {
            filename: e.filename.unwrap_or_else(|| e.name.clone()),
            name: e.name,
            mime_type: e.mime_type,
            size: e.size as u64,
        })
        .collect();
    Ok(Listing { is_portfolio, entries })
}

/// Extrae a `out_dir` los archivos con esos nombres (todos si `names` está vacío). Los nombres se limpian para
/// que nada salga de la carpeta, y si ya existe un archivo se añade « (2)» en vez de pisarlo.
pub fn extract(
    path: &Path,
    out_dir: &Path,
    names: &[String],
    password: Option<&str>,
    ctx: &JobContext,
) -> Result<Report> {
    let doc = open(path, password)?;
    let all = doc.embedded_files()?;
    let chosen: Vec<_> = if names.is_empty() {
        all
    } else {
        let mut v = Vec::new();
        for n in names {
            match all
                .iter()
                .find(|e| &e.name == n || e.filename.as_deref() == Some(n.as_str()))
            {
                Some(e) => v.push(e.clone()),
                None => return Err(Error::InvalidInput(format!("la cartera no contiene «{n}»"))),
            }
        }
        v
    };
    std::fs::create_dir_all(out_dir).map_err(|e| Error::io_at(out_dir, e))?;
    let mut taken: HashSet<String> = std::fs::read_dir(out_dir)
        .map_err(|e| Error::io_at(out_dir, e))?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_lowercase())
        .collect();
    let mut report = Report::default();
    let total = chosen.len() as u64;
    for (i, e) in chosen.iter().enumerate() {
        ctx.step(i as u64, total, &e.name)?;
        let Some(data) = doc.load_embedded_file(&e.name)? else {
            report.warnings.push(format!("«{}» no se pudo leer", e.name));
            continue;
        };
        let name = unique(&safe_name(e.filename.as_deref().unwrap_or(&e.name)), &mut taken);
        let dest = out_dir.join(&name);
        std::fs::write(&dest, data).map_err(|e| Error::io_at(&dest, e))?;
        report.outputs.push(dest);
    }
    report.summary = format!("{} archivos extraídos", report.outputs.len());
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_names_cannot_escape() {
        assert_eq!(safe_name("../../etc/passwd"), "passwd");
        assert_eq!(safe_name("C:\\Windows\\x.dll"), "x.dll");
        assert_eq!(safe_name(".."), "archivo");
        assert_eq!(safe_name("a:b?.txt"), "a_b_.txt");
    }

    #[test]
    fn unique_names_do_not_collide() {
        let mut t = HashSet::new();
        assert_eq!(unique("a.pdf", &mut t), "a.pdf");
        assert_eq!(unique("A.pdf", &mut t), "A (2).pdf");
        assert_eq!(unique("a.pdf", &mut t), "a (3).pdf");
    }
}
