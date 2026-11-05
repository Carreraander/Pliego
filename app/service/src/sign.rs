//! Firmas de la app (W28): firma digital PAdES, validación y flujo de firmas, sin Tauri.
//!
//! Capa fina sobre `pliego-sign` y `pliego-signflow`: elige el firmante (almacén de Windows o archivo PFX) y deja que
//! los comandos de `app/src-tauri` solo traduzcan argumentos. Todas las operaciones escriben una copia nueva; el
//! original no se toca.

use std::path::{Path, PathBuf};

use pliego_core::{Error, JobContext, Report, Result};
use pliego_sign::{Pkcs12Signer, SignLang, SignOptions, SignatureReport, Signer};
use pliego_signflow::{AuditEntry, PendingSigner, SignerSpec, Signers};
use serde::{Deserialize, Serialize};

/// De dónde sale la clave con la que se firma.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum KeySource {
    /// Certificado del almacén personal de Windows (la clave puede estar en una tarjeta, como el DNIe).
    Store { thumbprint: String },
    /// Archivo PKCS#12 (.pfx/.p12).
    Pfx { path: PathBuf, password: String },
}

/// Certificado ofrecido en el selector del asistente.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CertInfo {
    pub thumbprint: String,
    pub subject: String,
    pub issuer: String,
    pub not_after: String,
    pub has_private_key: bool,
}

/// Certificados del almacén personal de Windows. En otros sistemas no hay almacén: lista vacía.
pub fn list_certificates() -> Result<Vec<CertInfo>> {
    #[cfg(windows)]
    {
        Ok(pliego_sign::list_windows_certificates()?
            .into_iter()
            .map(|c| CertInfo {
                thumbprint: c.thumbprint,
                subject: c.subject,
                issuer: c.issuer,
                not_after: c.not_after,
                has_private_key: c.has_private_key,
            })
            .collect())
    }
    #[cfg(not(windows))]
    {
        Ok(Vec::new())
    }
}

/// Abre el firmante elegido.
pub fn open_signer(key: &KeySource) -> Result<Box<dyn Signer>> {
    match key {
        KeySource::Pfx { path, password } => Ok(Box::new(Pkcs12Signer::from_file(path, password)?)),
        #[cfg(windows)]
        KeySource::Store { thumbprint } => Ok(Box::new(pliego_sign::WindowsStoreSigner::open(thumbprint)?)),
        #[cfg(not(windows))]
        KeySource::Store { .. } => Err(Error::Unsupported(
            "el almacén de certificados solo está disponible en Windows".into(),
        )),
    }
}

fn not_same(src: &Path, out: &Path) -> Result<()> {
    if src == out {
        return Err(Error::InvalidInput(
            "el archivo de salida no puede ser el mismo que el de entrada".into(),
        ));
    }
    Ok(())
}

fn read(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path).map_err(|e| Error::io_at(path, e))
}

fn write_new(path: &Path, data: &[u8]) -> Result<()> {
    let tmp = path.with_extension("pliego-tmp");
    std::fs::write(&tmp, data).map_err(|e| Error::io_at(&tmp, e))?;
    std::fs::rename(&tmp, path).map_err(|e| Error::io_at(path, e))
}

/// Firma `src` y escribe el resultado en `out`.
pub fn sign_pdf(src: &Path, out: &Path, key: &KeySource, options: SignOptions, ctx: &JobContext) -> Result<Report> {
    not_same(src, out)?;
    let signer = open_signer(key)?;
    pliego_sign::sign(src, out, signer.as_ref(), options, ctx)
}

/// Comprueba las firmas del documento (sin raíces de confianza propias: una cadena válida pero desconocida sale como
/// «raíz no reconocida»).
pub fn validate(path: &Path) -> Result<Vec<SignatureReport>> {
    pliego_sign::validate(path)
}

/// Prepara `src` para que lo firmen `signers` en orden y lo escribe en `out`.
pub fn flow_prepare(
    src: &Path,
    out: &Path,
    signers: Vec<SignerSpec>,
    lang: SignLang,
    ctx: &JobContext,
) -> Result<Report> {
    not_same(src, out)?;
    ctx.step(0, 2, "Preparando el paquete")?;
    let n = signers.len();
    let pkg = pliego_signflow::prepare_in(&read(src)?, &Signers::new(signers), lang)?;
    write_new(out, &pkg.pdf)?;
    ctx.step(2, 2, "Listo")?;
    Ok(Report::with_output(
        out,
        format!("Paquete preparado para {n} firmantes"),
    ))
}

/// A quién le toca firmar; `None` si ya firmaron todos.
pub fn flow_next(path: &Path) -> Result<Option<PendingSigner>> {
    pliego_signflow::next_signer(&read(path)?)
}

/// Añade la firma del siguiente firmante a `src` y escribe el resultado en `out`.
pub fn flow_sign(src: &Path, out: &Path, key: &KeySource, options: SignOptions, ctx: &JobContext) -> Result<Report> {
    not_same(src, out)?;
    ctx.step(0, 2, "Firmando")?;
    let signer = open_signer(key)?;
    let (pdf, after) = pliego_signflow::sign_next(&read(src)?, signer.as_ref(), options)?;
    write_new(out, &pdf)?;
    ctx.step(2, 2, "Listo")?;
    let summary = match after {
        Some(p) => format!("Firmado. Siguiente: {} ({} de {})", p.spec.name, p.index + 1, p.total),
        None => "Firmado. Ya han firmado todos".to_string(),
    };
    Ok(Report::with_output(out, summary))
}

