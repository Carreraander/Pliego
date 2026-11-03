//! Utilidades internas: apertura de PDF, escritura atómica y escape de XML.

use std::io::Write;
use std::path::{Path, PathBuf};

use mupdf::Document;
use pliego_core::{Error, Result};

pub(crate) fn pdf_err(e: mupdf::Error) -> Error {
    Error::Pdf(e.to_string())
}

/// Abre un PDF y lo desbloquea si hace falta.
pub(crate) fn open_document(path: &Path, password: Option<&str>) -> Result<Document> {
    if !path.exists() {
        return Err(Error::io_at(path, std::io::Error::from(std::io::ErrorKind::NotFound)));
    }
    let s = path
        .to_str()
        .ok_or_else(|| Error::InvalidInput(format!("ruta con caracteres no válidos: {}", path.display())))?;
    let mut doc = Document::open(s).map_err(pdf_err)?;
    if doc.needs_password().map_err(pdf_err)? {
        let ok = match password {
            Some(p) => doc.authenticate(p).map_err(pdf_err)?,
            None => false,
        };
        if !ok {
            return Err(Error::Password);
        }
    }
    Ok(doc)
}

/// Escribe en un temporal de la misma carpeta y renombra: nunca deja un archivo a medias.
pub(crate) fn write_atomic(dest: &Path, bytes: &[u8]) -> Result<()> {
    let dir = match dest.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    };
    std::fs::create_dir_all(&dir).map_err(|e| Error::io_at(&dir, e))?;
    let mut tmp = tempfile::NamedTempFile::new_in(&dir).map_err(|e| Error::io_at(&dir, e))?;
    tmp.write_all(bytes).map_err(|e| Error::io_at(dest, e))?;
    tmp.persist(dest).map_err(|e| Error::io_at(dest, e.error))?;
    Ok(())
}

pub(crate) fn ensure_not_same(input: &Path, output: &Path) -> Result<()> {
    if let (Ok(a), Ok(b)) = (input.canonicalize(), output.canonicalize())
        && a == b
    {
        return Err(Error::InvalidInput(
            "el archivo de salida no puede ser el mismo que el de entrada".into(),
        ));
    }
    Ok(())
}

/// Escapa texto para XML y descarta caracteres no permitidos en XML 1.0.
pub(crate) fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\t' | '\n' | '\r' => out.push(c),
            c if (c as u32) < 0x20 || c == '\u{fffe}' || c == '\u{ffff}' || (0xd800..=0xdfff).contains(&(c as u32)) => {
            }
            c => out.push(c),
        }
    }
    out
}

/// Conversión de puntos a twips (1/20 de punto).
pub(crate) fn twips(pt: f32) -> i32 {
    (pt * 20.0).round() as i32
}

/// Conversión de puntos a EMU.
pub(crate) fn emu(pt: f32) -> i64 {
    (pt as f64 * 12700.0).round() as i64
}

/// Contenedor ZIP en memoria para los formatos OOXML.
pub(crate) struct Package {
    zip: zip::ZipWriter<std::io::Cursor<Vec<u8>>>,
}

impl Package {
    pub fn new() -> Self {
        Self {
            zip: zip::ZipWriter::new(std::io::Cursor::new(Vec::new())),
        }
    }

    pub fn add(&mut self, name: &str, data: &[u8]) -> Result<()> {
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        self.zip
            .start_file(name, opts)
            .and_then(|()| self.zip.write_all(data).map_err(zip::result::ZipError::Io))
            .map_err(|e| Error::Other(format!("no se pudo escribir el contenedor OOXML: {e}")))
    }

    pub fn finish(self) -> Result<Vec<u8>> {
        self.zip
            .finish()
            .map(|c| c.into_inner())
            .map_err(|e| Error::Other(format!("no se pudo cerrar el contenedor OOXML: {e}")))
    }
}
