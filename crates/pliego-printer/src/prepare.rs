//! Prepara el PDF para imprimir: comprueba los permisos y aplana los campos de formulario para que salgan con su valor.

use std::path::Path;

use pliego_core::{Error, Result};
use pliego_pdf::{Encryption, PdfDocument, Permission, WriteOptions};

/// El PDF listo para PDFium: sus bytes y la contraseña con la que debe abrirlos (si siguen cifrados).
pub struct Prepared {
    pub bytes: Vec<u8>,
    pub password: Option<String>,
}

/// Lee `path` y lo deja listo para imprimir.
///
/// Los campos de formulario se aplanan en una copia en memoria (PDFium no los dibuja al imprimir si no hay un
/// entorno de formularios); el archivo no se toca. Un PDF cifrado que prohíbe imprimir se rechaza.
pub fn prepare(path: &Path, password: Option<&str>) -> Result<Prepared> {
    let password = password.filter(|p| !p.is_empty());
    let mut doc = PdfDocument::open_with_password(path, password)?;
    if doc.needs_password()? {
        return Err(Error::Password);
    }
    if !doc.permissions().contains(Permission::PRINT) {
        return Err(Error::Unsupported(
            "el autor del documento no permite imprimirlo (permisos del PDF)".into(),
        ));
    }
    if doc.has_acro_form()? {
        doc.bake(false, true)?;
        let mut options = WriteOptions::default();
        options.set_encryption(Encryption::None);
        let bytes = doc.to_bytes(&options)?;
        return Ok(Prepared { bytes, password: None });
    }
    let bytes = std::fs::read(path).map_err(|e| Error::io_at(path, e))?;
    Ok(Prepared {
        bytes,
        password: password.map(str::to_owned),
    })
}