/// Registro de auditoría del flujo.
pub fn flow_audit(path: &Path) -> Result<Vec<AuditEntry>> {
    pliego_signflow::audit_log(&read(path)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use p12_keystore::{Certificate, KeyStore, KeyStoreEntry, PrivateKey, PrivateKeyChain};
    use rcgen::{CertificateParams, DnType, KeyPair, KeyUsagePurpose, PKCS_ECDSA_P256_SHA256};

    const PASSWORD: &str = "prueba";

    fn pdf() -> Vec<u8> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/basic/hola.pdf");
        std::fs::read(path).unwrap()
    }

    /// PFX autofirmado de pruebas (nunca claves reales).
    fn pfx(dir: &Path, name: &str) -> KeySource {
        let key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).unwrap();
        let mut params = CertificateParams::new(Vec::<String>::new()).unwrap();
        params.distinguished_name.push(DnType::CommonName, name);
        params.key_usages = vec![KeyUsagePurpose::DigitalSignature, KeyUsagePurpose::ContentCommitment];
        let cert = params.self_signed(&key).unwrap();
        let chain = PrivateKeyChain::new(
            b"k".to_vec(),
            PrivateKey::from_der(&key.serialize_der()).unwrap(),
            [Certificate::from_der(cert.der()).unwrap()],
        );
        let mut store = KeyStore::new();
        store.add_entry("firma", KeyStoreEntry::PrivateKeyChain(chain));
        let path = dir.join(format!("{name}.p12"));
        std::fs::write(&path, store.writer(PASSWORD).write().unwrap()).unwrap();
        KeySource::Pfx {
            path,
            password: PASSWORD.into(),
        }
    }

    #[test]
    fn signs_a_copy_and_validates_it() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.pdf");
        std::fs::write(&src, pdf()).unwrap();
        let out = dir.path().join("a (firmado).pdf");
        let opts = SignOptions {
            reason: Some("Aprobado".into()),
            ..Default::default()
        };
        let report = sign_pdf(&src, &out, &pfx(dir.path(), "Ana Prueba"), opts, &JobContext::silent()).unwrap();
        assert_eq!(report.outputs, vec![out.clone()]);
        assert_eq!(std::fs::read(&src).unwrap(), pdf(), "el original no cambia");
        let sigs = validate(&out).unwrap();
        assert_eq!(sigs.len(), 1);
        assert!(sigs[0].integrity);
        assert_eq!(sigs[0].signer, "Ana Prueba");
        assert!(validate(&src).unwrap().is_empty());
    }

    #[test]
    fn wrong_password_and_same_output_are_errors() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.pdf");
        std::fs::write(&src, pdf()).unwrap();
        let KeySource::Pfx { path, .. } = pfx(dir.path(), "Ana") else {
            unreachable!()
        };
        let bad = KeySource::Pfx {
            path,
            password: "mal".into(),
        };
        let out = dir.path().join("b.pdf");
        assert!(sign_pdf(&src, &out, &bad, SignOptions::default(), &JobContext::silent()).is_err());
        assert!(!out.exists());
        let ok = pfx(dir.path(), "Ana2");
        assert!(sign_pdf(&src, &src, &ok, SignOptions::default(), &JobContext::silent()).is_err());
    }

    #[cfg(not(windows))]
    #[test]
    fn store_is_unsupported_outside_windows() {
        assert!(list_certificates().unwrap().is_empty());
        assert!(
            open_signer(&KeySource::Store {
                thumbprint: "AB".into()
            })
            .is_err()
        );
    }

    #[test]
    fn signflow_runs_in_order_and_keeps_an_audit_log() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("c.pdf");
        std::fs::write(&src, pdf()).unwrap();
        let prepared = dir.path().join("c (para firmar).pdf");
        let ctx = JobContext::silent();
        let signers = vec![SignerSpec::new("Ana"), SignerSpec::new("Beto")];
        flow_prepare(&src, &prepared, signers, SignLang::Es, &ctx).unwrap();
        assert_eq!(flow_next(&prepared).unwrap().unwrap().spec.name, "Ana");

        let one = dir.path().join("c1.pdf");
        flow_sign(&prepared, &one, &pfx(dir.path(), "Ana"), SignOptions::default(), &ctx).unwrap();
        assert_eq!(flow_next(&one).unwrap().unwrap().spec.name, "Beto");

        let two = dir.path().join("c2.pdf");
        let report = flow_sign(&one, &two, &pfx(dir.path(), "Beto"), SignOptions::default(), &ctx).unwrap();
        assert!(report.summary.contains("todos"));
        assert!(flow_next(&two).unwrap().is_none());
        let log = flow_audit(&two).unwrap();
        assert_eq!(log.len(), 3, "preparado + dos firmas");
    }
}
