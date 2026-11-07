//! Comentar (W26), sin Tauri para probarlo en Linux; `app/src-tauri/src/commands/annot.rs` solo traduce.
//! Operaciones sobre archivos para la app: abrir con contraseña, resumen, exportar e importar XFDF.
//!
//! Son las funciones que llaman los comandos de la app (que no tienen pruebas propias): reciben rutas y nunca
//! tocan el original; las que escriben un PDF lo hacen a una ruta distinta, de forma atómica.

use std::path::Path;

use pliego_core::{Error, Result};
use pliego_pdf::{Document, PdfDocument, WriteOptions};

use crate::history::{History, Popped, Previous};
use pliego_appdata::copyjob::unique_sibling;
use pliego_core::Report;

pub use pliego_annot::{AnnotId, AnnotSpec, CommentSummary};
use pliego_annot::{export_xfdf, import_xfdf, summary};
pub use pliego_forms::{FieldInfo, FieldValue, FieldValues, KeystrokeResult};

/// Abre un PDF para anotarlo. Se lee a memoria para no dejar el archivo abierto (en Windows impediría sustituirlo).
pub fn open(path: &Path, password: Option<&str>) -> Result<PdfDocument> {
    let bytes = std::fs::read(path).map_err(|e| Error::io_at(path, e))?;
    open_vec(bytes, password).map_err(|e| match e {
        Error::InvalidInput(_) => Error::InvalidInput(format!("{} no es un PDF", path.display())),
        e => e,
    })
}

/// Abre un PDF desde memoria; [`Error::Password`] si está protegido y la contraseña falta o es incorrecta.
pub fn open_bytes(bytes: &[u8], password: Option<&str>) -> Result<PdfDocument> {
    open_vec(bytes.to_vec(), password)
}

/// Como [`open_bytes`] pero toma el búfer: con un PDF grande, la única copia en memoria es la del documento.
pub fn open_vec(bytes: Vec<u8>, password: Option<&str>) -> Result<PdfDocument> {
    let mut doc = Document::from_vec(bytes, "application/pdf")?;
    if doc.needs_password()? && !password.is_some_and(|p| doc.authenticate(p).unwrap_or(false)) {
        return Err(Error::Password);
    }
    if !doc.is_pdf() {
        return Err(Error::InvalidInput("no es un PDF".into()));
    }
    doc.into_pdf()
}

/// Opciones de guardado de la app: limpieza de objetos no usados (nivel 3) y compresión.
fn save_options() -> WriteOptions {
    let mut opts = WriteOptions::default();
    opts.set_garbage_level(3).set_compress(true);
    opts
}

/// Un temporal junto a `out` (sin manejador abierto: en Windows impide renombrar mientras exista).
fn temp_beside(out: &Path) -> Result<tempfile::TempPath> {
    let dir = out
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    Ok(tempfile::Builder::new()
        .prefix(".pliego-")
        .suffix(".tmp")
        .tempfile_in(dir)
        .map_err(|e| Error::io_at(dir, e))?
        .into_temp_path())
}

/// Guarda a un temporal junto al destino y lo renombra.
pub fn save(doc: &PdfDocument, out: &Path) -> Result<()> {
    let tmp = temp_beside(out)?;
    doc.save_with_options(&tmp, &save_options())?;
    persist_with_retry(tmp, out)
}

/// Guardado que no toca lo que ya había: incremental si se puede, completo si no.
fn save_preserving(doc: &PdfDocument, out: &Path) -> Result<()> {
    if doc.can_be_saved_incrementally() {
        let mut opts = WriteOptions::default();
        opts.set_incremental(true);
        if let Ok(bytes) = doc.to_bytes(&opts) {
            return write_bytes(out, &bytes);
        }
    }
    save(doc, out)
}

