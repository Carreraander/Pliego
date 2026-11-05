//! Documento PDF editable de Pliego (AGPL-3.0-or-later).
//!
//! [`Document`] envuelve un `pliego_pdf::PdfDocument` y añade:
//! - número de **revisión** (sube con cada operación que modifica el documento),
//! - **deshacer/rehacer** con instantáneas en memoria (acotadas por número y por bytes),
//! - **guardado atómico** (temporal en la misma carpeta + renombrar),
//! - **guardado incremental** para documentos firmados, que conserva intactos los bytes firmados,
//! - **reparación** de PDFs dañados.
//!
//! `Document` no es `Send` (el motor usa contextos por hilo): úsalo siempre desde el mismo hilo.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use pliego_core::{Error, Result};
use pliego_pdf::{PdfDocument, WriteOptions};
use serde::{Deserialize, Serialize};

/// Opciones de guardado.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SaveOptions {
    /// Comprimir flujos.
    pub compress: bool,
    /// Eliminar objetos no usados (nivel 1) al guardar.
    pub garbage: bool,
    /// Permitir guardar de forma completa un documento firmado, lo que invalida sus firmas.
    /// Si es `false` y el documento está firmado, se guarda de forma incremental.
    pub allow_invalidate_signatures: bool,
}

impl Default for SaveOptions {
    fn default() -> Self {
        Self {
            compress: true,
            garbage: false,
            allow_invalidate_signatures: false,
        }
    }
}

/// Límites del historial de deshacer.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default)]
pub struct UndoLimits {
    /// Máximo de pasos de deshacer.
    pub max_steps: usize,
    /// Máximo de bytes que pueden ocupar las instantáneas (se descartan las más antiguas).
    pub max_bytes: usize,
}

impl Default for UndoLimits {
    fn default() -> Self {
        Self {
            max_steps: 50,
            max_bytes: 256 * 1024 * 1024,
        }
    }
}

/// Estado completo del documento en una revisión.
struct Snapshot {
    revision: u64,
    bytes: Vec<u8>,
}

pub struct Document {
    pdf: PdfDocument,
    password: Option<String>,
    /// Ruta de origen, si se abrió desde disco.
    path: Option<PathBuf>,
    /// Bytes tal como estaban en disco al abrir (base del guardado incremental).
    original_len: Option<usize>,
    /// `true` mientras `pdf` descienda directamente del archivo original (sin recargas por deshacer).
    pdf_matches_original: bool,
    revision: u64,
    next_revision: u64,
    /// Estado actual serializado (necesario para poder deshacer la siguiente operación).
    current: Snapshot,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    limits: UndoLimits,
    /// Revisión guardada por última vez (para `is_modified`).
    saved_revision: u64,
}

/// Abre un PDF por ruta.
fn open_pdf(path: &Path) -> Result<PdfDocument> {
    PdfDocument::open(path)
}

// TODO(W02): cada operación serializa el documento completo sin comprimir; con PDFs de 100 MB o más
// la memoria y la latencia crecen. Pasar a instantáneas incrementales o limitar el historial por tamaño.
fn serialize(pdf: &PdfDocument) -> Result<Vec<u8>> {
    let mut opts = WriteOptions::default();
    // Sin compresión extra ni limpieza: la instantánea debe ser fiel y rápida.
    opts.set_compress(false).set_garbage(false);
    let mut out = Vec::new();
    pdf.write_to_with_options(&mut out, &opts)?;
    Ok(out)
}

fn load(bytes: &[u8], password: Option<&str>) -> Result<PdfDocument> {
    let mut pdf = PdfDocument::from_bytes(bytes)?;
    unlock(&mut pdf, password)?;
    Ok(pdf)
}

fn unlock(pdf: &mut PdfDocument, password: Option<&str>) -> Result<()> {
    if pdf.needs_password()? {
        let ok = match password {
            Some(p) => pdf.authenticate(p)?,
            None => false,
        };
        if !ok {
            return Err(Error::Password);
        }
    }
    Ok(())
}

