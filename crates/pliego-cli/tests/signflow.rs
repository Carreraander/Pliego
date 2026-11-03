//! `pliego signflow`: preparar, firmar en orden y auditar.
use std::path::{Path, PathBuf};
use std::process::Command;

use p12_keystore::{Certificate, KeyStore, KeyStoreEntry, PrivateKey, PrivateKeyChain};
use rcgen::{BasicConstraints, CertificateParams, DnType, IsCa, Issuer, KeyPair, KeyUsagePurpose};

fn pliego() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_pliego"));
    c.env("PLIEGO_P12_PASSWORD", "prueba");
    c
}

fn make_p12(path: &Path, cn: &str) {
    let ca_key = KeyPair::generate().unwrap();
    let mut ca_params = CertificateParams::new(Vec::<String>::new()).unwrap();
    ca_params.distinguished_name.push(DnType::CommonName, "CA de pruebas");
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    ca_params.key_usages = vec![KeyUsagePurpose::KeyCertSign];
    let ca = ca_params.self_signed(&ca_key).unwrap();
    let issuer = Issuer::new(ca_params, ca_key);
    let key = KeyPair::generate().unwrap();
    let mut params = CertificateParams::new(Vec::<String>::new()).unwrap();
    params.distinguished_name.push(DnType::CommonName, cn);
    let leaf = params.signed_by(&key, &issuer).unwrap();
    let chain = PrivateKeyChain::new(
        b"k".to_vec(),
        PrivateKey::from_der(&key.serialize_der()).unwrap(),
        [
            Certificate::from_der(leaf.der()).unwrap(),
            Certificate::from_der(ca.der()).unwrap(),
        ],
    );
    let mut store = KeyStore::new();
    store.add_entry("f", KeyStoreEntry::PrivateKeyChain(chain));
    std::fs::write(path, store.writer("prueba").write().unwrap()).unwrap();
}

#[test]
fn prepare_sign_audit() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/basic/hola.pdf");
    let run = |args: &[&str]| {
        let out = pliego().arg("signflow").args(args).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8(out.stdout).unwrap()
    };
    let s = |p: &str| d.join(p).to_string_lossy().into_owned();
    run(&[
        "prepare",
        src.to_str().unwrap(),
        "-o",
        &s("p0.pdf"),
        "--signer",
        "Ana<ana@example.org>",
        "--signer",
        "Beto",
    ]);
    assert!(run(&["next", &s("p0.pdf")]).contains("Ana"));
    make_p12(&d.join("a.p12"), "Ana");
    make_p12(&d.join("b.p12"), "Beto");
    run(&["sign", &s("p0.pdf"), "-o", &s("p1.pdf"), "--p12", &s("a.p12")]);
    assert!(run(&["next", &s("p1.pdf")]).contains("Beto"));
    run(&["sign", &s("p1.pdf"), "-o", &s("p2.pdf"), "--p12", &s("b.p12")]);
    assert!(run(&["next", &s("p2.pdf")]).contains("Ya han firmado"));
    let audit = run(&["audit", &s("p2.pdf")]);
    assert!(
        audit.contains("Firma de Ana") && audit.contains("Firma de Beto"),
        "{audit}"
    );
}