fn persist_with_retry(tmp: tempfile::TempPath, out: &Path) -> Result<()> {
    // En Windows un antivirus o el indexador pueden tener el archivo recién escrito unos milisegundos.
    let mut tmp = tmp;
    for attempt in 0..5 {
        match tmp.persist(out) {
            Ok(()) => return Ok(()),
            Err(e) if attempt < 4 => {
                tmp = e.path;
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Err(e) => return Err(Error::io_at(out, e.error)),
        }
    }
    unreachable!("el bucle devuelve en el último intento")
}

/// Campos del formulario de `src` (con su recuadro en la página) para dibujar los controles sobre el visor.
pub fn form_fields(src: &Path, password: Option<&str>) -> Result<Vec<FieldInfo>> {
    pliego_forms::fields_with_scripts(&mut open(src, password)?)
}

/// Filtra una pulsación en curso en el campo `name` con su script de tecleo. No modifica el archivo.
pub fn form_keystroke(
    src: &Path,
    password: Option<&str>,
    name: &str,
    change: &str,
    current: &str,
    sel: (usize, usize),
) -> Result<pliego_forms::KeystrokeResult> {
    pliego_forms::keystroke(&mut open(src, password)?, name, change, current, sel)
}

/// Resumen de los comentarios de `src`.
pub fn summary_of(src: &Path, password: Option<&str>) -> Result<CommentSummary> {
    summary(&mut open(src, password)?)
}

/// Exporta los comentarios de `src` a `<nombre> (<suffix>).xfdf` junto a él.
pub fn export_xfdf_copy(src: &Path, password: Option<&str>, suffix: &str) -> Result<Report> {
    let mut doc = open(src, password)?;
    let total = summary(&mut doc)?.total;
    let out = unique_sibling(src, suffix, "xfdf");
    std::fs::write(&out, export_xfdf(&mut doc)?).map_err(|e| Error::io_at(&out, e))?;
    Ok(Report::with_output(out, format!("{total} comentario(s) exportados")))
}

/// Importa el XFDF `xfdf` en una copia `<nombre> (<suffix>).pdf` de `src`; el original no cambia.
pub fn import_xfdf_copy(src: &Path, password: Option<&str>, xfdf: &Path, suffix: &str) -> Result<Report> {
    let text = std::fs::read_to_string(xfdf).map_err(|e| Error::io_at(xfdf, e))?;
    let mut doc = open(src, password)?;
    let added = import_xfdf(&mut doc, &text)?;
    let out = unique_sibling(src, suffix, "pdf");
    save(&doc, &out)?;
    Ok(Report::with_output(out, format!("{added} comentario(s) importados")))
}

/// Resultado de una edición para recargar el visor. Con un PDF grande el contenido no pasa por la memoria de la app:
/// se recarga leyendo el archivo ya guardado.
#[derive(Debug)]
pub enum Saved {
    /// Contenido nuevo en memoria (documentos pequeños).
    Bytes(Vec<u8>),
    /// El archivo ya está guardado en esta ruta.
    File(std::path::PathBuf),
}

impl Saved {
    /// El contenido nuevo en memoria (lo lee del archivo si hace falta).
    pub fn into_bytes(self) -> Result<Vec<u8>> {
        match self {
            Saved::Bytes(b) => Ok(b),
            Saved::File(p) => std::fs::read(&p).map_err(|e| Error::io_at(&p, e)),
        }
    }
}

/// Desde este tamaño el resultado de una edición se entrega como archivo y no como bytes (16 MiB, como el worker).
const SAVED_AS_FILE_MIN: u64 = 16 * 1024 * 1024;

/// Desde este tamaño los comentarios se guardan en modo ligero (se añaden al final del archivo, sin cargarlo entero ni
/// reescribirlo). `PLIEGO_LEAN_MB` lo cambia (pruebas y diagnóstico).
fn lean_min_bytes() -> u64 {
    std::env::var("PLIEGO_LEAN_MB")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map_or(64 * 1024 * 1024, |mb| mb.saturating_mul(1024 * 1024))
}

/// Edición de comentarios **sobre el propio PDF** con deshacer. Antes del primer cambio de cada archivo se guarda una
/// copia de seguridad `<nombre> (copia de seguridad).pdf`; cada cambio apila el contenido anterior con un presupuesto
/// de memoria (los pasos grandes van a temporales en disco: véase [`History`]).
#[derive(Default)]
pub struct Editor {
    undo: History,
    /// Tamaño desde el que se guarda en modo ligero (`None`: el predeterminado).
    lean_min: Option<u64>,
}

impl Editor {
    /// Editor con otros límites de historial (pruebas y diagnóstico).
    pub fn with_history_limits(limits: crate::history::Limits) -> Self {
        Self {
            undo: History::new(limits),
            lean_min: None,
        }
    }

    /// Cambia el tamaño desde el que se guarda en modo ligero (pruebas).
    pub fn with_lean_min_bytes(mut self, bytes: u64) -> Self {
        self.lean_min = Some(bytes);
        self
    }

    /// Añade una anotación al archivo y devuelve el contenido nuevo (para recargar el visor).
    ///
    /// Todas las ediciones reciben la `password` del documento (`None` si no está protegido): el archivo se abre con
    /// ella y se guarda **con la misma protección**.
    pub fn add(&self, path: &Path, password: Option<&str>, spec: &AnnotSpec, backup_suffix: &str) -> Result<Vec<u8>> {
        self.edit_pdf(path, password, backup_suffix, |doc| {
            pliego_annot::add(doc, spec).map(drop)
        })
    }

    /// Borra una anotación del archivo.
    pub fn delete(&self, path: &Path, password: Option<&str>, id: AnnotId, backup_suffix: &str) -> Result<Vec<u8>> {
        self.edit_pdf(path, password, backup_suffix, |doc| pliego_annot::delete(doc, id))
    }

    /// Como [`Editor::add`], para recargar el visor sin pasar un PDF grande por memoria (véase [`Saved`]).
    pub fn add_saved(
        &self,
        path: &Path,
        password: Option<&str>,
        spec: &AnnotSpec,
        backup_suffix: &str,
    ) -> Result<Saved> {
        self.edit_pdf_saved(path, password, backup_suffix, |doc| {
            pliego_annot::add(doc, spec).map(drop)
        })
    }

    /// Como [`Editor::delete`], con el resultado como [`Saved`].
    pub fn delete_saved(&self, path: &Path, password: Option<&str>, id: AnnotId, backup_suffix: &str) -> Result<Saved> {
        self.edit_pdf_saved(path, password, backup_suffix, |doc| pliego_annot::delete(doc, id))
    }

    /// Rellena campos de formulario en el archivo (Rellenar y firmar). Comparte la pila de deshacer con los comentarios.
    /// Devuelve el contenido nuevo.
    pub fn fill(
        &self,
        path: &Path,
        password: Option<&str>,
        values: &FieldValues,
        backup_suffix: &str,
    ) -> Result<Vec<u8>> {
        self.fill_report(path, password, values, backup_suffix)
            .map(|(bytes, _)| bytes)
    }

    /// Como [`Editor::fill`], con los avisos de los scripts del formulario (`app.alert`...).
    pub fn fill_report(
        &self,
        path: &Path,
        password: Option<&str>,
        values: &FieldValues,
        backup_suffix: &str,
    ) -> Result<(Vec<u8>, Vec<String>)> {
        let mut messages = Vec::new();
        let bytes = self.edit_pdf(path, password, backup_suffix, |doc| {
            // Con el JavaScript del formulario: validación, cálculo entre campos y formato, como un lector.
            let r = pliego_forms::fill_with_scripts(doc, values)?;
            if r.filled.is_empty() {
                let avisos = if r.messages.is_empty() {
                    String::new()
                } else {
                    format!(": {}", r.messages.join(" · "))
                };
                return Err(Error::InvalidInput(format!(
                    "ningún campo aceptó el valor ({} rechazados, {} no encontrados){avisos}",
                    r.rejected.len(),
                    r.not_found.len()
                )));
            }
            messages = r.messages;
            Ok(())
        })?;
        Ok((bytes, messages))
    }

    /// Ejecuta la acción `trigger` (`U` para un botón) del campo `name` y guarda lo que el script cambie en el
    /// documento. Devuelve el contenido nuevo y los avisos. Error si el campo no tiene esa acción.
    pub fn run_action(
        &self,
        path: &Path,
        password: Option<&str>,
        name: &str,
        trigger: &str,
        backup_suffix: &str,
    ) -> Result<(Vec<u8>, Vec<String>)> {
        let mut messages = Vec::new();
        let bytes = self.edit_pdf(path, password, backup_suffix, |doc| {
            match pliego_forms::run_action(doc, name, trigger)? {
                Some(m) => messages = m,
                None => return Err(Error::InvalidInput(format!("el campo «{name}» no tiene esa acción"))),
            }
            Ok(())
        })?;
        Ok((bytes, messages))
    }

    /// Deshace el último cambio; `None` si no hay nada que deshacer.
    pub fn undo(&self, path: &Path) -> Result<Option<Vec<u8>>> {
        let Some(prev) = self.undo.pop(path)? else {
            return Ok(None);
        };
        match prev {
            Popped::Mem(bytes) => {
                write_bytes(path, &bytes)?;
                Ok(Some(bytes))
            }
            Popped::Disk(tmp) => {
                // De archivo a archivo; solo se lee a memoria el resultado que se devuelve para recargar el visor.
                copy_over(&tmp, path)?;
                std::fs::read(path).map(Some).map_err(|e| Error::io_at(path, e))
            }
            Popped::Tail { before, after } => {
                cut_tail(path, before, after)?;
                std::fs::read(path).map(Some).map_err(|e| Error::io_at(path, e))
            }
        }
    }

    /// Como [`Editor::undo`], con el resultado como [`Saved`].
    pub fn undo_saved(&self, path: &Path) -> Result<Option<Saved>> {
        let Some(prev) = self.undo.pop(path)? else {
            return Ok(None);
        };
        match prev {
            Popped::Mem(bytes) => {
                write_bytes(path, &bytes)?;
                Ok(Some(Saved::Bytes(bytes)))
            }
            Popped::Disk(tmp) => {
                copy_over(&tmp, path)?;
                saved_of(path).map(Some)
            }
            Popped::Tail { before, after } => {
                cut_tail(path, before, after)?;
                saved_of(path).map(Some)
            }
        }
    }

    /// Cuántos cambios se pueden deshacer.
    pub fn undo_depth(&self, path: &Path) -> usize {
        self.undo.depth(path)
    }

    /// Aplica `op` a un `PdfDocument` del archivo (copia de seguridad la primera vez, deshacer y guardado atómico).
    /// Lo usan los demás modos que modifican el PDF abierto (diseño de formularios, edición de texto) para compartir
    /// la misma pila de deshacer que los comentarios.
    pub fn edit_pdf(
        &self,
        path: &Path,
        password: Option<&str>,
        backup_suffix: &str,
        op: impl FnOnce(&mut PdfDocument) -> Result<()>,
    ) -> Result<Vec<u8>> {
        self.edit_with(path, backup_suffix, |p| open(p, password), op, save)?;
        read_result(path)
    }

    /// Como [`Editor::edit_pdf`] pero conservando los bytes originales: añade los cambios al final del archivo
    /// (guardado incremental), sin recomprimir ni reordenar nada. Si el documento no lo admite (reparado,
    /// cifrado nuevo...) cae al guardado completo de siempre.
    pub fn edit_pdf_preserving(
        &self,
        path: &Path,
        password: Option<&str>,
        backup_suffix: &str,
        op: impl FnOnce(&mut PdfDocument) -> Result<()>,
    ) -> Result<Vec<u8>> {
        self.edit_with(path, backup_suffix, |p| open(p, password), op, save_preserving)?;
        read_result(path)
    }

    /// Igual que [`Editor::edit_pdf`] pero sobre un `pliego_document::Document` (lo que piden `pliego-edit`).
    pub fn edit_document(
        &self,
        path: &Path,
        password: Option<&str>,
        backup_suffix: &str,
        op: impl FnOnce(&mut pliego_document::Document) -> Result<()>,
    ) -> Result<Vec<u8>> {
        self.edit_with(
            path,
            backup_suffix,
            |p| pliego_document::Document::open(p, password),
            op,
            |d: &pliego_document::Document, out: &Path| save(d.pdf(), out),
        )?;
        read_result(path)
    }

    fn edit_with<D>(
        &self,
        path: &Path,
        backup_suffix: &str,
        open: impl FnOnce(&Path) -> Result<D>,
        op: impl FnOnce(&mut D) -> Result<()>,
        save_doc: impl FnOnce(&D, &Path) -> Result<()>,
    ) -> Result<()> {
        // El contenido anterior no se lee a memoria: el documento carga el archivo (su única copia) y la copia de
        // seguridad y el paso de deshacer se copian de archivo a archivo (o se leen si son pequeños).
        let mut doc = open(path)?;
        op(&mut doc)?;
        if !self.undo.tracks(path) {
            // La copia de seguridad se escribe antes de tocar el original.
            copy_file(path, &unique_sibling(path, backup_suffix, "pdf"))?;
        }
        let previous = self.undo.capture(path)?;
        save_doc(&doc, path)?;
        // Con un PDF grande cada copia pesa: se suelta el documento antes de leer el resultado.
        drop(doc);
        self.undo.push(path, previous);
        Ok(())
    }

    /// Como [`Editor::edit_pdf`] para cambios pequeños (comentarios): con un PDF grande abre el documento en modo ligero
    /// y guarda los cambios **al final del archivo**, sin cargar sus imágenes ni reescribirlo; el resultado se entrega
    /// como archivo ([`Saved::File`]). Si el documento no lo admite (cifrado, reparado...), usa el camino de siempre.
    pub fn edit_pdf_saved(
        &self,
        path: &Path,
        password: Option<&str>,
        backup_suffix: &str,
        mut op: impl FnMut(&mut PdfDocument) -> Result<()>,
    ) -> Result<Saved> {
        let size = std::fs::metadata(path).map_err(|e| Error::io_at(path, e))?.len();
        if size >= self.lean_min.unwrap_or_else(lean_min_bytes) {
            match self.edit_lean(path, password, backup_suffix, &mut op) {
                Err(Error::Unsupported(_)) => {}
                other => return other.map(|()| Saved::File(path.to_path_buf())),
            }
        }
        self.edit_with(path, backup_suffix, |p| open(p, password), op, save)?;
        saved_of(path)
    }

    fn edit_lean(
        &self,
        path: &Path,
        password: Option<&str>,
        backup_suffix: &str,
        op: &mut impl FnMut(&mut PdfDocument) -> Result<()>,
    ) -> Result<()> {
        let mut doc = PdfDocument::open_lean(path, password)?;
        op(&mut doc)?;
        // Primero se calcula lo que se añade, sin escribir: si no se puede (flujo grande afectado...) no se ha tocado nada.
        let append = doc.prepare_lean_append()?;
        drop(doc);
        if !self.undo.tracks(path) {
            copy_file(path, &unique_sibling(path, backup_suffix, "pdf"))?;
        }
        // El cambio solo añade al final: el contenido anterior son los primeros `before` bytes. Deshacer corta el
        // archivo ahí, sin guardar una copia de 1 GB por paso ni reescribirlo.
        let (before, after) = (append.before_len(), append.after_len());
        append.write()?;
        self.undo.push(path, Previous::Tail { before, after });
        Ok(())
    }
}

/// Contenido del archivo recién guardado.
fn read_result(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path).map_err(|e| Error::io_at(path, e))
}