impl Document {
    /// Abre un PDF desde disco.
    pub fn open(path: impl AsRef<Path>, password: Option<&str>) -> Result<Self> {
        let path = path.as_ref();
        let bytes = fs::read(path).map_err(|e| Error::io_at(path, e))?;
        let mut pdf = open_pdf(path)?;
        unlock(&mut pdf, password)?;
        Ok(Self::build(pdf, password, Some(path.to_path_buf()), bytes))
    }

    /// Abre un PDF desde memoria.
    pub fn open_bytes(bytes: &[u8], password: Option<&str>) -> Result<Self> {
        let pdf = load(bytes, password)?;
        Ok(Self::build(pdf, password, None, bytes.to_vec()))
    }

    fn build(pdf: PdfDocument, password: Option<&str>, path: Option<PathBuf>, bytes: Vec<u8>) -> Self {
        Self {
            pdf,
            password: password.map(str::to_owned),
            original_len: path.as_ref().map(|_| bytes.len()),
            path,
            pdf_matches_original: true,
            revision: 0,
            next_revision: 1,
            current: Snapshot { revision: 0, bytes },
            undo: Vec::new(),
            redo: Vec::new(),
            limits: UndoLimits::default(),
            saved_revision: 0,
        }
    }

    /// Cambia los límites del historial de deshacer.
    pub fn set_undo_limits(&mut self, limits: UndoLimits) {
        self.limits = limits;
        self.trim_undo();
    }

    /// Número de revisión actual. Empieza en 0 y cambia con cada operación, deshacer o rehacer.
    /// Sirve de clave para invalidar teselas cacheadas.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Acceso de solo lectura al documento MuPDF.
    pub fn pdf(&self) -> &PdfDocument {
        &self.pdf
    }

