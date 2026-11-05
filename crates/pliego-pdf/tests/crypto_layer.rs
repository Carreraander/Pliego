//! El interruptor de cifrado de la capa (`PLIEGO_CRYPTO=native`, W40): MuPDF serializa sin cifrar y el modelo propio
//! cifra. Este archivo es un binario aparte porque el interruptor es global del proceso.

use std::path::Path;
use std::process::Command;

use pliego_pdf::{CryptoBackend, Document, Encryption, MetadataName, PdfDocument, Permission, WriteOptions};

fn corpus(rel: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus")
        .join(rel)
}

/// MuPDF escribe `/Encrypt <<…>>` directo en el trailer; el modelo propio, `/Encrypt N 0 R`.
fn encrypt_is_indirect(bytes: &[u8]) -> bool {
    bytes
        .windows(10)
        .any(|w| w.starts_with(b"/Encrypt ") && w[9].is_ascii_digit())
}

#[test]
fn native_backend_encrypts_through_the_layer() {
    set_native();
    assert_eq!(pliego_pdf::crypto_backend(), CryptoBackend::Native);
    let doc = PdfDocument::open(corpus("basic/hola.pdf")).unwrap();
    for (enc, algo) in [
        (Encryption::Rc4_40, "R = 2"),
        (Encryption::Rc4_128, "R = 3"),
        (Encryption::Aes128, "R = 4"),
        (Encryption::Aes256, "R = 6"),
    ] {
        let mut o = WriteOptions::new();
        o.set_encryption(enc)
            .set_user_password("abre")
            .set_owner_password("manda");
        let bytes = doc.to_bytes(&o).unwrap();
        assert!(encrypt_is_indirect(&bytes), "{enc:?}: no lo escribió el modelo propio");
        for pw in ["abre", "manda"] {
            let mut d = Document::from_bytes(&bytes, "application/pdf").unwrap();
            assert!(d.needs_password().unwrap());
            assert!(d.authenticate(pw).unwrap(), "{enc:?} {pw}");
            assert_eq!(d.page_count().unwrap(), 2);
            let t = d
                .load_page(0)
                .unwrap()
                .to_text_page(Default::default())
                .unwrap()
                .to_text()
                .unwrap();
            assert!(t.contains("Hola Pliego"), "{enc:?}");
        }
        let mut bad = Document::from_bytes(&bytes, "application/pdf").unwrap();
        assert!(!bad.authenticate("mala").unwrap());
        if let Some(log) = qpdf_show(&bytes, "abre") {
            assert!(log.contains(algo), "{enc:?}: {log}");
        }
    }
}

#[test]
fn native_backend_writes_permissions_and_files_to_disk() {
    set_native();
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("p.pdf");
    let doc = PdfDocument::open(corpus("basic/hola.pdf")).unwrap();
    let mut o = WriteOptions::new();
    o.set_encryption(Encryption::Aes256)
        .set_owner_password("manda")
        .set_permissions(Permission::PRINT | Permission::COPY);
    o.set_garbage(true);
    doc.save_with_options(&out, &o).unwrap();
    let bytes = std::fs::read(&out).unwrap();
    assert!(encrypt_is_indirect(&bytes));
    // Se abre sin contraseña de usuario; los permisos que declara el archivo son los pedidos.
    let reopened = PdfDocument::open(&out).unwrap();
    assert!(
        !reopened.needs_password().unwrap() || {
            let mut r = PdfDocument::open(&out).unwrap();
            r.authenticate("").unwrap()
        }
    );
    if let Some(log) = qpdf_show(&bytes, "") {
        assert!(
            log.contains("extract for any purpose: allowed") && log.contains("modify anything: not allowed"),
            "{log}"
        );
    }
}

#[test]
fn reopening_and_changing_the_encryption_keeps_the_content() {
    set_native();
    let doc = PdfDocument::open(corpus("basic/hola.pdf")).unwrap();
    let mut o = WriteOptions::new();
    o.set_encryption(Encryption::Aes128)
        .set_user_password("uno")
        .set_owner_password("dos");
    let first = doc.to_bytes(&o).unwrap();
    // Abrir cifrado (lo lee MuPDF), guardar con otro cifrado y otras contraseñas.
    let mut again = PdfDocument::from_bytes(&first).unwrap();
    assert!(again.authenticate("uno").unwrap());
    let mut o2 = WriteOptions::new();
    o2.set_encryption(Encryption::Aes256)
        .set_user_password("tres")
        .set_owner_password("cuatro");
    let second = again.to_bytes(&o2).unwrap();
    let mut d = PdfDocument::from_bytes(&second).unwrap();
    assert!(!d.authenticate("uno").unwrap());
    assert!(d.authenticate("tres").unwrap());
    assert_eq!(d.page_count().unwrap(), 2);
    assert_eq!(d.metadata(MetadataName::Title).unwrap(), "Documento de prueba");
    // Guardar sin pedir cifrado conserva el del original (MuPDF).
    let mut kept_src = PdfDocument::from_bytes(&second).unwrap();
    assert!(kept_src.authenticate("tres").unwrap());
    let kept = kept_src.to_bytes(&WriteOptions::new()).unwrap();
    assert!(PdfDocument::from_bytes(&kept).unwrap().needs_password().unwrap());
    // Y quitarlo lo deja en claro.
    let mut clear_o = WriteOptions::new();
    clear_o.set_encryption(Encryption::None);
    let clear = kept_src.to_bytes(&clear_o).unwrap();
    assert!(!PdfDocument::from_bytes(&clear).unwrap().needs_password().unwrap());
}

fn set_native() {
    pliego_pdf::set_crypto_backend(CryptoBackend::Native);
}

fn qpdf_show(bytes: &[u8], password: &str) -> Option<String> {
    Command::new("qpdf").arg("--version").output().ok()?;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("x.pdf");
    std::fs::write(&path, bytes).unwrap();
    let out = Command::new("qpdf")
        .arg("--show-encryption")
        .arg(format!("--password={password}"))
        .arg(&path)
        .output()
        .unwrap();
    Some(format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    ))
}