/// El archivo recién guardado, como archivo si es grande y en memoria si no.
fn saved_of(path: &Path) -> Result<Saved> {
    let size = std::fs::metadata(path).map_err(|e| Error::io_at(path, e))?.len();
    if size >= SAVED_AS_FILE_MIN {
        Ok(Saved::File(path.to_path_buf()))
    } else {
        read_result(path).map(Saved::Bytes)
    }
}

/// Deshace un cambio que solo añadió bytes al final: corta `path` en `before` bytes. `after` es lo que medía justo
/// tras el cambio; si no coincide, el archivo cambió por otro lado y no se toca.
fn cut_tail(path: &Path, before: u64, after: u64) -> Result<()> {
    let f = std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .map_err(|e| Error::io_at(path, e))?;
    let len = f.metadata().map_err(|e| Error::io_at(path, e))?.len();
    if len != after {
        return Err(Error::InvalidInput(format!(
            "{} cambió desde el último comentario; no se puede deshacer",
            path.display()
        )));
    }
    f.set_len(before).map_err(|e| Error::io_at(path, e))
}

/// Sustituye `to` por una copia de `from` (temporal junto a `to` y renombrado), sin pasar el contenido por memoria.
fn copy_over(from: &Path, to: &Path) -> Result<()> {
    copy_file(from, to)
}

