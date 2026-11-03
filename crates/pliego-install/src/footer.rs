//! Pie de 64 bytes para pegar un `.paq` al final de un ejecutable (el instalador):
//! `PLIEGOPAQ1` (10) · desplazamiento u64 LE (8) · longitud u64 LE (8) · SHA-256 del `.paq` (32) · relleno (6).
//! El `.paq` ocupa `[offset, offset + len)`; el pie son los últimos 64 bytes del archivo.

use crate::error::{Error, Result};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;

pub const MAGIC: &[u8; 10] = b"PLIEGOPAQ1";
pub const FOOTER_LEN: u64 = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Footer {
    pub offset: u64,
    pub len: u64,
    pub sha256: [u8; 32],
}

impl Footer {
    pub fn to_bytes(&self) -> [u8; 64] {
        let mut b = [0u8; 64];
        b[..10].copy_from_slice(MAGIC);
        b[10..18].copy_from_slice(&self.offset.to_le_bytes());
        b[18..26].copy_from_slice(&self.len.to_le_bytes());
        b[26..58].copy_from_slice(&self.sha256);
        b
    }

    pub fn parse(b: &[u8; 64]) -> Option<Footer> {
        if &b[..10] != MAGIC {
            return None;
        }
        Some(Footer {
            offset: u64::from_le_bytes(b[10..18].try_into().ok()?),
            len: u64::from_le_bytes(b[18..26].try_into().ok()?),
            sha256: b[26..58].try_into().ok()?,
        })
    }
}

fn io_err(context: &str, e: io::Error) -> Error {
    Error::io(context.to_string(), e)
}

/// Crea `out` = `base` + `.paq` + pie.
pub fn append_paq(base: &Path, paq: &Path, out: &Path) -> Result<Footer> {
    let mut o = File::create(out).map_err(|e| io_err("no se pudo crear el instalador", e))?;
    let offset = io::copy(
        &mut File::open(base).map_err(|e| io_err("no se pudo abrir el binario base", e))?,
        &mut o,
    )
    .map_err(|e| io_err("no se pudo copiar el binario base", e))?;
    let mut src = File::open(paq).map_err(|e| io_err("no se pudo abrir el paquete", e))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    let mut len = 0u64;
    loop {
        let k = src
            .read(&mut buf)
            .map_err(|e| io_err("no se pudo leer el paquete", e))?;
        if k == 0 {
            break;
        }
        hasher.update(&buf[..k]);
        o.write_all(&buf[..k])
            .map_err(|e| io_err("no se pudo escribir el instalador", e))?;
        len += k as u64;
    }
    let footer = Footer {
        offset,
        len,
        sha256: hasher.finalize().into(),
    };
    o.write_all(&footer.to_bytes())
        .map_err(|e| io_err("no se pudo escribir el pie", e))?;
    Ok(footer)
}

/// Lee el pie de un ejecutable; `None` si no lleva paquete.
pub fn read_footer(exe: &Path) -> Result<Option<Footer>> {
    let mut f = File::open(exe).map_err(|e| io_err("no se pudo abrir el ejecutable", e))?;
    let total = f
        .metadata()
        .map_err(|e| io_err("no se pudo leer el ejecutable", e))?
        .len();
    if total < FOOTER_LEN {
        return Ok(None);
    }
    f.seek(SeekFrom::Start(total - FOOTER_LEN))
        .map_err(|e| io_err("no se pudo leer el pie", e))?;
    let mut b = [0u8; 64];
    f.read_exact(&mut b).map_err(|e| io_err("no se pudo leer el pie", e))?;
    let Some(footer) = Footer::parse(&b) else {
        return Ok(None);
    };
    match footer.offset.checked_add(footer.len) {
        Some(end) if end == total - FOOTER_LEN => Ok(Some(footer)),
        _ => Err(Error::Corrupt("el pie del instalador no es coherente".into())),
    }
}

/// Abre el `.paq` embebido, comprobando antes su SHA-256 completo. Devuelve un lector acotado al paquete.
pub fn open_embedded(exe: &Path) -> Result<Option<io::Take<File>>> {
    let Some(footer) = read_footer(exe)? else {
        return Ok(None);
    };
    let mut f = File::open(exe).map_err(|e| io_err("no se pudo abrir el ejecutable", e))?;
    f.seek(SeekFrom::Start(footer.offset))
        .map_err(|e| io_err("no se pudo leer el paquete", e))?;
    let mut hasher = Sha256::new();
    let mut r = (&mut f).take(footer.len);
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let k = r.read(&mut buf).map_err(|e| io_err("no se pudo leer el paquete", e))?;
        if k == 0 {
            break;
        }
        hasher.update(&buf[..k]);
    }
    let digest: [u8; 32] = hasher.finalize().into();
    if digest != footer.sha256 {
        return Err(Error::Corrupt(
            "la suma de comprobación del instalador no coincide".into(),
        ));
    }
    f.seek(SeekFrom::Start(footer.offset))
        .map_err(|e| io_err("no se pudo leer el paquete", e))?;
    Ok(Some(f.take(footer.len)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paq;
    use std::fs;

    #[test]
    fn footer_roundtrip_and_embedded_paq() {
        let d = tempfile::tempdir().unwrap();
        let src = d.path().join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("pliego-app.exe"), "x").unwrap();
        let paq_path = d.path().join("a.paq");
        paq::pack(&src, "0.4.0", &paq_path).unwrap();
        let base = d.path().join("setup-base.exe");
        fs::write(&base, vec![0x4du8; 5000]).unwrap();
        let exe = d.path().join("Setup.exe");
        let f = append_paq(&base, &paq_path, &exe).unwrap();
        assert_eq!(f.offset, 5000);
        assert_eq!(read_footer(&exe).unwrap(), Some(f));
        let r = open_embedded(&exe).unwrap().unwrap();
        let m = paq::unpack_verified(r, &d.path().join("app-0.4.0")).unwrap();
        assert_eq!(m.version, "0.4.0");
    }

    #[test]
    fn plain_exe_has_no_footer() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("a.exe");
        fs::write(&p, vec![1u8; 200]).unwrap();
        assert_eq!(read_footer(&p).unwrap(), None);
        assert!(open_embedded(&p).unwrap().is_none());
    }

    #[test]
    fn detects_corruption_and_bad_footer() {
        let d = tempfile::tempdir().unwrap();
        let base = d.path().join("b.exe");
        let paq = d.path().join("p.paq");
        fs::write(&base, b"MZ....").unwrap();
        fs::write(&paq, vec![9u8; 1000]).unwrap();
        let exe = d.path().join("s.exe");
        append_paq(&base, &paq, &exe).unwrap();
        let mut bytes = fs::read(&exe).unwrap();
        bytes[100] ^= 1;
        fs::write(&exe, &bytes).unwrap();
        assert!(matches!(open_embedded(&exe), Err(Error::Corrupt(_))));
        // Pie con desplazamiento imposible.
        let n = bytes.len();
        bytes[n - 54] ^= 0xff;
        fs::write(&exe, &bytes).unwrap();
        assert!(matches!(read_footer(&exe), Err(Error::Corrupt(_))));
    }
}
