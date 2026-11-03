use std::path::Path;

use mupdf::pdf::{Encryption as MuEncryption, PdfWriteOptions, Permission};
use pliego_core::{Error, Report, Result};
use serde::{Deserialize, Serialize};

use crate::io::{open_pdf, pdf_err, save_atomic};

/// Algoritmo de cifrado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Cipher {
    /// AES de 256 bits (recomendado).
    #[default]
    Aes256,
    /// AES de 128 bits, para lectores antiguos.
    Aes128,
}

/// Qué puede hacer quien abre el documento sin la contraseña de permisos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Permissions {
    pub print: bool,
    pub print_high_quality: bool,
    pub copy: bool,
    pub modify: bool,
    pub annotate: bool,
    pub fill_forms: bool,
    pub accessibility: bool,
    pub assemble: bool,
}

impl Default for Permissions {
    fn default() -> Self {
        Self::all()
    }
}

impl Permissions {
    pub fn all() -> Self {
        Self {
            print: true,
            print_high_quality: true,
            copy: true,
            modify: true,
            annotate: true,
            fill_forms: true,
            accessibility: true,
            assemble: true,
        }
    }

    /// Nada permitido salvo lo que se active después.
    pub fn none() -> Self {
        Self {
            print: false,
            print_high_quality: false,
            copy: false,
            modify: false,
            annotate: false,
            fill_forms: false,
            accessibility: false,
            assemble: false,
        }
    }

    pub fn is_restricted(&self) -> bool {
        *self != Self::all()
    }

    fn to_bits(self) -> Permission {
        let mut p = Permission::empty();
        let pairs = [
            (self.print, Permission::PRINT),
            (self.print_high_quality, Permission::PRINT_HQ),
            (self.copy, Permission::COPY),
            (self.modify, Permission::MODIFY),
            (self.annotate, Permission::ANNOTATE),
            (self.fill_forms, Permission::FORM),
            (self.accessibility, Permission::ACCESSIBILITY),
            (self.assemble, Permission::ASSEMBLE),
        ];
        for (on, bit) in pairs {
            if on {
                p |= bit;
            }
        }
        p
    }

    fn from_bits(p: Permission) -> Self {
        Self {
            print: p.contains(Permission::PRINT),
            print_high_quality: p.contains(Permission::PRINT_HQ),
            copy: p.contains(Permission::COPY),
            modify: p.contains(Permission::MODIFY),
            annotate: p.contains(Permission::ANNOTATE),
            fill_forms: p.contains(Permission::FORM),
            accessibility: p.contains(Permission::ACCESSIBILITY),
            assemble: p.contains(Permission::ASSEMBLE),
        }
    }
}

/// Opciones de cifrado.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Encryption {
    pub cipher: Cipher,
    /// Contraseña para abrir el documento. Vacía = se abre sin contraseña (solo se aplican permisos).
    pub user_password: String,
    /// Contraseña de permisos (propietario). Obligatoria si hay restricciones.
    pub owner_password: String,
    pub permissions: Permissions,
}

impl Encryption {
    fn validate(&self) -> Result<()> {
        if self.user_password.is_empty() && self.owner_password.is_empty() {
            return Err(Error::InvalidInput(
                "indica al menos una contraseña (para abrir o de permisos)".into(),
            ));
        }
        if self.permissions.is_restricted() && self.owner_password.is_empty() {
            return Err(Error::InvalidInput(
                "para restringir permisos hace falta una contraseña de permisos; sin ella cualquiera podría quitarlos"
                    .into(),
            ));
        }
        // MuPDF limita las contraseñas a 127 bytes UTF-8.
        for (nombre, pw) in [
            ("de apertura", &self.user_password),
            ("de permisos", &self.owner_password),
        ] {
            if pw.len() > 127 {
                return Err(Error::InvalidInput(format!(
                    "la contraseña {nombre} es demasiado larga (máx. 127 bytes)"
                )));
            }
        }
        Ok(())
    }
}

fn write_options(enc: &Encryption) -> PdfWriteOptions {
    let mut o = PdfWriteOptions::default();
    o.set_encryption(match enc.cipher {
        Cipher::Aes256 => MuEncryption::Aes256,
        Cipher::Aes128 => MuEncryption::Aes128,
    });
    // Si solo hay contraseña de apertura, la de permisos coincide con ella.
    let owner = if enc.owner_password.is_empty() {
        &enc.user_password
    } else {
        &enc.owner_password
    };
    o.set_owner_password(owner);
    o.set_user_password(&enc.user_password);
    o.set_permissions(enc.permissions.to_bits());
    o.set_garbage(true);
    o
}

/// Cifra `input` y escribe `output`. `input_password` solo hace falta si el original ya estaba cifrado.
pub fn encrypt(input: &Path, output: &Path, input_password: Option<&str>, enc: &Encryption) -> Result<Report> {
    enc.validate()?;
    let doc = open_pdf(input, input_password)?;
    save_atomic(&doc, input, output, write_options(enc))?;
    let algo = match enc.cipher {
        Cipher::Aes256 => "AES-256",
        Cipher::Aes128 => "AES-128",
    };
    Ok(Report::with_output(output, format!("Documento cifrado con {algo}.")))
}