/// Copia `from` en `to` sin pasar el contenido por memoria (el renombrado sustituye lo que hubiera en `to`).
fn copy_file(from: &Path, to: &Path) -> Result<()> {
    let dir = to
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    // Temporal en la misma carpeta y renombrado: un corte a medias no deja una copia de seguridad truncada.
    let tmp = tempfile::Builder::new()
        .prefix(".pliego-")
        .suffix(".tmp")
        .tempfile_in(dir)
        .map_err(|e| Error::io_at(dir, e))?
        .into_temp_path();
    std::fs::copy(from, &tmp).map_err(|e| Error::io_at(from, e))?;
    persist_with_retry(tmp, to)
}

/// Escribe `bytes` en `out` de forma atómica (temporal + renombrado).
fn write_bytes(out: &Path, bytes: &[u8]) -> Result<()> {
    let dir = out
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut tmp = tempfile::Builder::new()
        .prefix(".pliego-")
        .suffix(".tmp")
        .tempfile_in(dir)
        .map_err(|e| Error::io_at(dir, e))?;
    std::io::Write::write_all(&mut tmp, bytes).map_err(|e| Error::io_at(out, e))?;
    let tmp = tmp.into_temp_path();
    persist_with_retry(tmp, out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pliego_annot::{AnnotSpec, Geometry, add};
    use pliego_pdf::Size;

    fn pdf_with_notes(path: &Path, n: usize) {
        let mut doc = PdfDocument::new();
        doc.new_page(Size::A4).unwrap();
        for i in 0..n {
            let mut s = AnnotSpec::new(0, Geometry::Note { at: [50.0, 50.0] });
            s.author = Some("Ana".into());
            s.contents = Some(format!("nota {i}"));
            add(&mut doc, &s).unwrap();
        }
        save(&doc, path).unwrap();
    }

    #[test]
    fn summary_counts_comments() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.pdf");
        pdf_with_notes(&p, 2);
        let s = summary_of(&p, None).unwrap();
        assert_eq!(s.total, 2);
        assert_eq!(s.by_author.get("Ana"), Some(&2));
    }

    #[test]
    fn xfdf_roundtrip_goes_to_a_copy_and_leaves_the_original() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.pdf");
        pdf_with_notes(&src, 2);
        let r = export_xfdf_copy(&src, None, "comentarios").unwrap();
        let xfdf = r.outputs[0].clone();
        assert_eq!(xfdf, dir.path().join("a (comentarios).xfdf"));
        assert!(std::fs::read_to_string(&xfdf).unwrap().contains("nota 0"));

        let mut blank = PdfDocument::new();
        blank.new_page(Size::A4).unwrap();
        let clean = dir.path().join("b.pdf");
        save(&blank, &clean).unwrap();
        let before = std::fs::read(&clean).unwrap();
        let r = import_xfdf_copy(&clean, None, &xfdf, "con comentarios").unwrap();
        assert_eq!(r.outputs[0], dir.path().join("b (con comentarios).pdf"));
        assert_eq!(std::fs::read(&clean).unwrap(), before);
        assert_eq!(summary_of(&r.outputs[0], None).unwrap().total, 2);
        // Una segunda importación no pisa la anterior.
        let r2 = import_xfdf_copy(&clean, None, &xfdf, "con comentarios").unwrap();
        assert_eq!(r2.outputs[0], dir.path().join("b (con comentarios 2).pdf"));
    }

    #[test]
    fn missing_file_is_an_error() {
        assert!(open(Path::new("/no/existe.pdf"), None).is_err());
    }

    #[test]
    fn editing_in_place_keeps_a_backup_and_can_be_undone() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.pdf");
        pdf_with_notes(&p, 1);
        let original = std::fs::read(&p).unwrap();
        let ed = Editor::default();
        let spec = AnnotSpec::new(
            0,
            Geometry::Highlight {
                quads: vec![[10.0, 10.0, 90.0, 10.0, 10.0, 24.0, 90.0, 24.0]],
            },
        );
        let after = ed.add(&p, None, &spec, "copia de seguridad").unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), after);
        assert_eq!(summary_of(&p, None).unwrap().total, 2);
        assert_eq!(
            std::fs::read(dir.path().join("a (copia de seguridad).pdf")).unwrap(),
            original
        );
        // Un segundo cambio no crea otra copia de seguridad.
        ed.add(&p, None, &spec, "copia de seguridad").unwrap();
        assert!(!dir.path().join("a (copia de seguridad 2).pdf").exists());
        assert_eq!(ed.undo_depth(&p), 2);
        ed.undo(&p).unwrap().unwrap();
        assert_eq!(summary_of(&p, None).unwrap().total, 2);
        ed.undo(&p).unwrap().unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), original);
        assert!(ed.undo(&p).unwrap().is_none());
    }

    #[test]
    fn a_failed_edit_leaves_the_file_and_the_undo_stack_alone() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.pdf");
        pdf_with_notes(&p, 1);
        let before = std::fs::read(&p).unwrap();
        let ed = Editor::default();
        let bad = AnnotSpec::new(99, Geometry::Note { at: [1.0, 1.0] });
        assert!(ed.add(&p, None, &bad, "copia de seguridad").is_err());
        assert_eq!(std::fs::read(&p).unwrap(), before);
        assert_eq!(ed.undo_depth(&p), 0);
        assert!(!dir.path().join("a (copia de seguridad).pdf").exists());
    }

    fn form_pdf(path: &Path) {
        // Un formulario mínimo con un campo de texto, escrito a mano.
        let src = b"%PDF-1.4\n1 0 obj<</Type/Catalog/Pages 2 0 R/AcroForm<</Fields[4 0 R]/DA(/Helv 0 Tf 0 g)/DR<</Font<</Helv 5 0 R>>>>>>>>endobj\n\
2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj\n\
3 0 obj<</Type/Page/Parent 2 0 R/MediaBox[0 0 200 200]/Annots[4 0 R]>>endobj\n\
4 0 obj<</Type/Annot/Subtype/Widget/FT/Tx/T(nombre)/Rect[20 150 180 170]/F 4/P 3 0 R/DA(/Helv 12 Tf 0 g)>>endobj\n\
5 0 obj<</Type/Font/Subtype/Type1/BaseFont/Helvetica>>endobj\n\
trailer<</Root 1 0 R/Size 6>>\n%%EOF\n";
        std::fs::write(path, src).unwrap();
    }

    #[test]
    fn fills_a_form_field_with_backup_and_undo() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("f.pdf");
        form_pdf(&p);
        let fs = form_fields(&p, None).unwrap();
        assert_eq!(fs.len(), 1);
        let r = fs[0].rect.unwrap();
        assert!(r[3] > r[1] && r[2] > r[0]);
        let ed = Editor::default();
        let mut v = FieldValues::new();
        v.insert("nombre".into(), FieldValue::Text("Ñandú".into()));
        let (_, avisos) = ed.fill_report(&p, None, &v, "copia de seguridad").unwrap();
        assert!(avisos.is_empty());
        assert_eq!(form_fields(&p, None).unwrap()[0].value.as_deref(), Some("Ñandú"));
        assert!(dir.path().join("f (copia de seguridad).pdf").exists());
        // Un nombre inexistente no cuenta como cambio.
        let mut bad = FieldValues::new();
        bad.insert("nadie".into(), FieldValue::Text("x".into()));
        assert!(ed.fill(&p, None, &bad, "copia de seguridad").is_err());
        assert_eq!(ed.undo_depth(&p), 1);
        ed.undo(&p).unwrap().unwrap();
        assert_eq!(form_fields(&p, None).unwrap()[0].value.as_deref().unwrap_or(""), "");
    }

    #[test]
    fn lean_editing_falls_back_to_the_normal_path_when_the_document_does_not_allow_it() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("cifrado.pdf");
        let mut doc = PdfDocument::new();
        doc.new_page(Size::A4).unwrap();
        let mut o = WriteOptions::new();
        o.set_encryption(pliego_pdf::Encryption::Aes256)
            .set_user_password("abrir")
            .set_owner_password("duenio");
        std::fs::write(&p, doc.to_bytes(&o).unwrap()).unwrap();
        let ed = Editor::default().with_lean_min_bytes(0);
        let mut s = AnnotSpec::new(0, Geometry::Note { at: [50.0, 50.0] });
        s.contents = Some("nota".into());
        let saved = ed.add_saved(&p, Some("abrir"), &s, "copia de seguridad").unwrap();
        // Cifrado: no admite el modo ligero; se guardó entero y sigue protegido.
        assert!(matches!(saved, Saved::Bytes(_)), "{saved:?}");
        assert_eq!(summary_of(&p, Some("abrir")).unwrap().total, 1);
        assert!(summary_of(&p, None).is_err());
        ed.undo_saved(&p).unwrap().unwrap();
        assert_eq!(summary_of(&p, Some("abrir")).unwrap().total, 0);
    }
}
