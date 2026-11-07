//! [`Document`]: un PDF abierto para leer. Es el nombre con el que la capa siempre ha abierto documentos; ahora es
//! un envoltorio fino de [`PdfDocument`] (el modelo propio, lopdf) al que se accede por `Deref`.
//!
//! - No es `Send`: abrir y usar cada documento en un único hilo.
//! - Los índices de página son `i32` desde 0.
//! - Solo se abren PDF. Imágenes, texto, HTML, EPUB, CBZ o XPS ya no se leen como documentos (las conversiones están en
//!   `pliego-convert`: `images_to_pdf`, `cbz_to_pdf`, `epub_to_pdf`, `xps_to_pdf`…).

use std::ops::{Deref, DerefMut};
use std::path::Path;

use pliego_core::{Error, Result};

use crate::native::PdfDocument;

/// Documento PDF abierto. Todos los métodos de [`PdfDocument`] están disponibles.
pub struct Document {
    pdf: PdfDocument,
}

impl Deref for Document {
    type Target = PdfDocument;
    fn deref(&self) -> &PdfDocument {
        &self.pdf
    }
}

impl DerefMut for Document {
    fn deref_mut(&mut self) -> &mut PdfDocument {
        &mut self.pdf
    }
}

impl Document {
    /// Abre un PDF. **No** pide contraseña: usa [`Document::open_with_password`] o `authenticate`.
    pub fn open(path: impl AsRef<Path>) -> Result<Document> {
        PdfDocument::open(path).map(|pdf| Document { pdf })
    }

    /// Abre desde memoria. `magic` es el tipo MIME o la extensión (`"application/pdf"`, `"pdf"`); cualquier otro
    /// formato da [`Error::Unsupported`].
    pub fn from_bytes(bytes: &[u8], magic: &str) -> Result<Document> {
        check_magic(magic)?;
        PdfDocument::from_bytes(bytes).map(|pdf| Document { pdf })
    }

    /// Como [`Document::from_bytes`] pero toma el búfer (sin copiarlo).
    pub fn from_vec(bytes: Vec<u8>, magic: &str) -> Result<Document> {
        check_magic(magic)?;
        PdfDocument::from_vec(bytes).map(|pdf| Document { pdf })
    }

    /// Abre un PDF y, si pide contraseña, la comprueba. [`Error::Password`] si falta o es incorrecta.
    pub fn open_with_password(path: impl AsRef<Path>, password: Option<&str>) -> Result<Document> {
        PdfDocument::open_with_password(path, password).map(|pdf| Document { pdf })
    }

    /// Como [`Document::open_with_password`] desde memoria.
    pub fn from_bytes_with_password(bytes: &[u8], magic: &str, password: Option<&str>) -> Result<Document> {
        check_magic(magic)?;
        PdfDocument::from_bytes_with_password(bytes, password).map(|pdf| Document { pdf })
    }

    /// Siempre `true`: solo se abren PDF.
    pub fn is_pdf(&self) -> bool {
        true
    }

    /// El mismo documento como [`PdfDocument`].
    pub fn into_pdf(self) -> Result<PdfDocument> {
        Ok(self.pdf)
    }
}

fn check_magic(magic: &str) -> Result<()> {
    if magic.to_ascii_lowercase().contains("pdf") {
        Ok(())
    } else {
        Err(Error::Unsupported(format!(
            "solo se pueden abrir PDF (formato pedido: {magic:?}); las demás conversiones están en pliego-convert"
        )))
    }
}