/// Quita la protección de un documento del que se conoce la contraseña.
pub fn decrypt(input: &Path, output: &Path, password: &str) -> Result<Report> {
    let doc = open_pdf(input, Some(password))?;
    let mut o = PdfWriteOptions::default();
    o.set_encryption(MuEncryption::None);
    save_atomic(&doc, input, output, o)?;
    Ok(Report::with_output(output, "Protección eliminada."))
}

/// Cambia los permisos de un documento (lo vuelve a cifrar con AES-256).
///
/// `current_password` abre el original si hace falta; `user_password` es la que pedirá el documento nuevo
/// (vacía = abre sin contraseña).
pub fn set_permissions(
    input: &Path,
    output: &Path,
    current_password: Option<&str>,
    owner_password: &str,
    user_password: &str,
    permissions: Permissions,
) -> Result<Report> {
    let enc = Encryption {
        cipher: Cipher::Aes256,
        user_password: user_password.to_owned(),
        owner_password: owner_password.to_owned(),
        permissions,
    };
    encrypt(input, output, current_password, &enc)
}

/// Permisos que declara el documento (entrada /P del diccionario de cifrado) para quien no tiene la
/// contraseña de permisos. Sin cifrado, todo está permitido.
///
/// Se lee /P directamente: `PdfDocument::permissions` de `mupdf` 0.8 devuelve "todo permitido" cuando /P
/// trae los bits reservados activados (lo normal), así que no sirve para esto.
pub fn permissions_of(input: &Path, password: Option<&str>) -> Result<Permissions> {
    let doc = open_pdf(input, password)?;
    let trailer = doc.trailer().map_err(pdf_err)?;
    let p = match trailer.get_dict("Encrypt").map_err(pdf_err)? {
        Some(enc) => match enc.get_dict("P").map_err(pdf_err)? {
            Some(p) => p.as_int().map_err(pdf_err)? as u32,
            None => return Ok(Permissions::all()),
        },
        None => return Ok(Permissions::all()),
    };
    Ok(Permissions::from_bits(Permission::from_bits_truncate(p)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn corpus() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/basic/hola.pdf")
    }

    fn bytes_contain(path: &Path, needle: &[u8]) -> bool {
        std::fs::read(path).unwrap().windows(needle.len()).any(|w| w == needle)
    }

    #[test]
    fn roundtrip_aes256() {
        let dir = tempfile::tempdir().unwrap();
        let enc_pdf = dir.path().join("c.pdf");
        let enc = Encryption {
            user_password: "abre".into(),
            owner_password: "manda".into(),
            ..Default::default()
        };
        encrypt(&corpus(), &enc_pdf, None, &enc).unwrap();
        assert!(bytes_contain(&enc_pdf, b"/V 5"), "debe usar AES-256 (V 5)");
        assert!(matches!(open_pdf(&enc_pdf, None), Err(Error::Password)));
        assert!(matches!(open_pdf(&enc_pdf, Some("mala")), Err(Error::Password)));
        let doc = open_pdf(&enc_pdf, Some("abre")).unwrap();
        assert_eq!(doc.page_count().unwrap(), 2);

        let plain = dir.path().join("p.pdf");
        decrypt(&enc_pdf, &plain, "abre").unwrap();
        assert!(open_pdf(&plain, None).is_ok());
        assert!(!bytes_contain(&plain, b"/Encrypt"));
    }

    #[test]
    fn aes128_uses_v4() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("c.pdf");
        let enc = Encryption {
            cipher: Cipher::Aes128,
            user_password: "x".into(),
            ..Default::default()
        };
        encrypt(&corpus(), &out, None, &enc).unwrap();
        assert!(bytes_contain(&out, b"/V 4"));
        assert!(bytes_contain(&out, b"/AESV2"));
    }

    #[test]
    fn permissions_are_written_and_read_back() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("c.pdf");
        let perms = Permissions {
            copy: false,
            modify: false,
            ..Permissions::all()
        };
        set_permissions(&corpus(), &out, None, "manda", "", perms).unwrap();
        // Se abre sin contraseña pero con restricciones.
        let got = permissions_of(&out, None).unwrap();
        assert_eq!(got, perms);
    }

    #[test]
    fn restricted_permissions_need_owner_password() {
        let dir = tempfile::tempdir().unwrap();
        let enc = Encryption {
            user_password: "x".into(),
            permissions: Permissions::none(),
            ..Default::default()
        };
        let err = encrypt(&corpus(), &dir.path().join("c.pdf"), None, &enc).unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)));
        assert!(matches!(
            encrypt(&corpus(), &dir.path().join("c.pdf"), None, &Encryption::default()),
            Err(Error::InvalidInput(_))
        ));
    }

    #[test]
    fn never_overwrites_input() {
        let dir = tempfile::tempdir().unwrap();
        let copy = dir.path().join("a.pdf");
        std::fs::copy(corpus(), &copy).unwrap();
        let enc = Encryption {
            user_password: "x".into(),
            ..Default::default()
        };
        assert!(encrypt(&copy, &copy, None, &enc).is_err());
        assert!(open_pdf(&copy, None).is_ok());
    }
}
