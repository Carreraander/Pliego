//! Lectura segura de paquetes ZIP: nombres normalizados, búsqueda sin distinguir mayúsculas y límites de tamaño.

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use pliego_core::{Error, Result};
use zip::ZipArchive;

/// Entradas máximas que se aceptan en un paquete (defensa contra ZIP malformados).
const MAX_ENTRIES: usize = 50_000;

/// Un paquete ZIP abierto de solo lectura.
pub(crate) struct Package {
    zip: ZipArchive<File>,
    /// Nombre normalizado en minúsculas → posición en el archivo.
    index: HashMap<String, usize>,
    /// Nombres normalizados (con sus mayúsculas) en el orden del archivo; solo los ficheros.
    names: Vec<String>,
    path_label: String,
}

impl Package {
    pub(crate) fn open(path: &Path) -> Result<Package> {
        let file = File::open(path).map_err(|e| Error::io_at(path, e))?;
        let zip = ZipArchive::new(file).map_err(|_| {
            Error::InvalidInput(format!(
                "{}: no es un paquete ZIP válido (¿está dañado?)",
                path.display()
            ))
        })?;
        if zip.len() > MAX_ENTRIES {
            return Err(Error::InvalidInput(format!(
                "{}: el paquete tiene demasiadas entradas ({})",
                path.display(),
                zip.len()
            )));
        }
        let mut index = HashMap::new();
        let mut names = Vec::new();
        let mut pkg = Package {
            zip,
            index: HashMap::new(),
            names: Vec::new(),
            path_label: path.display().to_string(),
        };
        for i in 0..pkg.zip.len() {
            let Ok(entry) = pkg.zip.by_index_raw(i) else { continue };
            if entry.is_dir() {
                continue;
            }
            let name = normalize(entry.name());
            index.entry(name.to_lowercase()).or_insert(i);
            names.push(name);
        }
        pkg.index = index;
        pkg.names = names;
        Ok(pkg)
    }

    /// Nombres de los ficheros, normalizados.
    pub(crate) fn names(&self) -> &[String] {
        &self.names
    }

    /// Bytes de la entrada `name` (sin distinguir mayúsculas). `Ok(None)` si no existe; error si pasa de `limit`.
    pub(crate) fn read(&mut self, name: &str, limit: u64) -> Result<Option<Vec<u8>>> {
        let Some(&i) = self.index.get(&normalize(name).to_lowercase()) else {
            return Ok(None);
        };
        let entry = self
            .zip
            .by_index(i)
            .map_err(|e| Error::InvalidInput(format!("{}: «{name}» no se puede leer ({e})", self.path_label)))?;
        let mut out = Vec::new();
        entry
            .take(limit + 1)
            .read_to_end(&mut out)
            .map_err(|e| Error::InvalidInput(format!("{}: «{name}» está dañado ({e})", self.path_label)))?;
        if out.len() as u64 > limit {
            return Err(Error::InvalidInput(format!(
                "{}: «{name}» es demasiado grande para abrirlo",
                self.path_label
            )));
        }
        Ok(Some(out))
    }

    /// Como [`Package::read`], pero falla si falta.
    pub(crate) fn require(&mut self, name: &str, limit: u64) -> Result<Vec<u8>> {
        self.read(name, limit)?
            .ok_or_else(|| Error::InvalidInput(format!("{}: falta «{name}»", self.path_label)))
    }
}

/// `\` → `/`, sin `/` inicial ni `./`.
pub(crate) fn normalize(name: &str) -> String {
    let n = name.replace('\\', "/");
    let n = n.trim_start_matches("./").trim_start_matches('/');
    n.to_string()
}

/// Decodifica `%20` y similares (bytes UTF-8).
pub(crate) fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2]))
        {
            out.push(h * 16 + l);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex(c: u8) -> Option<u8> {
    (c as char).to_digit(16).map(|d| d as u8)
}

/// Resuelve `href` (relativo o absoluto dentro del paquete) respecto al fichero `base`. Quita `#fragmento` y `?consulta`
/// y decodifica los `%XX`. Devuelve `None` si apunta fuera del paquete (`..` de más) o es una URL externa.
pub(crate) fn resolve(base: &str, href: &str) -> Option<String> {
    let href = href.trim();
    if href.is_empty() || href.contains("://") || href.starts_with("data:") || href.starts_with("mailto:") {
        return None;
    }
    let href = href.split(['#', '?']).next().unwrap_or("");
    let href = percent_decode(href).replace('\\', "/");
    let mut parts: Vec<&str> = Vec::new();
    if !href.starts_with('/') {
        let dir = base.rsplit_once('/').map_or("", |(d, _)| d);
        parts.extend(dir.split('/').filter(|p| !p.is_empty()));
    }
    for seg in href.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            other => parts.push(other),
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_relative_paths() {
        assert_eq!(
            resolve("OEBPS/text/ch1.xhtml", "../img/a%20b.png#x").as_deref(),
            Some("OEBPS/img/a b.png")
        );
        assert_eq!(resolve("a.opf", "b/c.html").as_deref(), Some("b/c.html"));
        assert_eq!(resolve("a/b.xml", "/c/d.png").as_deref(), Some("c/d.png"));
        assert_eq!(resolve("a.xml", "../fuera.png"), None);
        assert_eq!(resolve("a.xml", "https://x.org/a.png"), None);
    }

    #[test]
    fn decodes_percent_escapes() {
        assert_eq!(percent_decode("a%20b%C3%A1%zz%4"), "a bá%zz%4");
    }
}