    /// Ruta de origen, si se abrió desde disco.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// `true` si hay cambios sin guardar.
    pub fn is_modified(&self) -> bool {
        self.revision != self.saved_revision
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// `true` si el documento contiene firmas digitales (`/SigFlags` con el bit SignaturesExist).
    pub fn is_signed(&self) -> bool {
        let sig_flags = || -> Option<i32> {
            let root = self.pdf.trailer().ok()?.get_dict("Root").ok()??;
            let form = root.get_dict("AcroForm").ok()??;
            form.get_dict("SigFlags").ok()??.as_int().ok()
        };
        sig_flags().is_some_and(|f| f & 1 != 0)
    }

    /// Aplica una operación de modificación. Si `f` devuelve `Ok`, sube la revisión y se puede deshacer;
    /// si devuelve `Err`, el documento vuelve al estado anterior.
    pub fn with_pdf_mut<T>(&mut self, f: impl FnOnce(&mut PdfDocument) -> Result<T>) -> Result<T> {
        match f(&mut self.pdf) {
            Ok(value) => {
                let bytes = serialize(&self.pdf)?;
                let rev = self.next_revision;
                self.next_revision += 1;
                let previous = std::mem::replace(&mut self.current, Snapshot { revision: rev, bytes });
                self.undo.push(previous);
                self.redo.clear();
                self.revision = rev;
                self.trim_undo();
                Ok(value)
            }
            Err(e) => {
                // La operación pudo dejar el documento a medias: se restaura el estado anterior.
                self.restore_current()?;
                Err(e)
            }
        }
    }

    fn trim_undo(&mut self) {
        let mut total: usize = self.undo.iter().map(|s| s.bytes.len()).sum::<usize>() + self.current.bytes.len();
        while !self.undo.is_empty() && (self.undo.len() > self.limits.max_steps || total > self.limits.max_bytes) {
            total -= self.undo.remove(0).bytes.len();
        }
    }

    fn restore_current(&mut self) -> Result<()> {
        // En la revisión 0 se vuelve a abrir el archivo original: así sigue disponible el guardado incremental.
        let from_disk = match (&self.path, self.current.revision) {
            (Some(p), 0) => open_pdf(p)
                .ok()
                .and_then(|mut d| unlock(&mut d, self.password.as_deref()).ok().map(|_| d)),
            _ => None,
        };
        self.pdf_matches_original = from_disk.is_some();
        self.pdf = match from_disk {
            Some(d) => d,
            None => load(&self.current.bytes, self.password.as_deref())?,
        };
        Ok(())
    }

    /// Deshace la última operación. Devuelve `false` si no hay nada que deshacer.
    pub fn undo(&mut self) -> Result<bool> {
        let Some(prev) = self.undo.pop() else {
            return Ok(false);
        };
        let cur = std::mem::replace(&mut self.current, prev);
        self.redo.push(cur);
        self.revision = self.next_revision;
        self.next_revision += 1;
        self.current.revision = self.revision;
        self.restore_after_history()?;
        Ok(true)
    }

    /// Rehace la última operación deshecha. Devuelve `false` si no hay nada que rehacer.
    pub fn redo(&mut self) -> Result<bool> {
        let Some(next) = self.redo.pop() else {
            return Ok(false);
        };
        let cur = std::mem::replace(&mut self.current, next);
        self.undo.push(cur);
        self.revision = self.next_revision;
        self.next_revision += 1;
        self.current.revision = self.revision;
        self.restore_after_history()?;
        Ok(true)
    }

    fn restore_after_history(&mut self) -> Result<()> {
        self.pdf = load(&self.current.bytes, self.password.as_deref())?;
        // Tras recargar desde memoria el documento ya no coincide con el archivo en disco.
        self.pdf_matches_original = false;
        Ok(())
    }

    fn write_options(opts: &SaveOptions, incremental: bool) -> WriteOptions {
        let mut w = WriteOptions::default();
        if incremental {
            w.set_incremental(true);
        } else {
            w.set_compress(opts.compress).set_garbage(opts.garbage);
        }
        w
    }

    /// Guarda en `path` de forma atómica (temporal en la misma carpeta y renombrar).
    ///
    /// Si el documento está firmado se guarda de forma incremental, de modo que las firmas siguen
    /// siendo válidas; esto exige que se haya abierto desde `path` y que no se haya deshecho/rehecho
    /// desde entonces. Con `allow_invalidate_signatures` se permite un guardado completo.
    pub fn save(&mut self, path: impl AsRef<Path>, options: &SaveOptions) -> Result<()> {
        let path = path.as_ref();
        if self.is_signed() && !options.allow_invalidate_signatures {
            return self.save_incremental_to(path);
        }
        let tmp = temp_sibling(path)?;
        let res = (|| {
            let opts = Self::write_options(options, false);
            let mut buf = Vec::new();
            self.pdf.write_to_with_options(&mut buf, &opts)?;
            let mut f = fs::File::create(&tmp).map_err(|e| Error::io_at(&tmp, e))?;
            f.write_all(&buf).map_err(|e| Error::io_at(&tmp, e))?;
            f.sync_all().map_err(|e| Error::io_at(&tmp, e))?;
            drop(f);
            self.release_file_handle()?;
            fs::rename(&tmp, path).map_err(|e| Error::io_at(path, e))
        })();
        if res.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        res?;
        self.after_save(path);
        Ok(())
    }

    /// Guarda en el archivo de origen añadiendo solo los cambios al final (conserva las firmas).
    pub fn save_incremental(&mut self) -> Result<()> {
        let Some(path) = self.path.clone() else {
            return Err(Error::Unsupported(
                "el guardado incremental necesita un documento abierto desde un archivo".into(),
            ));
        };
        self.save_incremental_to(&path)
    }

    fn save_incremental_to(&mut self, dest: &Path) -> Result<()> {
        let unsupported = |why: &str| {
            Error::Unsupported(format!(
                "no se puede guardar este documento firmado sin invalidar sus firmas: {why}. \
                 Guarda una copia sin firmas o conserva el archivo original"
            ))
        };
        let Some(origin) = self.path.clone() else {
            return Err(unsupported("el documento no se abrió desde un archivo"));
        };
        if !self.pdf_matches_original {
            return Err(unsupported("se deshizo o rehízo después de abrir"));
        }
        if !self.pdf.can_be_saved_incrementally() {
            return Err(unsupported("el formato no admite guardado incremental"));
        }
        // MuPDF añade los cambios sobre el archivo original, así que se trabaja sobre una copia
        // temporal y se renombra al final: el original nunca queda a medias.
        let on_disk = fs::metadata(&origin).map_err(|e| Error::io_at(&origin, e))?.len() as usize;
        if Some(on_disk) != self.original_len {
            return Err(unsupported("el archivo cambió en disco desde que se abrió"));
        }
        let tmp = temp_sibling(dest)?;
        let res = (|| {
            fs::copy(&origin, &tmp).map_err(|e| Error::io_at(&tmp, e))?;
            self.pdf
                .save_with_options(&tmp, &Self::write_options(&SaveOptions::default(), true))?;
            // Abrir con permiso de escritura: en Windows `sync_all` falla sobre un handle de solo lectura.
            fs::OpenOptions::new()
                .write(true)
                .open(&tmp)
                .and_then(|f| f.sync_all())
                .map_err(|e| Error::io_at(&tmp, e))?;
            self.release_file_handle()?;
            fs::rename(&tmp, dest).map_err(|e| Error::io_at(dest, e))
        })();
        if res.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        res?;
        self.after_save(dest);
        Ok(())
    }

    /// Windows no deja reemplazar un archivo que MuPDF mantiene abierto: se pasa el documento a memoria
    /// (misma revisión) antes de renombrar. `after_save` vuelve a abrir el archivo guardado.
    fn release_file_handle(&mut self) -> Result<()> {
        self.pdf = load(&self.current.bytes, self.password.as_deref())?;
        self.pdf_matches_original = false;
        Ok(())
    }

    fn after_save(&mut self, path: &Path) {
        self.saved_revision = self.revision;
        // El archivo guardado pasa a ser la nueva base. Se recarga para que el guardado incremental
        // posterior parta de él; si falla, simplemente deja de estar disponible.
        let Ok(bytes) = fs::read(path) else { return };
        let Ok(mut doc) = open_pdf(path) else { return };
        if unlock(&mut doc, self.password.as_deref()).is_err() {
            return;
        }
        self.pdf = doc;
        self.path = Some(path.to_path_buf());
        self.original_len = Some(bytes.len());
        self.pdf_matches_original = true;
        if let Ok(snapshot) = serialize(&self.pdf) {
            self.current.bytes = snapshot;
        }
    }
}

/// Repara un PDF dañado: MuPDF reconstruye la tabla de referencias al abrirlo y aquí se reescribe
/// el resultado completo en `output` (de forma atómica). No modifica `input`.
pub fn repair(input: impl AsRef<Path>, output: impl AsRef<Path>, password: Option<&str>) -> Result<()> {
    let (input, output) = (input.as_ref(), output.as_ref());
    if input == output {
        return Err(Error::InvalidInput(
            "el archivo reparado debe ser distinto del original".into(),
        ));
    }
    let mut pdf = open_pdf(input)?;
    unlock(&mut pdf, password)?;
    // Forzar la lectura del árbol de páginas detecta daños que la apertura perezosa no ve.
    let pages = pdf.page_count()?;
    if pages <= 0 {
        return Err(Error::Pdf("no se pudo recuperar ninguna página del documento".into()));
    }
    let mut opts = WriteOptions::default();
    opts.set_garbage(true).set_compress(true);
    let mut buf = Vec::new();
    pdf.write_to_with_options(&mut buf, &opts)?;
    let tmp = temp_sibling(output)?;
    let res = fs::write(&tmp, &buf)
        .map_err(|e| Error::io_at(&tmp, e))
        .and_then(|_| fs::rename(&tmp, output).map_err(|e| Error::io_at(output, e)));
    if res.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    res
}

fn temp_sibling(path: &Path) -> Result<PathBuf> {
    let name = path
        .file_name()
        .ok_or_else(|| Error::InvalidInput("ruta de destino sin nombre de archivo".into()))?;
    let mut tmp_name = std::ffi::OsString::from(".");
    tmp_name.push(name);
    tmp_name.push(format!(".pliego-{}.tmp", std::process::id()));
    Ok(path.with_file_name(tmp_name))
}

#[cfg(test)]
mod tests;
