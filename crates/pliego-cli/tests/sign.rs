//! `pliego sign` + `pliego verify` con una CA y un certificado de prueba generados al vuelo.
use std::path::{Path, PathBuf};
use std::process::Command;

use p12_keystore::{Certificate, KeyStore, KeyStoreEntry, PrivateKey, PrivateKeyChain};
use rcgen::{BasicConstraints, CertificateParams, DnType, IsCa, Issuer, KeyPair, KeyUsagePurpose};

fn pliego() -> Command {
    Command::new(env!("CARGO_BIN_EXE_pliego"))
}

fn corpus() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/basic/hola.pdf")
}

/// Escribe `test.p12` (contraseña "prueba") y `ca.der` en `dir`.
fn make_pki(dir: &Path) {
    let ca_key = KeyPair::generate().unwrap();
    let mut ca_params = CertificateParams::new(Vec::<String>::new()).unwrap();
    ca_params
        .distinguished_name
        .push(DnType::CommonName, "CA de pruebas Pliego");
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    ca_params.key_usages = vec![KeyUsagePurpose::KeyCertSign];
    let ca = ca_params.self_signed(&ca_key).unwrap();
    let issuer = Issuer::new(ca_params, ca_key);
    let key = KeyPair::generate().unwrap();
    let mut params = CertificateParams::new(Vec::<String>::new()).unwrap();
    params.distinguished_name.push(DnType::CommonName, "Persona de Prueba");
    let leaf = params.signed_by(&key, &issuer).unwrap();
    let chain = PrivateKeyChain::new(
        b"k1".to_vec(),
        PrivateKey::from_der(&key.serialize_der()).unwrap(),
        [
            Certificate::from_der(leaf.der()).unwrap(),
            Certificate::from_der(ca.der()).unwrap(),
        ],
    );
    let mut store = KeyStore::new();
    store.add_entry("firma", KeyStoreEntry::PrivateKeyChain(chain));
    std::fs::write(dir.join("test.p12"), store.writer("prueba").write().unwrap()).unwrap();
    std::fs::write(dir.join("ca.der"), ca.der()).unwrap();
}

#[test]
fn sign_then_verify() {
    let dir = tempfile::tempdir().unwrap();
    make_pki(dir.path());
    let signed = dir.path().join("firmado.pdf");
    let out = pliego()
        .arg("sign")
        .arg(corpus())
        .arg("-o")
        .arg(&signed)
        .arg("--p12")
        .arg(dir.path().join("test.p12"))
        .args(["--password", "prueba", "--reason", "Prueba"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));

    let out = pliego()
        .args(["--json", "verify"])
        .arg(&signed)
        .arg("--trust")
        .arg(dir.path().join("ca.der"))
        .arg("--strict")
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stdout));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v[0]["signer"], "Persona de Prueba");
    assert_eq!(v[0]["chain"], "trusted");

    // Un byte cambiado en el contenido firmado rompe la firma y la CLI falla.
    let mut bytes = std::fs::read(&signed).unwrap();
    let pos = bytes.windows(5).position(|w| w == b"Hola ").unwrap();
    bytes[pos] = b'h';
    let broken = dir.path().join("roto.pdf");
    std::fs::write(&broken, bytes).unwrap();
    let out = pliego().arg("verify").arg(&broken).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("ROTA"));
}

#[test]
fn verify_unsigned_document_fails() {
    let out = pliego().arg("verify").arg(corpus()).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("no contiene firmas"));
}

#[test]
fn sign_refuses_to_overwrite_input() {
    let dir = tempfile::tempdir().unwrap();
    make_pki(dir.path());
    let copy = dir.path().join("copia.pdf");
    std::fs::copy(corpus(), &copy).unwrap();
    let out = pliego()
        .arg("sign")
        .arg(&copy)
        .arg("-o")
        .arg(&copy)
        .arg("--p12")
        .arg(dir.path().join("test.p12"))
        .args(["--password", "prueba"])
        .output()
        .unwrap();
    assert!(!out.status.success());
}

#[test]
fn level_b_t_without_tsa_fails_cleanly() {
    let dir = tempfile::tempdir().unwrap();
    make_pki(dir.path());
    let out = pliego()
        .arg("sign")
        .arg(corpus())
        .arg("-o")
        .arg(dir.path().join("x.pdf"))
        .arg("--p12")
        .arg(dir.path().join("test.p12"))
        .args(["--password", "prueba", "--level", "b-t"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("sellado de tiempo"));
}

#[test]
fn visible_signature_changes_the_rendered_page() {
    let dir = tempfile::tempdir().unwrap();
    make_pki(dir.path());
    let signed = dir.path().join("visible.pdf");
    let out = pliego()
        .arg("sign")
        .arg(corpus())
        .arg("-o")
        .arg(&signed)
        .arg("--p12")
        .arg(dir.path().join("test.p12"))
        .args(["--password", "prueba", "--visible", "1,72,600,240,60"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));

    let render = |pdf: &std::path::Path, name: &str| {
        let o = dir.path().join(name);
        std::fs::create_dir_all(&o).unwrap();
        let r = pliego()
            .arg("render")
            .arg(pdf)
            .args(["--pages", "1", "--dpi", "72", "-o"])
            .arg(&o)
            .output()
            .unwrap();
        assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
        std::fs::read(std::fs::read_dir(&o).unwrap().next().unwrap().unwrap().path()).unwrap()
    };
    assert_ne!(
        render(&corpus(), "a"),
        render(&signed, "b"),
        "la apariencia debe verse en la página"
    );

    let bad = pliego()
        .arg("sign")
        .arg(corpus())
        .arg("-o")
        .arg(dir.path().join("x.pdf"))
        .arg("--p12")
        .arg(dir.path().join("test.p12"))
        .args(["--visible", "1,2,3"])
        .output()
        .unwrap();
    assert!(!bad.status.success());
}

#[test]
fn certify_is_reported_by_verify() {
    let dir = tempfile::tempdir().unwrap();
    make_pki(dir.path());
    let signed = dir.path().join("cert.pdf");
    let out = pliego()
        .arg("sign")
        .arg(corpus())
        .arg("-o")
        .arg(&signed)
        .arg("--p12")
        .arg(dir.path().join("test.p12"))
        .args(["--password", "prueba", "--certify", "2"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let out = pliego().args(["--json", "verify"]).arg(&signed).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v[0]["certification"], 2);
    let bad = pliego()
        .arg("sign")
        .arg(corpus())
        .arg("-o")
        .arg(dir.path().join("x.pdf"))
        .arg("--p12")
        .arg(dir.path().join("test.p12"))
        .args(["--password", "prueba", "--certify", "9"])
        .output()
        .unwrap();
    assert!(!bad.status.success());
}
