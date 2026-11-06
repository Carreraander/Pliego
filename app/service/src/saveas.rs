//! «Guardar como»: copia el archivo del documento abierto a otra ruta. Las herramientas de Pliego guardan solas en el
//! archivo (encima, con copia previa, o en una copia al lado), así que el archivo en disco ya es el documento entero:
//! basta con copiar sus bytes. La copia conserva la protección (contraseña y permisos) del original.

use std::io::Write;
use std::path::{Path, PathBuf};

use pliego_core::{Error, Result};

/// Copia `src` en `dest` de forma atómica (temporal en la carpeta de destino y renombrado). Si `dest` no termina en
/// `.pdf` se le añade. Devuelve la ruta final. No se puede guardar encima del propio archivo.
pub fn save_copy_as(src: &Path, dest: &Path) -> Result<PathBuf> {
    let dest = with_pdf_extension(dest);
    if same_file(src, &dest) {
        return Err(Error::InvalidInput("el destino es el propio documento".into()));
    }
    let bytes = std::fs::read(src).map_err(|e| Error::io_at(src, e))?;
    let dir = dest
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut tmp = tempfile::Builder::new()
        .prefix(".pliego-guardar-")
        .suffix(".tmp")
        .tempfile_in(dir)
        .map_err(|e| Error::io_at(dir, e))?;
    tmp.write_all(&bytes)
        .and_then(|()| tmp.as_file().sync_all())
        .map_err(|e| Error::io_at(&dest, e))?;
    tmp.persist(&dest).map_err(|e| Error::io_at(&dest, e.error))?;
    Ok(dest)
}

fn with_pdf_extension(p: &Path) -> PathBuf {
    let is_pdf = p
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"));
    if is_pdf {
        p.to_path_buf()
    } else {
        let mut s = p.as_os_str().to_owned();
        s.push(".pdf");
        PathBuf::from(s)
    }
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_bytes_and_adds_the_extension() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("informe.pdf");
        std::fs::write(&src, b"%PDF-1.7 contenido").unwrap();
        let out = save_copy_as(&src, &dir.path().join("sub-informe")).unwrap();
        assert_eq!(out, dir.path().join("sub-informe.pdf"));
        assert_eq!(std::fs::read(&out).unwrap(), b"%PDF-1.7 contenido");
        assert_eq!(
            std::fs::read(&src).unwrap(),
            b"%PDF-1.7 contenido",
            "el original no cambia"
        );
        // Sin temporales sueltos en la carpeta.
        let names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names.len(), 2, "{names:?}");
    }

    #[test]
    fn replaces_an_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.pdf");
        let dest = dir.path().join("b.PDF");
        std::fs::write(&src, b"nuevo").unwrap();
        std::fs::write(&dest, b"viejo").unwrap();
        assert_eq!(save_copy_as(&src, &dest).unwrap(), dest);
        assert_eq!(std::fs::read(&dest).unwrap(), b"nuevo");
    }

    #[test]
    fn refuses_to_overwrite_itself() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.pdf");
        std::fs::write(&src, b"x").unwrap();
        let err = save_copy_as(&src, &dir.path().join(".").join("a.pdf")).unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)));
    }

    #[test]
    fn missing_source_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        assert!(save_copy_as(&dir.path().join("no.pdf"), &dir.path().join("b.pdf")).is_err());
    }
}
